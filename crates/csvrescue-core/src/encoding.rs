//! Encoding detection, strict decoding, mojibake repair, and output encoding.

use crate::errors::{Error, Result};
use serde::{Deserialize, Serialize};

/// Canonical encoding names offered in the UI and accepted by [`decode`].
pub const ENCODING_CHOICES: &[&str] = &[
    "UTF-8",
    "UTF-16 LE",
    "UTF-16 BE",
    "UTF-32 LE",
    "UTF-32 BE",
    "Windows-1252 (Western Europe)",
    "ISO-8859-1 (Latin 1)",
    "ISO-8859-15 (Latin 9)",
    "Windows-1250 (Central Europe)",
    "ISO-8859-2 (Central Europe)",
    "Windows-1251 (Cyrillic)",
    "Windows-1253 (Greek)",
    "Windows-1254 (Turkish)",
    "Windows-1256 (Arabic)",
    "Windows-1257 (Baltic)",
    "Shift-JIS (Japanese)",
    "GBK (Simplified Chinese)",
    "Big5 (Traditional Chinese)",
    "EUC-KR (Korean)",
    "IBM437 (old DOS)",
];

/// Output text encoding options for CSV exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum OutputEncoding {
    #[default]
    Utf8,
    Utf16LeBom,
    Windows1252,
}

impl OutputEncoding {
    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf16LeBom => "UTF-16 LE (with BOM)",
            Self::Windows1252 => "Windows-1252 (Western Europe)",
        }
    }
}

/// Record of a single double-encoded UTF-8 mojibake repair applied to a text span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MojibakeRepair {
    pub byte_offset: usize,
    pub original: String,
    pub repaired: String,
}

/// Result of decoding raw input bytes into Unicode text without altering the source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Decoded {
    pub text: String,
    pub encoding_name: String,
    pub had_bom: bool,
    pub note: String,
    pub invalid_sequences: usize,
    pub was_lossy: bool,
}

/// Windows-1252 code points for bytes `0x80..=0x9F`. `U+FFFD` marks the 5 undefined slots.
const WIN1252_HIGH: [char; 32] = [
    '€', '\u{FFFD}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{FFFD}', 'Ž',
    '\u{FFFD}', '\u{FFFD}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{FFFD}',
    'ž', 'Ÿ',
];

/// Full 256-entry IBM437 (DOS US) codepage table (`0x80..=0xFF` high half).
const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ',
    'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ',
    'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕',
    '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦',
    '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐',
    '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±',
    '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{00A0}',
];

