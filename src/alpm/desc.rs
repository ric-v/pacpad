//! Parser for pacman's `desc` block format, used identically by
//! `local/<pkg>/desc` and by the `desc` entries inside a sync repo's
//! gzipped tarball.
//!
//! Format (observed on-disk, not documented anywhere authoritative):
//!
//! ```text
//! %NAME%
//! ripgrep
//!
//! %DEPENDS%
//! glibc
//! libgcc
//! pcre2
//!
//! ```
//!
//! Each field is a `%FIELD%` header line followed by one or more value
//! lines, terminated by a blank line (or EOF). Multi-value fields
//! (`%DEPENDS%`, `%OPTDEPENDS%`, ...) are just multiple value lines --
//! there is no separator between them beyond the newline.

use std::collections::HashMap;

/// A parsed `desc` block: field name -> ordered list of value lines.
///
/// Deliberately untyped at this layer -- `local.rs` and `sync.rs` each
/// interpret the fields they care about into their own struct. This
/// keeps the on-disk format parser honest about what pacman actually
/// writes, rather than baking assumptions about which fields exist.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DescBlock {
    fields: HashMap<String, Vec<String>>,
}

impl DescBlock {
    pub fn parse(text: &str) -> Self {
        let mut fields: HashMap<String, Vec<String>> = HashMap::new();
        let mut current: Option<String> = None;

        for line in text.lines() {
            if let Some(name) = field_header(line) {
                current = Some(name.to_string());
                fields.entry(name.to_string()).or_default();
                continue;
            }
            if line.trim().is_empty() {
                current = None;
                continue;
            }
            if let Some(name) = &current {
                fields
                    .get_mut(name)
                    .expect("inserted above")
                    .push(line.to_string());
            }
            // A non-empty line with no active field (malformed input) is
            // silently dropped rather than panicking -- pacman's own
            // format has no escaping, so a corrupt or half-written db
            // entry should degrade to "missing field", not a crash.
        }

        DescBlock { fields }
    }

    /// The first value line for a field, if the field is present and non-empty.
    pub fn first(&self, field: &str) -> Option<&str> {
        self.fields
            .get(field)
            .and_then(|v| v.first())
            .map(|s| s.as_str())
    }

    /// All value lines for a field, in file order. Empty slice if absent.
    pub fn all(&self, field: &str) -> &[String] {
        self.fields.get(field).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Whether the field header appeared at all (even with zero values --
    /// this matters for `%REASON%`, whose mere presence is the signal).
    pub fn has(&self, field: &str) -> bool {
        self.fields.contains_key(field)
    }

    pub fn first_owned(&self, field: &str) -> Option<String> {
        self.first(field).map(|s| s.to_string())
    }

    pub fn all_owned(&self, field: &str) -> Vec<String> {
        self.all(field).to_vec()
    }

    pub fn first_u64(&self, field: &str) -> Option<u64> {
        self.first(field).and_then(|s| s.parse().ok())
    }

    pub fn first_i64(&self, field: &str) -> Option<i64> {
        self.first(field).and_then(|s| s.parse().ok())
    }
}

/// Recognizes a `%FIELD_NAME%` header line and returns the bare name.
fn field_header(line: &str) -> Option<&str> {
    let line = line.trim_end();
    let inner = line.strip_prefix('%')?.strip_suffix('%')?;
    if inner.is_empty() {
        return None;
    }
    if inner.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
        Some(inner)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_and_multi_value_fields() {
        let text = "%NAME%\nripgrep\n\n%DEPENDS%\nglibc\nlibgcc\npcre2\n\n";
        let d = DescBlock::parse(text);
        assert_eq!(d.first("NAME"), Some("ripgrep"));
        assert_eq!(
            d.all("DEPENDS"),
            &[
                "glibc".to_string(),
                "libgcc".to_string(),
                "pcre2".to_string()
            ]
        );
    }

    #[test]
    fn missing_field_is_absent_not_empty_vec() {
        let d = DescBlock::parse("%NAME%\nfoo\n\n");
        assert_eq!(d.all("OPTDEPENDS"), &[] as &[String]);
        assert!(!d.has("OPTDEPENDS"));
        assert_eq!(d.first("OPTDEPENDS"), None);
    }

    #[test]
    fn reason_presence_is_the_signal_not_its_value() {
        // %REASON% absent => explicit; present (value "1") => dependency.
        let explicit = DescBlock::parse("%NAME%\nripgrep\n\n");
        let dependency = DescBlock::parse("%NAME%\nhtop\n\n%REASON%\n1\n\n");
        assert!(!explicit.has("REASON"));
        assert!(dependency.has("REASON"));
        assert_eq!(dependency.first("REASON"), Some("1"));
    }

    #[test]
    fn handles_no_trailing_blank_line_at_eof() {
        let d = DescBlock::parse("%NAME%\nfoo\n\n%VERSION%\n1.0-1");
        assert_eq!(d.first("VERSION"), Some("1.0-1"));
    }

    #[test]
    fn utf8_description_survives() {
        let d = DescBlock::parse("%DESC%\nRecursively searches — em dash, café, 日本語\n\n");
        assert_eq!(
            d.first("DESC"),
            Some("Recursively searches — em dash, café, 日本語")
        );
    }

    #[test]
    fn multiline_license_is_multi_value() {
        // fd's real sync desc has MIT + Apache-2.0 on separate lines.
        let d = DescBlock::parse("%LICENSE%\nMIT\nApache-2.0\n\n");
        assert_eq!(
            d.all("LICENSE"),
            &["MIT".to_string(), "Apache-2.0".to_string()]
        );
    }
}

#[cfg(test)]
mod numeric_tests {
    use super::*;

    #[test]
    fn parses_numeric_fields_and_tolerates_garbage() {
        let d = DescBlock::parse(
            "%SIZE%\n3742597\n\n%INSTALLDATE%\n1788972672\n\n%BOGUS%\nnot-a-number\n\n",
        );
        assert_eq!(d.first_u64("SIZE"), Some(3742597));
        assert_eq!(d.first_i64("INSTALLDATE"), Some(1788972672));
        assert_eq!(d.first_u64("BOGUS"), None);
        assert_eq!(d.first_u64("MISSING"), None);
    }
}
