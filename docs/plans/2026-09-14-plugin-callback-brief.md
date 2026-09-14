# Plugin callback channel: scoping brief (#1622)

Status: decision record. Docs-only; no production code in this PR.
Date: 2026-09-14. Head: `656cddbe3` (`origin/main`).
Issue: #1622 (`mass migration: plugin callback channel for check_callable_call`).
Parent: #1625 (master plan; #1622 is ranked lowest priority there).

## 1. Verdict

**Do not build the Rust to Python hook invocation channel now.** The
arithmetic in section 5 says it buys at most 0.02 to 0.2 percent of the cold
self-check wall, and the channel cannot be built in the shape the issue
proposes because the call-hook contexts carry live Python objects the kernel
does not hold (section 3).

The real blocker is a different one and it is worse than the issue states:
**on every corpus that loads a user plugin, all native plugin-hook seams are
unconditionally inert**. The repository's own self-check is such a corpus
(`mypy_self_check.ini:22` sets `plugins = mypy.plugins.proper_plugin`), so
the primary gate corpus cannot exercise the machinery at all. Slice 1 below
fixes that; it is a plugin-API enumerability change, not a kernel seam.

## 2. Ground truth: the hook surface and its measured volume

### 2.1 Dispatch sites

| Site | Anchor | Kind |
|---|---|---|
| Call-hook dispatch | `mypy/checkexpr.py:3349-3362` | `get_function_hook` / `get_method_hook` |
| Signature-hook dispatch | `mypy/checkexpr.py:2606-2628` | `get_function_signature_hook` / `get_method_signature_hook` |
| Attribute-hook dispatch | `mypy/checkmember.py:1373-1374`, `:1784-1785`, `:1912-1913` | `get_attribute_hook` |
| Class-decorator dispatch | `mypy/semanal.py:2984-2985`, `mypy/semanal_main.py:519-520` | `get_class_decorator_hook`, `_2` |
| Metaclass / base-class dispatch | `mypy/semanal.py:3004-3005`, `:3019-3020` | not overridden by `DefaultPlugin` |
| Hook body application | `mypy/checkexpr.py:2430-2498` (function + method), `:2503+` (signature) | `apply_function_plugin`, `apply_signature_hook` |
| Class-hook body application | `mypy/semanal.py:2974` | `apply_class_plugin_hooks` |

Guards used by every site: `plugin_call_hook_known_absent`
(`mypy/checkexpr.py:530-561`) and `plugin_hook_known_absent`
(`mypy/checkexpr.py:565-587`). Native lookup:
`_try_native_plugin_hook` (`mypy/checkexpr.py:605-637`, method form at
`:1461-1463`) into `rust_resolve_plugin_hook`
(`crates/type_kernel/src/plugin_hooks.rs:112-149`), backed by
`PluginHookRegistry` (`crates/type_kernel/src/plugin_hooks.rs:38-95`).

The declared surface is larger than the issue states. `DEFAULT_HOOK_FULLNAMES_BY_KIND`
(`mypy/plugins/default.py:143-204`) is **81 distinct fullnames across 7 hook
kinds**; the issue's "~46" is `DEFAULT_CALL_HOOK_FULLNAMES`, the union of the
four call-hook kinds only. Per kind: `get_function_hook` 5,
`get_function_signature_hook` 7, `get_method_signature_hook` 21,
`get_method_hook` 20, `get_attribute_hook` 22, `get_class_decorator_hook` 12,
`get_class_decorator_hook_2` 13.

### 2.2 Measured volume (cold self-check)

One instrumented cold run, single process, no pytest, no `.so` build:

```
--config-file mypy_self_check.ini --no-incremental --no-native-parser -n0 -p mypy -p mypyc
353 source files, 0 errors, wall 66.8s, user 48.8s
```

The probe monkey-patched the Python dispatch sites from outside the tree and
was deleted afterwards; no instrumented code was committed. Because the type
kernel was not on `PYTHONPATH` in this run the native resolve calls did no
FFI, but that does not change any hook count: with a user plugin present the
resolve path returns `None` at `mypy/checkexpr.py:622-623` before touching
Rust, and the self-check has one (section 4.2).

Dispatch-site probes:

