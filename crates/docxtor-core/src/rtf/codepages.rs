//! Code pages, language tags and highlight colours: the tables and the functions that read them.


pub(super) const CP1252_HI: [u16; 32] = [
    0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0, 0x017D, 0, 0, 0x2018, 0x2019, 0x201C, 0x201D,
    0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
];

pub(super) const CP1250: [u16; 128] = [
    0x20AC, 0, 0x201A, 0, 0x201E, 0x2026, 0x2020, 0x2021, 0, 0x2030, 0x0160, 0x2039, 0x015A, 0x0164, 0x017D, 0x0179, //
    0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0, 0x2122, 0x0161, 0x203A, 0x015B, 0x0165, 0x017E, 0x017A, //
    0x00A0, 0x02C7, 0x02D8, 0x0141, 0x00A4, 0x0104, 0x00A6, 0x00A7, 0x00A8, 0x00A9, 0x015E, 0x00AB, 0x00AC, 0x00AD, 0x00AE, 0x017B, //
    0x00B0, 0x00B1, 0x02DB, 0x0142, 0x00B4, 0x00B5, 0x00B6, 0x00B7, 0x00B8, 0x0105, 0x015F, 0x00BB, 0x013D, 0x02DD, 0x013E, 0x017C, //
    0x0154, 0x00C1, 0x00C2, 0x0102, 0x00C4, 0x0139, 0x0106, 0x00C7, 0x010C, 0x00C9, 0x0118, 0x00CB, 0x011A, 0x00CD, 0x00CE, 0x010E, //
    0x0110, 0x0143, 0x0147, 0x00D3, 0x00D4, 0x0150, 0x00D6, 0x00D7, 0x0158, 0x016E, 0x00DA, 0x0170, 0x00DC, 0x00DD, 0x0162, 0x00DF, //
    0x0155, 0x00E1, 0x00E2, 0x0103, 0x00E4, 0x013A, 0x0107, 0x00E7, 0x010D, 0x00E9, 0x0119, 0x00EB, 0x011B, 0x00ED, 0x00EE, 0x010F, //
    0x0111, 0x0144, 0x0148, 0x00F3, 0x00F4, 0x0151, 0x00F6, 0x00F7, 0x0159, 0x016F, 0x00FA, 0x0171, 0x00FC, 0x00FD, 0x0163, 0x02D9,
];

/// Windows-1251 from 0x80 to 0xBF; 0xC0 and up are the Cyrillic letters in order.
pub(super) const CP1251_LO: [u16; 64] = [
    0x0402, 0x0403, 0x201A, 0x0453, 0x201E, 0x2026, 0x2020, 0x2021, 0x20AC, 0x2030, 0x0409, 0x2039, 0x040A, 0x040C, 0x040B, 0x040F, //
    0x0452, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0, 0x2122, 0x0459, 0x203A, 0x045A, 0x045C, 0x045B, 0x045F, //
    0x00A0, 0x040E, 0x045E, 0x0408, 0x00A4, 0x0490, 0x00A6, 0x00A7, 0x0401, 0x00A9, 0x0404, 0x00AB, 0x00AC, 0x00AD, 0x00AE, 0x0407, //
    0x00B0, 0x00B1, 0x0406, 0x0456, 0x0491, 0x00B5, 0x00B6, 0x00B7, 0x0451, 0x2116, 0x0454, 0x00BB, 0x0458, 0x0405, 0x0455, 0x0457,
];

/// Windows-1253 from 0x80 to 0xBF; the Greek letters above follow in order.
pub(super) const CP1253_LO: [u16; 64] = [
    0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0, 0x2030, 0, 0x2039, 0, 0, 0, 0, //
    0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0, 0x2122, 0, 0x203A, 0, 0, 0, 0, //
    0x00A0, 0x0385, 0x0386, 0x00A3, 0x00A4, 0x00A5, 0x00A6, 0x00A7, 0x00A8, 0x00A9, 0, 0x00AB, 0x00AC, 0x00AD, 0x00AE, 0x2015, //
    0x00B0, 0x00B1, 0x00B2, 0x00B3, 0x0384, 0x00B5, 0x00B6, 0x00B7, 0x0388, 0x0389, 0x038A, 0x00BB, 0x038C, 0x00BD, 0x038E, 0x038F,
];

