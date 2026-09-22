use std::result::Result;

use crate::error::FormatError;

pub(super) fn validate(bytes: &[u8]) -> Result<(), FormatError> {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .map(|_| ())
        .map_err(FormatError::from)
}

#[cfg(test)]
mod tests {
    use crate::error::FormatError;

    use super::validate;

    #[test]
    fn accepts_well_formed_json() {
        assert!(validate(b"{}").is_ok());
        assert!(validate(b"[]").is_ok());
        assert!(validate(br#"{"env": {}}"#).is_ok());
        assert!(validate(b"null").is_ok());
    }

    #[test]
    fn rejects_malformed_json_with_typed_source() {
        for bytes in [b"".as_slice(), b"{".as_slice(), b"{'k': 1}".as_slice()] {
            assert!(matches!(validate(bytes), Err(FormatError::Json(_))));
        }
    }
}
