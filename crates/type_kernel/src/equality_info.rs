//! parity for `mypy.checker.equality_value_info` (checker.py:10696).
//!
//! Computes the set of value-equality domains a type participates in, used by
//! `is_equality_ambiguous_for_narrowing` to detect whether two values may
//! compare equal through a broader value domain than their nominal type (an
//! IntEnum member vs its underlying int, a StrEnum member vs str, etc.).
//!
//! The Rust port folds `combine_equality_value_info` (checker.py:10724) into
//! the recursion (no separate pyfunction): unions and TypeVar.values combine
//! the per-item infos exactly as the Python generator expression does. A
//! `TypeAliasType` defers (`None`) — `get_proper_type` first resolves it from
//! live `TypeInfo`, which the wire format cannot do — and so does an
//! `Instance` whose snapshot is missing from the resolver (conservative).
//! The Python caller falls back to the pure-Python path in both cases.
//!
//! Return shape: `(is_top, domains)` where `domains` is a list of
//! `(domain_fullname, type_names, enum_type_names)` triples. All names are
//! sorted for determinism; the Python shim rebuilds sets from the lists, so
//! ordering is semantically irrelevant (set equality).

use std::collections::{HashMap, HashSet};

use pyo3::prelude::*;

use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, ReadBuffer, Type};

/// `(is_top, domains)` result of `rust_equality_value_info`, where each
/// domain is `(domain_fullname, type_names, enum_type_names)`.
type EqualityResult = Option<(bool, Vec<(String, Vec<String>, Vec<String>)>)>;

/// `VALUE_EQUALITY_DOMAINS` (checker.py:10611): merged open and closed value
/// domains. `(type_fullname, domain_fullname)`.
const VALUE_EQUALITY_DOMAINS: &[(&str, &str)] = &[
    ("builtins.str", "builtins.str"),
    ("builtins.bool", "builtins.numeric"),
    ("builtins.int", "builtins.numeric"),
    ("builtins.float", "builtins.numeric"),
    ("builtins.complex", "builtins.numeric"),
    ("builtins.bytes", "builtins.bytes"),
    ("builtins.bytearray", "builtins.bytes"),
    ("builtins.memoryview", "builtins.bytes"),
    ("typing.Mapping", "typing.Mapping"),
    ("typing.AbstractSet", "typing.AbstractSet"),
];

fn value_equality_domain(fullname: &str) -> Option<&'static str> {
    VALUE_EQUALITY_DOMAINS
        .iter()
        .find(|(k, _)| *k == fullname)
        .map(|(_, v)| *v)
}

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// `EqualityDomainInfo` (checker.py:10614): the member type's names and enum
/// names that participate in one equality domain.
#[derive(Debug, Clone)]
pub(crate) struct EqualityDomainInfo {
    pub(crate) type_names: HashSet<String>,
    pub(crate) enum_type_names: HashSet<String>,
}

/// `EqualityValueInfo` (checker.py:10620).
#[derive(Debug, Clone, Default)]
pub(crate) struct EqualityValueInfo {
    pub(crate) domains: HashMap<String, EqualityDomainInfo>,
    pub(crate) is_top: bool,
}

impl EqualityValueInfo {
    /// `EqualityValueInfo({}, is_top=True)` — Any and builtins.object.
    fn top() -> Self {
        EqualityValueInfo {
            domains: HashMap::new(),
            is_top: true,
        }
    }

    /// Step of `combine_equality_value_info` (checker.py:10724) applied to a
    /// single info: merge each domain (fresh copy on first sight, in-place
    /// set union on repeat) and OR the is_top flags.
    fn merge(&mut self, info: EqualityValueInfo) {
        for (domain, domain_info) in info.domains {
            match self.domains.get_mut(&domain) {
                None => {
                    self.domains.insert(
                        domain,
                        EqualityDomainInfo {
                            type_names: domain_info.type_names,
                            enum_type_names: domain_info.enum_type_names,
                        },
                    );
                }
                Some(existing) => {
                    existing.type_names.extend(domain_info.type_names);
                    existing.enum_type_names.extend(domain_info.enum_type_names);
                }
            }
        }
        self.is_top = self.is_top || info.is_top;
    }
}

