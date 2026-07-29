//! Stage 11b extended type codec (types_codec.rs) for Issue #92.
//!
//! Ports extended serialization helper utilities.

use pyo3::prelude::*;

#[pyfunction]
pub fn rust_is_valid_type_codec_tag(tag: u8) -> bool {
    tag <= 254
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_type_codec_tag() {
        assert!(rust_is_valid_type_codec_tag(0));
        assert!(rust_is_valid_type_codec_tag(254));
        assert!(!rust_is_valid_type_codec_tag(255));
    }
}
