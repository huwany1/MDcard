// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Strict local text codecs. Detection is a suggestion, never a lossless guarantee.
use std::io;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
    #[default]
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Gbk,
    Gb18030,
    Big5,
    ShiftJis,
    EucKr,
    Windows1251,
    Windows1252,
}
impl Encoding {
    pub const ALL: [Self; 11] = [
        Self::Utf8,
        Self::Utf8Bom,
        Self::Utf16Le,
        Self::Utf16Be,
        Self::Gbk,
        Self::Gb18030,
        Self::Big5,
        Self::ShiftJis,
        Self::EucKr,
        Self::Windows1251,
        Self::Windows1252,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 BOM",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Gbk => "GBK",
            Self::Gb18030 => "GB18030",
            Self::Big5 => "Big5",
            Self::ShiftJis => "Shift_JIS",
            Self::EucKr => "EUC-KR",
            Self::Windows1251 => "Windows-1251",
            Self::Windows1252 => "Windows-1252",
        }
    }
    fn codec(self) -> &'static encoding_rs::Encoding {
        match self {
            Self::Utf8 | Self::Utf8Bom => encoding_rs::UTF_8,
            Self::Utf16Le => encoding_rs::UTF_16LE,
            Self::Utf16Be => encoding_rs::UTF_16BE,
            Self::Gbk => encoding_rs::GBK,
            Self::Gb18030 => encoding_rs::GB18030,
            Self::Big5 => encoding_rs::BIG5,
            Self::ShiftJis => encoding_rs::SHIFT_JIS,
            Self::EucKr => encoding_rs::EUC_KR,
            Self::Windows1251 => encoding_rs::WINDOWS_1251,
            Self::Windows1252 => encoding_rs::WINDOWS_1252,
        }
    }
}
pub const INVALID: &str =
    "The bytes are not valid in the selected encoding. Choose another encoding to reopen.";
pub const BINARY: &str =
    "The decoded file contains binary control characters. It cannot be opened as text.";
pub const UNREPRESENTABLE: &str = "This encoding cannot preserve every character. Choose UTF-8 or UTF-16; your edits have been kept.";
pub const BOM_CONFLICT: &str =
    "The selected encoding conflicts with the file BOM. Choose the matching Unicode encoding.";