/// `checker.equality_value_info(t)` — collect value-equality domains of `t`.
///
/// Mirrors checker.py:10696-10721 with `combine_equality_value_info`
/// folded in. Alias nodes expand through the type alias snapshot
/// (`expanded_alias_target`), mirroring the leading `get_proper_type`; a
/// snapshot miss or an undecidable substitution defers. Also defers on an
/// `Instance` whose TypeInfo snapshot is missing from the resolver. Recursion into
/// `last_known_value`, `LiteralType.fallback`, `TypeVarType` values /
/// upper_bound, and union items mirrors the Python dispatch order exactly.
/// Reused by `equality_ambiguity` for the per-item split.
pub(crate) fn equality_value_info_inner(
    t: &Type,
    resolver: &TypeResolver,
    aliases: &dyn crate::aliases::AliasLookup,
) -> Option<EqualityValueInfo> {
    match t {
        // get_proper_type(t) expands the alias from the live TypeAlias node
        // (checker.py:12505); mirror it with the alias snapshot. The
        // recursion re-enters through raw items, so each level re-expands.
        // A snapshot miss or an undecidable substitution defers (a snapshot
        // cycle is semanal-rejected, so its defer stays conservative).
        Type::TypeAliasType { .. } => {
            let (proper, _, _) = crate::checkexpr_functions::expanded_alias_target(t, aliases)?;
            equality_value_info_inner(&proper, resolver, aliases)
        }
        Type::UnionType { items, .. } => {
            let mut out = EqualityValueInfo::default();
            for item in items {
                out.merge(equality_value_info_inner(item, resolver, aliases)?);
            }
            Some(out)
        }
        Type::TypeVarType {
            values,
            upper_bound,
            ..
        } => {
            if values.is_empty() {
                return equality_value_info_inner(upper_bound, resolver, aliases);
            }
            let mut out = EqualityValueInfo::default();
            for value in values {
                out.merge(equality_value_info_inner(value, resolver, aliases)?);
            }
            Some(out)
        }
        Type::Instance {
            type_ref,
            last_known_value,
            ..
        } => {
            if let Some(lkv) = last_known_value {
                return equality_value_info_inner(lkv, resolver, aliases);
            }
            if type_ref == "builtins.object" {
                return Some(EqualityValueInfo::top());
            }
            let snap = resolver.get(type_ref)?;
            let mut enum_type_names = HashSet::new();
            if snap.is_enum {
                enum_type_names.insert(type_ref.clone());
            }
            let mut domains: HashMap<String, EqualityDomainInfo> = HashMap::new();
            for base in &snap.mro {
                if let Some(domain) = value_equality_domain(base) {
                    let mut type_names = HashSet::new();
                    type_names.insert(type_ref.clone());
                    domains.insert(
                        domain.to_string(),
                        EqualityDomainInfo {
                            type_names,
                            enum_type_names: enum_type_names.clone(),
                        },
                    );
                }
            }
            Some(EqualityValueInfo {
                domains,
                is_top: false,
            })
        }
        Type::LiteralType { fallback, .. } => {
            equality_value_info_inner(fallback, resolver, aliases)
        }
        Type::AnyType { .. } => Some(EqualityValueInfo::top()),
        _ => Some(EqualityValueInfo::default()),
    }
}

/// Native `rust_equality_value_info(t_bytes, resolver)` — parity seam for
/// `mypy.checker.equality_value_info`.
///
/// Returns `(is_top, [(domain, type_names, enum_type_names), ...])` with
/// names sorted for deterministic output, or `None` to defer to Python.
#[pyfunction]
pub(crate) fn rust_equality_value_info(
    t_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> EqualityResult {
    let t = decode_type(t_bytes)?;
    let info = equality_value_info_inner(&t, resolver.resolver(), resolver.alias_resolver())?;
    let mut domains: Vec<(String, Vec<String>, Vec<String>)> =
        Vec::with_capacity(info.domains.len());
    for (domain, domain_info) in info.domains {
        let mut type_names: Vec<String> = domain_info.type_names.into_iter().collect();
        type_names.sort();
        let mut enum_type_names: Vec<String> = domain_info.enum_type_names.into_iter().collect();
        enum_type_names.sort();
        domains.push((domain, type_names, enum_type_names));
    }
    Some((info.is_top, domains))
}
