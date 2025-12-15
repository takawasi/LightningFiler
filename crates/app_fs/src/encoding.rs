//! Character encoding detection and conversion
//!
//! Handles legacy archive filenames encoded in various character sets.

use chardetng::EncodingDetector;
use encoding_rs::Encoding;

/// Hint for encoding detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingHint {
    /// Prefer Japanese encodings (Shift_JIS)
    Japanese,
    /// Prefer Chinese Simplified (GBK/GB18030)
    ChineseSimplified,
    /// Prefer Chinese Traditional (Big5)
    ChineseTraditional,
    /// Prefer Korean (EUC-KR)
    Korean,
    /// No preference
    None,
}

/// Detect the most likely encoding of a byte sequence
pub fn detect_encoding(bytes: &[u8], hint: EncodingHint) -> &'static Encoding {
    // First, check if it's valid UTF-8
    if std::str::from_utf8(bytes).is_ok() {
        return encoding_rs::UTF_8;
    }

    // Use chardetng for detection
    let mut detector = EncodingDetector::new();
    detector.feed(bytes, true);

    // Get the detected encoding with optional hint
    let encoding = match hint {
        EncodingHint::Japanese => {
            // Prefer Shift_JIS for Japanese
            let detected = detector.guess(Some(b"ja"), true);
            if detected == encoding_rs::WINDOWS_1252 {
                encoding_rs::SHIFT_JIS
            } else {
                detected
            }
        }
        EncodingHint::ChineseSimplified => {
            let detected = detector.guess(Some(b"zh-cn"), true);
            if detected == encoding_rs::WINDOWS_1252 {
                encoding_rs::GBK
            } else {
                detected
            }
        }
        EncodingHint::ChineseTraditional => {
            let detected = detector.guess(Some(b"zh-tw"), true);
            if detected == encoding_rs::WINDOWS_1252 {
                encoding_rs::BIG5
            } else {
                detected
            }
        }
        EncodingHint::Korean => {
            let detected = detector.guess(Some(b"ko"), true);
            if detected == encoding_rs::WINDOWS_1252 {
                encoding_rs::EUC_KR
            } else {
                detected
            }
        }
        EncodingHint::None => detector.guess(None, true),
    };

    encoding
}

/// Decode bytes to UTF-8 string
///
/// Returns the decoded string and a flag indicating if there were errors
pub fn decode_bytes(bytes: &[u8], hint: EncodingHint) -> (String, bool) {
    // First try UTF-8
    if let Ok(s) = std::str::from_utf8(bytes) {
        return (s.to_string(), false);
    }

    // Detect encoding
    let encoding = detect_encoding(bytes, hint);

    // Decode with replacement for invalid sequences
    let (result, _, had_errors) = encoding.decode(bytes);
    (result.into_owned(), had_errors)
}

/// Force decode bytes with a specific encoding
pub fn decode_with_encoding(bytes: &[u8], encoding_name: &str) -> Result<String, String> {
    let encoding = Encoding::for_label(encoding_name.as_bytes())
        .ok_or_else(|| format!("Unknown encoding: {}", encoding_name))?;

    let (result, _, had_errors) = encoding.decode(bytes);

    if had_errors {
        tracing::warn!("Decoding errors occurred with encoding {}", encoding_name);
    }

    Ok(result.into_owned())
}

/// Encode UTF-8 string to Shift_JIS bytes (for Susie plugin compatibility)
pub fn encode_to_shift_jis(s: &str) -> Result<Vec<u8>, String> {
    let (result, _, had_errors) = encoding_rs::SHIFT_JIS.encode(s);

    if had_errors {
        return Err(format!("Cannot encode '{}' to Shift_JIS", s));
    }

    Ok(result.into_owned())
}

/// Get the system default encoding hint based on locale
#[cfg(windows)]
pub fn system_encoding_hint() -> EncodingHint {
    use windows::Win32::Globalization::GetUserDefaultLCID;

    let lcid = unsafe { GetUserDefaultLCID() };

    // Extract primary language ID
    let primary_lang = lcid & 0x3FF;

    match primary_lang {
        0x11 => EncodingHint::Japanese,   // Japanese
        0x04 => EncodingHint::ChineseSimplified, // Chinese (could be simplified or traditional)
        0x12 => EncodingHint::Korean,     // Korean
        _ => EncodingHint::None,
    }
}

#[cfg(not(windows))]
pub fn system_encoding_hint() -> EncodingHint {
    // Check LANG environment variable
    std::env::var("LANG")
        .map(|lang| {
            let lang = lang.to_lowercase();
            if lang.contains("ja") {
                EncodingHint::Japanese
            } else if lang.contains("zh_cn") || lang.contains("zh-cn") {
                EncodingHint::ChineseSimplified
            } else if lang.contains("zh_tw") || lang.contains("zh-tw") {
                EncodingHint::ChineseTraditional
            } else if lang.contains("ko") {
                EncodingHint::Korean
            } else {
                EncodingHint::None
            }
        })
        .unwrap_or(EncodingHint::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_detection() {
        let bytes = "Hello, 世界!".as_bytes();
        let (decoded, had_errors) = decode_bytes(bytes, EncodingHint::None);
        assert_eq!(decoded, "Hello, 世界!");
        assert!(!had_errors);
    }

    #[test]
    fn test_shift_jis_detection() {
        // "テスト" in Shift_JIS
        let bytes = [0x83, 0x65, 0x83, 0x58, 0x83, 0x67];
        let (decoded, _) = decode_bytes(&bytes, EncodingHint::Japanese);
        assert_eq!(decoded, "テスト");
    }
}
