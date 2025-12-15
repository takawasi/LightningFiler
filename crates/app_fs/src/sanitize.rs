//! Filename sanitization for Windows compatibility

/// Mode for sanitizing filenames
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SanitizeMode {
    /// Replace forbidden characters with full-width equivalents (default)
    FullWidth,
    /// Replace forbidden characters with underscores
    Underscore,
    /// URL-encode forbidden characters (%XX)
    UrlEncode,
}

/// Windows reserved filenames
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL",
    "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Windows forbidden characters
const FORBIDDEN_CHARS: &[(char, char)] = &[
    ('\\', '＼'), // U+FF3C
    ('/', '／'),  // U+FF0F
    (':', '：'),  // U+FF1A
    ('*', '＊'),  // U+FF0A
    ('?', '？'),  // U+FF1F
    ('"', '＂'),  // U+FF02
    ('<', '＜'),  // U+FF1C
    ('>', '＞'),  // U+FF1E
    ('|', '｜'),  // U+FF5C
];

/// Sanitize a filename for Windows compatibility
pub fn sanitize_filename(name: &str, mode: SanitizeMode) -> String {
    let mut result = String::with_capacity(name.len());

    // Replace forbidden characters
    for c in name.chars() {
        if let Some(replacement) = get_replacement(c, mode) {
            result.push_str(&replacement);
        } else {
            result.push(c);
        }
    }

    // Handle reserved names
    let name_upper = result.to_uppercase();
    let base_name = name_upper.split('.').next().unwrap_or("");

    if RESERVED_NAMES.contains(&base_name) {
        result = format!("_{}", result);
    }

    // Handle trailing dots and spaces (Windows removes them)
    while result.ends_with('.') || result.ends_with(' ') {
        result.pop();
    }

    // Handle empty result
    if result.is_empty() {
        result = "_unnamed".to_string();
    }

    result
}

fn get_replacement(c: char, mode: SanitizeMode) -> Option<String> {
    for (forbidden, fullwidth) in FORBIDDEN_CHARS {
        if c == *forbidden {
            return Some(match mode {
                SanitizeMode::FullWidth => fullwidth.to_string(),
                SanitizeMode::Underscore => "_".to_string(),
                SanitizeMode::UrlEncode => format!("%{:02X}", c as u32),
            });
        }
    }

    // Control characters (0x00-0x1F)
    if c.is_control() {
        return Some(match mode {
            SanitizeMode::FullWidth | SanitizeMode::Underscore => "_".to_string(),
            SanitizeMode::UrlEncode => format!("%{:02X}", c as u32),
        });
    }

    None
}

/// Check if a filename is valid for Windows
pub fn is_valid_filename(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    // Check for forbidden characters
    for c in name.chars() {
        if FORBIDDEN_CHARS.iter().any(|(f, _)| *f == c) {
            return false;
        }
        if c.is_control() {
            return false;
        }
    }

    // Check for reserved names
    let name_upper = name.to_uppercase();
    let base_name = name_upper.split('.').next().unwrap_or("");
    if RESERVED_NAMES.contains(&base_name) {
        return false;
    }

    // Check for trailing dots/spaces
    if name.ends_with('.') || name.ends_with(' ') {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_colon() {
        assert_eq!(
            sanitize_filename("image:01.jpg", SanitizeMode::FullWidth),
            "image：01.jpg"
        );
        assert_eq!(
            sanitize_filename("image:01.jpg", SanitizeMode::Underscore),
            "image_01.jpg"
        );
    }

    #[test]
    fn test_sanitize_reserved() {
        assert_eq!(
            sanitize_filename("CON.txt", SanitizeMode::FullWidth),
            "_CON.txt"
        );
        assert_eq!(
            sanitize_filename("aux", SanitizeMode::FullWidth),
            "_aux"
        );
    }

    #[test]
    fn test_sanitize_trailing() {
        assert_eq!(
            sanitize_filename("test.", SanitizeMode::FullWidth),
            "test"
        );
        assert_eq!(
            sanitize_filename("test ", SanitizeMode::FullWidth),
            "test"
        );
    }

    #[test]
    fn test_is_valid() {
        assert!(is_valid_filename("normal.txt"));
        assert!(!is_valid_filename("test:file.txt"));
        assert!(!is_valid_filename("CON"));
        assert!(!is_valid_filename("test."));
    }
}
