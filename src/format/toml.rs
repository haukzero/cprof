use std::result::Result;
use std::str;

use crate::error::FormatError;

pub(super) fn validate(bytes: &[u8]) -> Result<(), FormatError> {
    let text = str::from_utf8(bytes).map_err(FormatError::from)?;
    toml::from_str::<toml::Value>(text)
        .map(|_| ())
        .map_err(FormatError::from)
}

#[cfg(test)]
mod tests {
    use crate::error::FormatError;

    use super::validate;

    #[test]
    fn accepts_well_formed_toml() {
        assert!(validate(b"").is_ok());
        assert!(validate(b"# comment\n").is_ok());
        assert!(validate(b"key = 1\n").is_ok());
        assert!(validate(b"[table]\nkey = \"value\"\n").is_ok());
    }

    #[test]
    fn rejects_invalid_utf8() {
        assert!(matches!(validate(&[0xff, 0xfe]), Err(FormatError::Utf8(_))));
    }

    #[test]
    fn rejects_malformed_toml_with_typed_source() {
        assert!(matches!(
            validate(b"[invalid"),
            Err(FormatError::Toml(_))
        ));
    }
}