/// Decodes raw file bytes using either automatic detection (`forced = None` or `Some("Auto")`)
/// or an explicit encoding name from [`ENCODING_CHOICES`].
pub fn decode(data: &[u8], forced: Option<&str>) -> Result<Decoded> {
    if let Some(choice) = forced.filter(|s| !s.is_empty() && *s != "Auto") {
        if !ENCODING_CHOICES.contains(&choice) {
            return Err(Error::EncodingError(format!(
                "Unsupported encoding '{choice}'."
            )));
        }
        return Ok(decode_explicit(data, choice, "Chosen by you."));
    }

    // 1. Explicit Byte Order Marks (order matters: UTF-32 LE starts with FF FE 00 00, same prefix as UTF-16 LE)
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok(decode_utf8_lenient(
            &data[3..],
            "UTF-8",
            true,
            "UTF-8 with a byte order mark.",
        ));
    }
    if data.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        return Ok(decode_utf32(
            &data[4..],
            true,
            true,
            "UTF-32 with a byte order mark.",
        ));
    }
    if data.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        return Ok(decode_utf32(
            &data[4..],
            false,
            true,
            "UTF-32 BE with a byte order mark.",
        ));
    }
    if data.starts_with(&[0xFF, 0xFE]) {
        return Ok(decode_utf16(
            &data[2..],
            true,
            true,
            "UTF-16 with a byte order mark.",
        ));
    }
    if data.starts_with(&[0xFE, 0xFF]) {
        return Ok(decode_utf16(
            &data[2..],
            false,
            true,
            "UTF-16 with a byte order mark.",
        ));
    }

    // 2. BOM-less UTF-16 null-byte ratio heuristic on the first 64 KB
    let probe = data.len().min(65_536);
    if probe >= 4 {
        let mut zeros_even = 0usize;
        let mut zeros_odd = 0usize;
        for (i, &b) in data[..probe].iter().enumerate() {
            if b == 0 {
                if i % 2 == 0 {
                    zeros_even += 1;
                } else {
                    zeros_odd += 1;
                }
            }
        }
        if zeros_odd > probe / 8 && zeros_odd > zeros_even * 4 {
            return Ok(decode_utf16(
                data,
                true,
                false,
                "UTF-16 without a byte order mark (most bytes are zero).",
            ));
        }
        if zeros_even > probe / 8 && zeros_even > zeros_odd * 4 {
            return Ok(decode_utf16(
                data,
                false,
                false,
                "UTF-16 without a byte order mark (most bytes are zero).",
            ));
        }
    }

    // 3. Strict UTF-8 / pure ASCII
    if let Ok(text) = std::str::from_utf8(data) {
        let is_ascii = data.is_ascii();
        let note = if is_ascii {
            "Plain ASCII text (valid as UTF-8 and as every Western encoding)."
        } else {
            "Valid UTF-8."
        };
        return Ok(Decoded {
            text: text.to_owned(),
            encoding_name: "UTF-8".to_owned(),
            had_bom: false,
            note: note.to_owned(),
            invalid_sequences: 0,
            was_lossy: false,
        });
    }

    // 4. Fallback to Windows-1252 when bytes are not valid UTF-8
    let mut win1252 = decode_windows_1252(data);
    let utf8_errors = count_utf8_invalid_sequences(data);
    win1252.encoding_name = "Windows-1252 (Western Europe)".to_owned();
    win1252.note = format!(
        "Not valid UTF-8 ({utf8_errors} invalid UTF-8 byte sequence{}), so the old Windows Western encoding is assumed. Pick another encoding if letters look wrong.",
        if utf8_errors == 1 { "" } else { "s" }
    );
    Ok(win1252)
}

fn decode_explicit(data: &[u8], name: &str, note: &str) -> Decoded {
    match name {
        "UTF-8" => {
            let (slice, bom) = if let Some(rest) = data.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
                (rest, true)
            } else {
                (data, false)
            };
            decode_utf8_lenient(slice, name, bom, note)
        }
        "UTF-16 LE" => {
            let (slice, bom) = if let Some(rest) = data.strip_prefix(&[0xFF, 0xFE]) {
                (rest, true)
            } else {
                (data, false)
            };
            decode_utf16(slice, true, bom, note)
        }
        "UTF-16 BE" => {
            let (slice, bom) = if let Some(rest) = data.strip_prefix(&[0xFE, 0xFF]) {
                (rest, true)
            } else {
                (data, false)
            };
            decode_utf16(slice, false, bom, note)
        }
        "UTF-32 LE" => {
            let (slice, bom) = if let Some(rest) = data.strip_prefix(&[0xFF, 0xFE, 0x00, 0x00]) {
                (rest, true)
            } else {
                (data, false)
            };
            decode_utf32(slice, true, bom, note)
        }
        "UTF-32 BE" => {
            let (slice, bom) = if let Some(rest) = data.strip_prefix(&[0x00, 0x00, 0xFE, 0xFF]) {
                (rest, true)
            } else {
                (data, false)
            };
            decode_utf32(slice, false, bom, note)
        }
        "Windows-1252 (Western Europe)" => {
            let mut d = decode_windows_1252(data);
            d.encoding_name = name.to_owned();
            d.note = note.to_owned();
            d
        }
        "ISO-8859-1 (Latin 1)" => Decoded {
            text: data.iter().map(|&b| b as char).collect(),
            encoding_name: name.to_owned(),
            had_bom: false,
            note: note.to_owned(),
            invalid_sequences: 0,
            was_lossy: false,
        },
        "IBM437 (old DOS)" => {
            let text: String = data
                .iter()
                .map(|&b| {
                    if b < 0x80 {
                        b as char
                    } else {
                        CP437_HIGH[(b - 0x80) as usize]
                    }
                })
                .collect();
            Decoded {
                text,
                encoding_name: name.to_owned(),
                had_bom: false,
                note: note.to_owned(),
                invalid_sequences: 0,
                was_lossy: false,
            }
        }
        _ => {
            let enc = match name {
                "ISO-8859-15 (Latin 9)" => encoding_rs::ISO_8859_15,
                "Windows-1250 (Central Europe)" => encoding_rs::WINDOWS_1250,
                "ISO-8859-2 (Central Europe)" => encoding_rs::ISO_8859_2,
                "Windows-1251 (Cyrillic)" => encoding_rs::WINDOWS_1251,
                "Windows-1253 (Greek)" => encoding_rs::WINDOWS_1253,
                "Windows-1254 (Turkish)" => encoding_rs::WINDOWS_1254,
                "Windows-1256 (Arabic)" => encoding_rs::WINDOWS_1256,
                "Windows-1257 (Baltic)" => encoding_rs::WINDOWS_1257,
                "Shift-JIS (Japanese)" => encoding_rs::SHIFT_JIS,
                "GBK (Simplified Chinese)" => encoding_rs::GBK,
                "Big5 (Traditional Chinese)" => encoding_rs::BIG5,
                "EUC-KR (Korean)" => encoding_rs::EUC_KR,
                _ => encoding_rs::WINDOWS_1252,
            };
            let (cow, had_errors) = enc.decode_without_bom_handling(data);
            let invalid = if had_errors {
                cow.matches('\u{FFFD}').count().max(1)
            } else {
                0
            };
            let full_note = if had_errors {
                format!("{note} ({invalid} unmapped byte sequence(s) replaced with U+FFFD)")
            } else {
                note.to_owned()
            };
            Decoded {
                text: cow.into_owned(),
                encoding_name: name.to_owned(),
                had_bom: false,
                note: full_note,
                invalid_sequences: invalid,
                was_lossy: had_errors,
            }
        }
    }
}