pub const UTF32: &str = "UTF-32 is not supported. Convert the file to UTF-8 or UTF-16 first.";
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn bom(bytes: &[u8]) -> io::Result<Option<(Encoding, usize)>> {
    if bytes.starts_with(&[0xff, 0xfe, 0, 0]) || bytes.starts_with(&[0, 0, 0xfe, 0xff]) {
        return Err(invalid(UTF32));
    }
    Ok(if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        Some((Encoding::Utf8Bom, 3))
    } else if bytes.starts_with(&[0xff, 0xfe]) {
        Some((Encoding::Utf16Le, 2))
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        Some((Encoding::Utf16Be, 2))
    } else {
        None
    })
}
pub struct Decoded {
    pub text: String,
    pub encoding: Encoding,
    pub bom: bool,
    pub guessed: bool,
}
pub fn decode(bytes: &[u8], selected: Option<Encoding>) -> io::Result<Decoded> {
    let marker = bom(bytes)?;
    let guessed = selected.is_none() && marker.is_none() && std::str::from_utf8(bytes).is_err();
    let encoding = if let Some(selected) = selected {
        if let Some((actual, _)) = marker
            && selected.codec() != actual.codec()
        {
            return Err(invalid(BOM_CONFLICT));
        }
        marker.map_or(
            if selected == Encoding::Utf8Bom {
                Encoding::Utf8
            } else {
                selected
            },
            |(actual, _)| actual,
        )
    } else if let Some((encoding, _)) = marker {
        encoding
    } else if std::str::from_utf8(bytes).is_ok() {
        Encoding::Utf8
    } else {
        let mut detector = chardetng::EncodingDetector::new();
        detector.feed(bytes, true);
        let guessed_codec = detector.guess(None, true);
        let codec = if guessed_codec == encoding_rs::GBK {
            encoding_rs::GB18030
        } else {
            guessed_codec
        };
        Encoding::ALL
            .into_iter()
            .find(|e| e.codec() == codec)
            .ok_or_else(|| invalid(INVALID))?
    };
    let payload = &bytes[marker.map_or(0, |(_, length)| length)..];
    let text = if matches!(encoding, Encoding::Utf16Le | Encoding::Utf16Be) {
        if !payload.len().is_multiple_of(2) {
            return Err(invalid(INVALID));
        }
        let words = payload
            .chunks_exact(2)
            .map(|b| {
                if encoding == Encoding::Utf16Le {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                }
            })
            .collect::<Vec<_>>();
        String::from_utf16(&words).map_err(|_| invalid(INVALID))?
    } else {
        encoding
            .codec()
            .decode_without_bom_handling_and_without_replacement(payload)
            .ok_or_else(|| invalid(INVALID))?
            .into_owned()
    };
    if text
        .chars()
        .any(|c| c < ' ' && !['\t', '\n', '\r', '\u{c}'].contains(&c))
    {
        return Err(invalid(BINARY));
    }
    Ok(Decoded {
        text,
        encoding,
        bom: marker.is_some(),
        guessed,
    })
}
pub fn encode(text: &str, encoding: Encoding, bom: bool) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    match encoding {
        Encoding::Utf8 | Encoding::Utf8Bom => {
            if bom || encoding == Encoding::Utf8Bom {
                bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]);
            }
            bytes.extend_from_slice(text.as_bytes());
        }
        Encoding::Utf16Le | Encoding::Utf16Be => {
            if bom {
                bytes.extend_from_slice(if encoding == Encoding::Utf16Le {
                    &[0xff, 0xfe]
                } else {
                    &[0xfe, 0xff]
                });
            }
            for word in text.encode_utf16() {
                bytes.extend_from_slice(&if encoding == Encoding::Utf16Le {
                    word.to_le_bytes()
                } else {
                    word.to_be_bytes()
                });
            }
        }
        _ => {
            let (encoded, _, errors) = encoding.codec().encode(text);
            if errors {
                return Err(invalid(UNREPRESENTABLE));
            }
            bytes = encoded.into_owned();
            // Some legacy encoders map Unicode aliases to different decoded characters.
            if encoding
                .codec()
                .decode_without_bom_handling_and_without_replacement(&bytes)
                .is_none_or(|decoded| decoded != text)
            {
                return Err(invalid(UNREPRESENTABLE));
            }
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_codec_roundtrips_without_normalizing_newlines() {
        for (encoding, text) in [
            (Encoding::Utf8, "中文 🦀"),
            (Encoding::Utf8Bom, "Русский 🦀"),
            (Encoding::Utf16Le, "中文 🦀"),
            (Encoding::Utf16Be, "日本語 🦀"),
            (Encoding::Gbk, "简体中文测试"),
            (Encoding::Gb18030, "中文𠀀🦀"),
            (Encoding::Big5, "繁體中文測試"),
            (Encoding::ShiftJis, "日本語テスト"),
            (Encoding::EucKr, "한국어 테스트"),
            (Encoding::Windows1251, "Русский текст"),
            (Encoding::Windows1252, "café € résumé"),
        ] {
            let text = format!("{text}\r\nline\nlast\r");
            for marker in [false, true] {
                let bytes = encode(
                    &text,
                    encoding,
                    marker && matches!(encoding, Encoding::Utf16Le | Encoding::Utf16Be),
                )
                .unwrap();
                assert_eq!(
                    decode(&bytes, Some(encoding)).unwrap().text,
                    text,
                    "{encoding:?}"
                );
            }
        }
    }
    #[test]
    fn malformed_unicode_bom_conflicts_and_unmappable_characters_fail() {
        for bytes in [
            &[0xff, 0xfe, 0][..],
            &[0xff, 0xfe, 0, 0xd8],
            &[0, 0, 0xfe, 0xff],
            &[0xff, 0xfe, 0, 0],
        ] {
            assert!(decode(bytes, None).is_err());
        }
        assert!(decode(b"\xef\xbb\xbftext", Some(Encoding::Gbk)).is_err());
        assert!(decode(&[0x81], Some(Encoding::ShiftJis)).is_err());
        assert!(encode("🦀", Encoding::Windows1252, false).is_err());
        assert!(encode("¥", Encoding::ShiftJis, false).is_err());
        assert!(decode(b"text\0binary", None).is_err());
    }
}