/// A byte from `\'hh` as a character in the given Windows code page. Code pages that are not
/// implemented are read as Latin-1 (with the Windows-1252 punctuation in 0x80 to 0x9F).
pub(super) fn decode_byte(cp: u16, b: u8) -> char {
    if b < 0x80 {
        return b as char;
    }
    let i = (b - 0x80) as usize;
    let v: u16 = match cp {
        1250 => CP1250[i],
        1251 => {
            if b >= 0xC0 {
                0x0410 + (b - 0xC0) as u16
            } else {
                CP1251_LO[i]
            }
        }
        1253 => match b {
            0x80..=0xBF => CP1253_LO[i],
            0xC0..=0xD1 => 0x0390 + (b - 0xC0) as u16,
            0xD2 | 0xFF => 0,
            _ => 0x03A3 + (b - 0xD3) as u16,
        },
        1254 => match b {
            0xD0 => 0x011E,
            0xDD => 0x0130,
            0xDE => 0x015E,
            0xF0 => 0x011F,
            0xFD => 0x0131,
            0xFE => 0x015F,
            _ if b < 0xA0 => CP1252_HI[(b - 0x80) as usize],
            _ => b as u16,
        },
        _ => {
            if b < 0xA0 {
                CP1252_HI[(b - 0x80) as usize]
            } else {
                b as u16
            }
        }
    };
    if v == 0 {
        '\u{FFFD}'
    } else {
        char::from_u32(v as u32).unwrap_or('\u{FFFD}')
    }
}

/// The code page a font's `\fcharset` points to; `doc` is the document's `\ansicpg`.
pub(super) fn charset_cp(charset: i64, doc: u16) -> u16 {
    match charset {
        238 => 1250,
        204 => 1251,
        161 => 1253,
        162 => 1254,
        177 => 1255,
        178 => 1256,
        186 => 1257,
        163 => 1258,
        222 => 874,
        128 => 932,
        134 => 936,
        129 => 949,
        136 => 950,
        77 => 10000,
        _ => doc,
    }
}

/// The language tag for a Windows locale number (`\langN`), for the common languages.
pub(super) fn lang_tag(lcid: i64) -> Option<&'static str> {
    Some(match lcid {
        1033 => "en-US",
        2057 => "en-GB",
        3081 => "en-AU",
        4105 => "en-CA",
        3082 | 1034 => "es-ES",
        2058 => "es-MX",
        1036 => "fr-FR",
        3084 => "fr-CA",
        1031 => "de-DE",
        1040 => "it-IT",
        1046 => "pt-BR",
        2070 => "pt-PT",
        1043 => "nl-NL",
        1049 => "ru-RU",
        1041 => "ja-JP",
        2052 => "zh-CN",
        1042 => "ko-KR",
        1053 => "sv-SE",
        1045 => "pl-PL",
        _ => return None,
    })
}

/// Word's sixteen highlight colours.
pub(super) const HIGHLIGHTS: [(&str, (u8, u8, u8)); 16] = [
    ("yellow", (255, 255, 0)),
    ("green", (0, 255, 0)),
    ("cyan", (0, 255, 255)),
    ("magenta", (255, 0, 255)),
    ("blue", (0, 0, 255)),
    ("red", (255, 0, 0)),
    ("darkBlue", (0, 0, 128)),
    ("darkCyan", (0, 128, 128)),
    ("darkGreen", (0, 128, 0)),
    ("darkMagenta", (128, 0, 128)),
    ("darkRed", (128, 0, 0)),
    ("darkYellow", (128, 128, 0)),
    ("darkGray", (128, 128, 128)),
    ("lightGray", (192, 192, 192)),
    ("black", (0, 0, 0)),
    ("white", (255, 255, 255)),
];

pub(super) fn nearest_highlight(rgb: (u8, u8, u8)) -> Option<&'static str> {
    let dist = |a: (u8, u8, u8)| (a.0 as i32 - rgb.0 as i32).abs() + (a.1 as i32 - rgb.1 as i32).abs() + (a.2 as i32 - rgb.2 as i32).abs();
    HIGHLIGHTS.iter().map(|(n, c)| (*n, dist(*c))).min_by_key(|(_, d)| *d).filter(|(_, d)| *d <= 48).map(|(n, _)| n)
}

pub(super) fn hex_of(rgb: (u8, u8, u8)) -> String {
    format!("{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2)
}