fn decode_utf8_lenient(data: &[u8], name: &str, bom: bool, note: &str) -> Decoded {
    match std::str::from_utf8(data) {
        Ok(s) => Decoded {
            text: s.to_owned(),
            encoding_name: name.to_owned(),
            had_bom: bom,
            note: note.to_owned(),
            invalid_sequences: 0,
            was_lossy: false,
        },
        Err(_) => {
            let mut out = String::with_capacity(data.len());
            let mut invalid = 0usize;
            let mut rest = data;
            while !rest.is_empty() {
                match std::str::from_utf8(rest) {
                    Ok(valid) => {
                        out.push_str(valid);
                        break;
                    }
                    Err(err) => {
                        let valid_len = err.valid_up_to();
                        // SAFETY: `valid_up_to()` guarantees valid UTF-8 prefix.
                        let prefix = unsafe { std::str::from_utf8_unchecked(&rest[..valid_len]) };
                        out.push_str(prefix);
                        out.push('\u{FFFD}');
                        invalid += 1;
                        let skip = err.error_len().unwrap_or(rest.len() - valid_len);
                        rest = &rest[valid_len + skip..];
                    }
                }
            }
            Decoded {
                text: out,
                encoding_name: name.to_owned(),
                had_bom: bom,
                note: format!("{note} ({invalid} invalid UTF-8 byte sequence(s) replaced)"),
                invalid_sequences: invalid,
                was_lossy: true,
            }
        }
    }
}

fn count_utf8_invalid_sequences(data: &[u8]) -> usize {
    let mut count = 0usize;
    let mut rest = data;
    while !rest.is_empty() {
        match std::str::from_utf8(rest) {
            Ok(_) => break,
            Err(err) => {
                count += 1;
                let skip = err.error_len().unwrap_or(rest.len() - err.valid_up_to());
                rest = &rest[err.valid_up_to() + skip..];
            }
        }
    }
    count
}

