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
    fn rejects_malformed_json_with_typed_source() {
        assert!(matches!(validate(b"{"), Err(FormatError::Json(_))));
    }
}