| Probe | Calls |
|---|---|
| `plugin_call_hook_known_absent` | 328,408 |
| `plugin_hook_known_absent(get_attribute_hook)` | 111,649 |
| `plugin_hook_known_absent(get_base_class_hook)` | 5,573 |
| `plugin_hook_known_absent(get_class_decorator_hook)` | 2,555 |
| `plugin_hook_known_absent(get_class_decorator_hook_2)` | 953 |
| `plugin_hook_known_absent(get_metaclass_hook)` | 226 |
| `_try_native_plugin_hook`, all 4 call kinds | 328,408 |

Python chain lookups at `ChainedPlugin` (the fallback each probe falls into):

| Lookup | Calls | Non-`None` (a hook fires) |
|---|---|---|
| `get_function_hook` | 95,788 | 25,474 |
| `get_method_hook` | 100,334 | 8,426 |
| `get_function_signature_hook` | 55,620 | 0 |
| `get_method_signature_hook` | 93,616 | 32 |
| `get_attribute_hook` | 111,649 | 200 |
| `get_class_decorator_hook` | 2,555 | 94 |
| `get_class_decorator_hook_2` | 953 | 61 |

Fires by fullname (hot entries only):

| Hook kind | Fullname | Fires |
|---|---|---|
| `get_function_hook` | `builtins.isinstance` | 13,862 (user plugin) |
| `get_function_hook` | `builtins.len` | 5,808 |
| `get_function_hook` | `mypy.types.get_proper_type` | 5,698 (user plugin) |
| `get_function_hook` | `mypy.types.get_proper_types` | 98 (user plugin) |
| `get_function_hook` | `functools.partial` | 6 |
| `get_function_hook` | `functools.singledispatch` | 2 |
| `get_method_hook` | `builtins.int.__neg__` | 8,052 |
| `get_method_hook` | `builtins.int.__pow__` | 266 |
| `get_method_hook` | `typing.Mapping.get` | 56 |
| `get_method_hook` | singledispatch `register` / register-callable `__call__` | 20 / 20 |
| `get_method_hook` | `builtins.tuple.__mul__` | 8 |
| `get_method_hook` | `builtins.int.__pos__` | 4 |
| `get_attribute_hook` | `enum.Enum.value` / `enum.Enum.name` | 198 / 2 |
| `get_method_signature_hook` | `typing.Mapping.get` / singledispatch `__call__` | 26 / 6 |
| `get_class_decorator_hook` | `dataclasses.dataclass` | 94 |
| `get_class_decorator_hook_2` | `dataclasses.dataclass` / `functools.total_ordering` | 57 / 4 |

Hook bodies actually run:

| Body | Calls |
|---|---|
| `ExpressionChecker.apply_function_plugin` | 16,950 (function 12,737 / method 4,213) |
| `ExpressionChecker.apply_signature_hook` | 110 |
| `SemanticAnalyzer.apply_class_plugin_hooks` | 7,420 |

Derived ratios, all from the table above:

- Call-hook dispatch visits: 179,172 (`get_function_hook` 83,051 plus
  `get_method_hook` 96,121 probes). Lookups exceed visits by exactly the body
  count (179,172 + 16,950 = 196,122) because `apply_function_plugin` re-looks
  up the callback at `mypy/checkexpr.py:2470` and `:2486`.
- A hook fires on **18.9 percent** of those visits (33,900 of 179,172).
  **58.0 percent of the fires are the user plugin** (19,658), leaving
  **7.9 percent of visits** with a `DefaultPlugin` hook.
- `DefaultPlugin` bodies on this corpus: **7,121** (2,908 function + 4,213
  method). The other 9,829 function-hook bodies are `proper_plugin`.

## 3. Why the proposed callback channel cannot be built as specified

The issue's approach 1 says "the kernel serializes the call arguments, Python
runs the hook, returns the result type as wire bytes". The invocation argument
is unsatisfiable for the two hot kinds, for a reason that is independent of
the wire format:

- `FunctionContext` (`mypy/plugin.py:451-468`) requires
  `args: list[list[Expression]]` and `api: CheckerPluginInterface`.
- `MethodContext` (`mypy/plugin.py:491-501`) requires the same two fields.
- `AttributeContext` (`mypy/plugin.py:505-510`) requires only
  `type`, `default_attr_type`, `is_lvalue`, `context`, `api`.