fn decode_utf16(data: &[u8], little_endian: bool, bom: bool, note: &str) -> Decoded {
    let name = if little_endian {
        "UTF-16 LE"
    } else {
        "UTF-16 BE"
    };
    let units = data.chunks_exact(2).map(|pair| {
        if little_endian {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        }
    });
    let mut out = String::with_capacity(data.len() / 2);
    let mut invalid = 0usize;
    for res in std::char::decode_utf16(units) {
        match res {
            Ok(ch) => out.push(ch),
            Err(_) => {
                out.push('\u{FFFD}');
                invalid += 1;
            }
        }
    }
    if data.len() % 2 != 0 {
        out.push('\u{FFFD}');
        invalid += 1;
    }
    let full_note = if invalid > 0 {
        format!("{note} ({invalid} invalid UTF-16 sequence(s))")
    } else {
        note.to_owned()
    };
    Decoded {
        text: out,
        encoding_name: name.to_owned(),
        had_bom: bom,
        note: full_note,
        invalid_sequences: invalid,
        was_lossy: invalid > 0,
    }
}

fn decode_utf32(data: &[u8], little_endian: bool, bom: bool, note: &str) -> Decoded {
    let name = if little_endian {
        "UTF-32 LE"
    } else {
        "UTF-32 BE"
    };
    let mut out = String::with_capacity(data.len() / 4);
    let mut invalid = 0usize;
    for quad in data.chunks_exact(4) {
        let code = if little_endian {
            u32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]])
        } else {
            u32::from_be_bytes([quad[0], quad[1], quad[2], quad[3]])
        };
        match char::from_u32(code) {
            Some(ch) => out.push(ch),
            None => {
                out.push('\u{FFFD}');
                invalid += 1;
            }
        }
    }
    if data.len() % 4 != 0 {
        out.push('\u{FFFD}');
        invalid += 1;
    }
    Decoded {
        text: out,
        encoding_name: name.to_owned(),
        had_bom: bom,
        note: if invalid > 0 {
            format!("{note} ({invalid} invalid UTF-32 sequence(s))")
        } else {
            note.to_owned()
        },
        invalid_sequences: invalid,
        was_lossy: invalid > 0,
    }
}

fn decode_windows_1252(data: &[u8]) -> Decoded {
    let mut out = String::with_capacity(data.len());
    let mut invalid = 0usize;
    for &b in data {
        let ch = if !(0x80..0xA0).contains(&b) {
            b as char
        } else {
            WIN1252_HIGH[(b - 0x80) as usize]
        };
        if ch == '\u{FFFD}' {
            invalid += 1;
        }
        out.push(ch);
    }
    Decoded {
        text: out,
        encoding_name: "Windows-1252 (Western Europe)".to_owned(),
        had_bom: false,
        note: String::new(),
        invalid_sequences: invalid,
        was_lossy: invalid > 0,
    }
}

/// Maps a Unicode scalar back to its Windows-1252 byte, if representable.
pub fn char_to_win1252(ch: char) -> Option<u8> {
    let cp = ch as u32;
    if cp < 0x80 || (0xA0..=0xFF).contains(&cp) {
        return Some(cp as u8);
    }
    WIN1252_HIGH
        .iter()
        .position(|&mapped| mapped != '\u{FFFD}' && mapped == ch)
        .map(|idx| (0x80 + idx) as u8)
}

fn is_cp1252_continuation_char(ch: char) -> bool {
    let cp = ch as u32;
    if (0x0080..=0x00BF).contains(&cp) || (0x00C0..=0x00FF).contains(&cp) {
        return true;
    }
    WIN1252_HIGH
        .iter()
        .any(|&mapped| mapped != '\u{FFFD}' && mapped == ch)
}

fn is_mojibake_lead(ch: char) -> bool {
    matches!(ch, 'Ã' | 'Â' | 'â' | 'ð')
}