`args` are live AST nodes and `api` is the live `TypeChecker`. Rust holds
neither: the kernel's model is wire bytes and resolver snapshots (ADR-0001,
ADR-0002 Decision 4), and `mypy.checker.TypeChecker` is not a kernel object.
So a generic wire callback would have to reconstruct the context in Python
anyway, meaning the crossing replaces no Python work; it only moves the
"should I call the hook" decision, which the kernel **already owns** via
`rust_resolve_plugin_hook` (`crates/type_kernel/src/plugin_hooks.rs:112`).

That asymmetry is the honest read of the issue text: the *lookup* channel
exists and is landed; the *invocation* channel is the part that is blocked,
and it is blocked by the context shape, not by an absent wire field.

The attribute-hook and signature-hook contexts are the exception: they carry
no `Expression` list. A wire plus location callback is technically
constructible there. At 200 (`get_attribute_hook`) and 32
(`get_method_signature_hook`) fires per cold self-check, a channel for them
does not pay for itself.

## 4. Cost model

### 4.1 Per-crossing cost

The repository's recorded estimate for one zero-payload FFI crossing is
**0.63 microseconds** (from the 957k-call `TypeInfo.is_metaclass` measurement
recorded for issue #1137 in `AGENTS.md`). Treat this as an estimate: it was
measured on a different seam and a different call mix.

### 4.2 The user-plugin gate makes the current seams inert

`mypy/build.py:2095` computes `has_user_plugins = len(plugins) > 1`, and
`mypy/build.py:955-957` always wraps the plugin into a `ChainedPlugin`. Every
native hook guard then reads the bit:

- `mypy/checkexpr.py:554-556` (`plugin_call_hook_known_absent`),
  `:577` (`plugin_hook_known_absent`), `:622-623` and
  `:1107-1108` (`_try_native_plugin_hook`, `_try_native_check_callable_call`).

With `mypy_self_check.ini:22` loading `mypy.plugins.proper_plugin`, the
self-check has two plugins, so the bit is `True` and **every** native hook
site defers to the Python chain. Measured confirmation: 19,658 hook fires on
`builtins.isinstance` and `mypy.types.get_proper_type*`, which only
`proper_plugin` declares (`mypy/plugins/proper_plugin.py:45-53`).

One seam does not honour the bit. `rust_analyze_var`'s gate
(`crates/type_kernel/src/checkmember.rs:2638-2655`) imports
`mypy.checkexpr`, calls `plugin_hook_known_absent`, and then, when that says
"not absent", calls `plugin.get_attribute_hook(fullname)` over FFI anyway. On
the self-check that is 111,649 probes and 111,649 live chain lookups to
discover what the registry already knew. This is the measured cost of the
current design on the primary gate corpus: roughly 0.1 to 0.2 percent of wall
(order 100 milliseconds at 0.6 to 1 microsecond per probe), paid for no
coverage.

### 4.3 Approach 1, callback channel

For call hooks it is not constructible (section 3). For the kinds where it
is constructible it *adds* one crossing plus the context wire encode/decode
per invocation. At the measured `DefaultPlugin` body volume (7,121) the added
crossing alone is about 4.5 milliseconds; with serialization of
`type` plus `default_attr_type` it grows, and the saving side is zero on the
current seams, because no current seam defers *because of* a hook that a
channel could satisfy. `rust_check_callable_call`
(`crates/type_kernel/src/checkcall.rs:552-627`) is the one seam the issue
names as blocked, and its own docstring is explicit that it handles only the
type-object single-argument calibration, deferring on every other shape
(`:627-700`). Its deferrals are dominated by its own calibration guard, not
by the hook probe at `:592-611`.

**Net: negative or zero.** Do not build it.

### 4.4 Approach 2, port the hook bodies

Bounded by the `DefaultPlugin` body volume: **7,121 invocations** on a 66.8
second cold self-check. At a generous 2 to 5 microseconds per body (a
`FunctionContext` NamedTuple alone costs about 1 microsecond to build) that
is 14 to 36 milliseconds, or **0.02 to 0.05 percent of wall**. At an
implausible 20 microseconds per body it is still 0.2 percent.

Portable bodies, in descending fire count:

| Body | Anchor | Portability |
|---|---|---|
| `int_neg_callback` (`int.__neg__`, `__pos__`) | `mypy/plugins/default.py:673-701` | Pure over `ctx.type` (wire) plus one scalar, `is_literal_type_like(ctx.api.type_context[-1])` (`:683`), which the shim can supply. |
| `int_pow_callback` | `mypy/plugins/default.py:653-672` | Same shape as above. |
| `len_callback` (`builtins.len`) | `mypy/plugins/default.py:332-343` | Fully pure; needs the `mypy_extensions.i64` `Instance` as a fresh-instance site (resolver snapshot). |
| `tuple_mul_callback` | `mypy/plugins/default.py:710+` | Same shape as `int_neg_callback`. |
| `enum_value_callback` / `enum_name_callback` | `mypy/plugins/default.py` (via `ENUM_VALUE_ACCESS` / `ENUM_NAME_ACCESS`) | Pure; 200 fires total. |
| `typed_dict_get_callback` | `mypy/plugins/default.py:384+` | Not pure; 56 fires. Floor. |
| `functools` singledispatch / partial hooks | `mypy/plugins/default.py:212-223`, `:253-270` | Stateful (register tables, `ctx.api` mutation); 48 fires. Floor. |

**Verdict: not worth doing standalone.** It is a coverage item for a future
full `check_callable_call` port, not a performance item.

## 5. Arithmetic, stated plainly

```
cold self-check wall                                    66.8 s
DefaultPlugin hook bodies                                   7,121
estimated saving at 2-5 us/body                    14-36 ms = 0.02-0.05 %
call-hook visits with a DefaultPlugin hook          14,242 / 179,172 = 7.9 %
call-hook visits already hook-free                 145,272 / 179,172 = 81.1 %
hook fires attributable to a user plugin            19,658 /  33,900 = 58.0 %
native hook resolve attempts on the self-check         328,408, 0 usable
```

The performance case does not exist at this volume, and the coverage case
cannot be validated on the repository's own gate corpus until user-plugin
hooks are enumerable.

## 6. Accepted design (recorded, not scheduled)

### 6.1 Keep the lookup port; declare the surface instead of guessing it

The blocker is that `ChainedPlugin._find_hook` (`mypy/plugin.py:875-895`) asks
each plugin an arbitrary Python predicate, so a user plugin's hook set is not
enumerable and `has_user_plugins` must stay `True`. Fix that at the API
level:

1. `mypy/plugin.py`: add to `Plugin`
   `def declare_hook_fullnames(self) -> dict[str, frozenset[str]] | None: return None`,
   with `None` meaning "not enumerable". `DefaultPlugin` returns
   `DEFAULT_HOOK_FULLNAMES_BY_KIND` (`mypy/plugins/default.py:143-204`).
2. `mypy/plugin.py:835+` (`ChainedPlugin`): `declare_hook_fullnames` returns
   the per-kind union, or `None` if any child returns `None`.
3. `mypy/build.py:_build_plugin_hook_registry` (`:2058-2099`): install the
   union and set the user-plugin bit only when the union is `None`, replacing
   the current `len(plugins) > 1` test at `:2095`.
4. `mypy/plugins/proper_plugin.py`: declare its three names so the self-check
   exercises the fast path. `mypy/suggestions.py:436-444` already flips the
   bit manually and keeps working unchanged.

Backward compatibility is automatic: a third-party plugin that does not
implement `declare_hook_fullnames` returns `None`, the union is `None`, and
the bit stays `True`, which is today's conservative behaviour.

### 6.2 Then, and only then, port the pure bodies

If a future wave ports `check_callable_call` in full, port the pure bodies in
the table in section 4.4 as classifier seams. Each returns
`(decided, blob)` per the #1101 protocol, with `None` deferring. This is
sequenced after 6.1, not before: without 6.1 the port cannot be validated on
any corpus that loads a plugin.

## 7. Seam shape, gate, registrar and reset discipline

- **Gate**: reuse `Options.native_type_kernel`. No new option; no new entry
  in `OPTIONS_AFFECTING_CACHE` (the hook surface is config-static and already
  feeds cache validation through `report_config_data`,
  `mypy/plugin.py:847-849`).
- **Registrar**: no new registrar. The registry is built once per build
  manager at `mypy/build.py:1244-1247`, which clears the stale snapshot
  (`_set_native_plugin_hook_registry(None, False)`) before rebuilding. Per
  the setter's own docstring, plugins are config-static, so a per-manager
  install is sufficient and a daemon recheck needs no refresh.
- **Reset contract**: `_set_native_plugin_hook_registry` in
  `mypy/checkexpr.py:500-513` remains the single install/reset entry, with
  `(None, False)` restoring the conservative defer. Keep the
  `_set_native_plugin_hook_has_user_plugins` escape hatch
  (`mypy/checkexpr.py:516-527`) for `suggestions.py`.
- **Defer contract**: map only `PyAttributeError` to a defer; anything else
  propagates (the #1466/#1468 narrowing rule in `AGENTS.md`). Do not swallow
  into `Ok(None)`.

## 8. Test plan

For 6.1:

- Extend `NativePluginHookSuite` (`mypy/test/testtypes.py:17352-17460`):
  a declaring plugin installs a union and answers "known absent" for an
  undeclared name; a plugin returning `None` from `declare_hook_fullnames`
  forces the whole union to `None` and restores the defer; a mixed chain
  (declaring plus non-declaring) defers. Keep
  `test_known_absent_defers_when_user_plugins_present`
  (`mypy/test/testtypes.py:17414`) green by re-expressing it as the
  non-declaring case.
- Rust unit tests are unchanged; `plugin_hooks.rs:154-212` already covers the
  kind-keyed membership and the unknown-kind filter. Add one case for a union
  that is `None` on the Python side (the FFI must not be reached).
- Differential: cold self-check 0 errors with the fast path live; testcheck
  gate-off versus gate-on parity (exact match against the pre-change head;
  the local baseline is 8,144 passed / 69 skipped / 7 xfailed, CI's lower
  skip count differs only by environment).
- One measurement to record in the PR body: `plugin_hook_known_absent` and
  `plugin_call_hook_known_absent` answer counts with the self-check's
  `proper_plugin` now declaring, showing the 328,408 probes leave the Python
  chain.

For 6.2 (when a full `check_callable_call` port exists):

- The relevant `Native*Suite` is whichever suite the port adds; the hook
  bodies themselves need direct seam tests plus gate-off versus gate-on
  differentials on the returned type string, following the established
  `Native*Suite` pattern in `mypy/test/testtypes.py`.
- testcheck gate-off versus gate-on parity: zero diff.
- Cold self-check 0 errors.

## 9. Corrections to issue #1622

1. "~46 hook fullnames" understates the surface: 81 names across 7 kinds;
   46 is the call-hook union alone (`mypy/plugins/default.py:143-204`).
2. `rust_classify_check_arg` (`crates/type_kernel/src/checkexpr_functions.rs`)
   does **not** gate on plugin hooks. The file contains no plugin or hook
   reference. The issue's "affected seams" entry for it is wrong.
3. `rust_analyze_member_method` does not probe `get_method_hook` directly.
   The hook probe in `checkmember.rs` is the attribute-hook gate at
   `:2638-2655` and the `plugin_get_attribute_hook_hits` helper
   (`crates/type_kernel/src/checker_helpers.rs:155-168`, used at `:1479`).
4. `rust_get_protocol_member` no longer "defers on
   `get_attribute_hook`" unconditionally: the wave-62B work (#1517) replaced
   the blanket user-plugin refusal with a live chain probe
   (`checker_helpers.rs:153-168`). What remains is the attribute-hook verdict
   itself, which is the accepted pattern (section 6.1 target).
5. The premise "Blocks full `check_callable_call` port" is true only for a
   *future* full port. The shipped `rust_check_callable_call` is a
   calibration-only tail (`crates/type_kernel/src/checkcall.rs:552-627`) and
   its deferrals are dominated by its own guard, not by hooks.

## 10. Slices filed

- **#1626**: enumerable plugin hook surface, the design in section 6.1. This
  is the actual blocker from #1622.
- **#1627**: port the pure `DefaultPlugin` hook bodies, section 6.2. Marked
  low priority and explicitly gated on a full `check_callable_call` port, so
  the honest 0.02 to 0.05 percent arithmetic travels with it.

#1622 is closed docs-only by this brief, following the precedent set by
#1455, #1458, #1462, #1469, #1479, #1481, #1490, #1494 and #1506. The
replacement work lives in #1626 and #1627.