/// Counts double-encoded UTF-8 marker sequences (e.g. `Ã©`, `â€™`).
pub fn mojibake_count(text: &str) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut count = 0usize;
    let mut i = 0usize;
    while i + 1 < chars.len() {
        let c0 = chars[i];
        let c1 = chars[i + 1];
        if (c0 == 'Ã' || c0 == 'Â') && is_cp1252_continuation_char(c1) {
            if let (Some(b0), Some(b1)) = (char_to_win1252(c0), char_to_win1252(c1)) {
                if std::str::from_utf8(&[b0, b1]).is_ok() {
                    count += 1;
                    i += 2;
                    continue;
                }
            }
        }
        if c0 == 'â' && i + 2 < chars.len() {
            let c2 = chars[i + 2];
            if is_cp1252_continuation_char(c1) && is_cp1252_continuation_char(c2) {
                if let (Some(b0), Some(b1), Some(b2)) = (
                    char_to_win1252(c0),
                    char_to_win1252(c1),
                    char_to_win1252(c2),
                ) {
                    if std::str::from_utf8(&[b0, b1, b2]).is_ok() {
                        count += 1;
                        i += 3;
                        continue;
                    }
                }
            }
        }
        if c0 == 'ð' && c1 == 'Ÿ' && i + 3 < chars.len() {
            if let (Some(b0), Some(b1), Some(b2), Some(b3)) = (
                char_to_win1252(chars[i]),
                char_to_win1252(chars[i + 1]),
                char_to_win1252(chars[i + 2]),
                char_to_win1252(chars[i + 3]),
            ) {
                if std::str::from_utf8(&[b0, b1, b2, b3]).is_ok() {
                    count += 1;
                    i += 4;
                    continue;
                }
            }
        }
        i += 1;
    }
    count
}

/// Repairs text that was saved as UTF-8, misread as Windows-1252, and saved as UTF-8 again.
///
/// Leaves any text untouched when reversing the CP1252 -> UTF-8 transformation is not strictly valid,
/// so clean text containing letters like `Ärger` or stray `Ã` is never damaged.
pub fn repair_double_encoding_detailed(text: &str) -> (String, Vec<MojibakeRepair>) {
    if mojibake_count(text) == 0 {
        return (text.to_owned(), Vec::new());
    }

    let mut out = String::with_capacity(text.len());
    let mut repairs = Vec::new();
    let indices: Vec<(usize, char)> = text.char_indices().collect();
    let mut idx = 0usize;

    while idx < indices.len() {
        let (start_byte, ch) = indices[idx];
        if is_mojibake_lead(ch) {
            let mut end_idx = idx;
            let mut raw_bytes = Vec::new();
            while end_idx < indices.len() {
                let c = indices[end_idx].1;
                if end_idx == idx || is_cp1252_continuation_char(c) || is_mojibake_lead(c) {
                    if let Some(b) = char_to_win1252(c) {
                        if b >= 0x80 {
                            raw_bytes.push(b);
                            end_idx += 1;
                            continue;
                        }
                    }
                }
                break;
            }
            if raw_bytes.len() >= 2 {
                if let Ok(fixed) = std::str::from_utf8(&raw_bytes) {
                    let end_byte = if end_idx < indices.len() {
                        indices[end_idx].0
                    } else {
                        text.len()
                    };
                    let orig = &text[start_byte..end_byte];
                    if fixed != orig {
                        repairs.push(MojibakeRepair {
                            byte_offset: start_byte,
                            original: orig.to_owned(),
                            repaired: fixed.to_owned(),
                        });
                        out.push_str(fixed);
                        idx = end_idx;
                        continue;
                    }
                }
            }
        }
        out.push(ch);
        idx += 1;
    }

    (out, repairs)
}

/// Convenience wrapper returning only the repaired string.
pub fn repair_double_encoding(text: &str) -> String {
    repair_double_encoding_detailed(text).0
}

/// Encodes text into the selected output encoding, returning `(bytes, lossy_char_count)`.
pub fn encode_output(text: &str, encoding: OutputEncoding, bom: bool) -> (Vec<u8>, usize) {
    match encoding {
        OutputEncoding::Utf8 => {
            let mut bytes = Vec::with_capacity(text.len() + if bom { 3 } else { 0 });
            if bom {
                bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            bytes.extend_from_slice(text.as_bytes());
            (bytes, 0)
        }
        OutputEncoding::Utf16LeBom => {
            let mut bytes = Vec::with_capacity(text.len() * 2 + 2);
            bytes.extend_from_slice(&[0xFF, 0xFE]);
            for unit in text.encode_utf16() {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
            (bytes, 0)
        }
        OutputEncoding::Windows1252 => {
            let mut bytes = Vec::with_capacity(text.len());
            let mut lossy = 0usize;
            for ch in text.chars() {
                if let Some(b) = char_to_win1252(ch) {
                    bytes.push(b);
                } else {
                    bytes.push(b'?');
                    lossy += 1;
                }
            }
            (bytes, lossy)
        }
    }
}
