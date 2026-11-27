//! Number formats for list labels and page numbers (`w:numFmt`): every format Word writes.
//!
//! `format(n, fmt, lang)` turns a count into its label text. Formats that spell the number out in words
//! (`cardinalText`, `ordinalText`) follow the language when Word does (English, Spanish, French and German
//! here) and are English otherwise. Everything else does not depend on the language.

/// The label text of count `n` in number format `fmt`. `lang` is a BCP 47 tag (`en-US`, `es`, ...).
pub fn format(n: i64, fmt: &str, lang: Option<&str>) -> String {
    let lang = lang.map(|l| l.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase());
    let lang = lang.as_deref().unwrap_or("en");
    match fmt {
        "decimal" | "custom" => n.to_string(),
        "decimalZero" => {
            if (0..10).contains(&n) {
                format!("{n:02}")
            } else {
                n.to_string()
            }
        }
        "upperRoman" => roman(n),
        "lowerRoman" => roman(n).to_lowercase(),
        "upperLetter" => repeated_letters(n, b'A'),
        "lowerLetter" => repeated_letters(n, b'a'),
        "ordinal" => ordinal(n, lang),
        "cardinalText" => capitalise(&cardinal_words(n, lang).unwrap_or_else(|| n.to_string())),
        "ordinalText" => capitalise(&ordinal_words(n, lang).unwrap_or_else(|| ordinal(n, lang))),
        "hex" => format!("{n:X}"),
        "chicago" => chicago(n),
        "numberInDash" => format!("- {n} -"),
        "none" | "bullet" => String::new(),
        "decimalEnclosedCircle" | "decimalEnclosedCircleChinese" => enclosed_circle(n),
        "decimalEnclosedFullstop" => match n {
            1..=20 => cp(0x2488 + (n as u32 - 1)),
            _ => format!("{n}."),
        },
        "decimalEnclosedParen" => match n {
            1..=20 => cp(0x2474 + (n as u32 - 1)),
            _ => format!("({n})"),
        },
        "decimalFullWidth" | "decimalFullWidth2" => n.to_string().chars().map(|c| if c.is_ascii_digit() { cp(0xFF10 + (c as u32 - '0' as u32)) } else { c.to_string() }).collect(),
        "decimalHalfWidth" => n.to_string(),
        // Japanese
        "japaneseCounting" | "japaneseDigitalTenThousand" => cjk(n, &CJK_JAPANESE, true),
        "japaneseLegal" => cjk(n, &CJK_JAPANESE_LEGAL, true),
        "aiueo" => alphabet(n, &AIUEO_HALF),
        "aiueoFullWidth" => alphabet(n, &AIUEO_FULL),
        "iroha" => alphabet(n, &IROHA_HALF),
        "irohaFullWidth" => alphabet(n, &IROHA_FULL),
        // Chinese
        "chineseCounting" | "chineseCountingThousand" => cjk(n, &CJK_CHINESE, false),
        "taiwaneseCounting" | "taiwaneseCountingThousand" => cjk(n, &CJK_TAIWANESE, false),
        "chineseLegalSimplified" => cjk(n, &CJK_LEGAL_SIMPLIFIED, false),
        "ideographLegalTraditional" => cjk(n, &CJK_LEGAL_TRADITIONAL, false),
        "ideographDigital" | "taiwaneseDigital" => digits_in(n, &["〇", "一", "二", "三", "四", "五", "六", "七", "八", "九"]),
        "ideographTraditional" => alphabet(n, &["甲", "乙", "丙", "丁", "戊", "己", "庚", "辛", "壬", "癸"]),
        "ideographZodiac" => cycle(n, &["子", "丑", "寅", "卯", "辰", "巳", "午", "未", "申", "酉", "戌", "亥"]),
        "ideographZodiacTraditional" => {
            let stems = ["甲", "乙", "丙", "丁", "戊", "己", "庚", "辛", "壬", "癸"];
            let branches = ["子", "丑", "寅", "卯", "辰", "巳", "午", "未", "申", "酉", "戌", "亥"];
            if n <= 0 {
                return n.to_string();
            }
            let i = ((n - 1) % 60) as usize;
            format!("{}{}", stems[i % 10], branches[i % 12])
        }
        "ideographEnclosedCircle" => match n {
            1..=10 => cp(0x3280 + (n as u32 - 1)),
            _ => n.to_string(),
        },
        // Korean
        "koreanDigital" => digits_in(n, &["영", "일", "이", "삼", "사", "오", "육", "칠", "팔", "구"]),
        "koreanDigital2" => digits_in(n, &["零", "一", "二", "三", "四", "五", "六", "七", "八", "九"]),
        "koreanCounting" => cjk(n, &CJK_KOREAN, true),
        "ganada" => alphabet(n, &["가", "나", "다", "라", "마", "바", "사", "아", "자", "차", "카", "타", "파", "하"]),
        "chosung" => alphabet(n, &["ㄱ", "ㄴ", "ㄷ", "ㄹ", "ㅁ", "ㅂ", "ㅅ", "ㅇ", "ㅈ", "ㅊ", "ㅋ", "ㅌ", "ㅍ", "ㅎ"]),
        // Hebrew and Arabic
        "hebrew1" => hebrew_numeral(n),
        "hebrew2" => alphabet(n, &["א", "ב", "ג", "ד", "ה", "ו", "ז", "ח", "ט", "י", "כ", "ל", "מ", "נ", "ס", "ע", "פ", "צ", "ק", "ר", "ש", "ת"]),
        "arabicAlpha" => alphabet(n, &["أ", "ب", "ت", "ث", "ج", "ح", "خ", "د", "ذ", "ر", "ز", "س", "ش", "ص", "ض", "ط", "ظ", "ع", "غ", "ف", "ق", "ك", "ل", "م", "ن", "هـ", "و", "ي"]),
        "arabicAbjad" => alphabet(n, &["أ", "ب", "ج", "د", "هـ", "و", "ز", "ح", "ط", "ي", "ك", "ل", "م", "ن", "س", "ع", "ف", "ص", "ق", "ر", "ش", "ت", "ث", "خ", "ذ", "ض", "ظ", "غ"]),
        // Hindi
        "hindiVowels" => alphabet(n, &["अ", "आ", "इ", "ई", "उ", "ऊ", "ऋ", "ए", "ऐ", "ओ", "औ"]),
        "hindiConsonants" => alphabet(
            n,
            &["क", "ख", "ग", "घ", "ङ", "च", "छ", "ज", "झ", "ञ", "ट", "ठ", "ड", "ढ", "ण", "त", "थ", "द", "ध", "न", "प", "फ", "ब", "भ", "म", "य", "र", "ल", "व", "श", "ष", "स", "ह"],
        ),
        "hindiNumbers" | "hindiCounting" => shift_digits(n, 0x0966),
        // Thai
        "thaiLetters" => alphabet(
            n,
            &[
                "ก", "ข", "ค", "ง", "จ", "ฉ", "ช", "ซ", "ฌ", "ญ", "ฎ", "ฏ", "ฐ", "ฑ", "ฒ", "ณ", "ด", "ต", "ถ", "ท", "ธ", "น", "บ", "ป", "ผ", "ฝ", "พ", "ฟ", "ภ", "ม", "ย", "ร", "ล", "ว", "ศ", "ษ", "ส", "ห", "ฬ", "อ", "ฮ",
            ],
        ),
        "thaiNumbers" => shift_digits(n, 0x0E50),
        "thaiCounting" => thai_words(n),
        // Vietnamese and Russian
        "vietnameseCounting" => vietnamese(n),
        "russianLower" => alphabet(n, &RUSSIAN_LOWER),
        "russianUpper" => alphabet(n, &RUSSIAN_UPPER),
        _ => n.to_string(),
    }
}

/// Every `w:numFmt` value Word writes that `format` knows by name, for tests and for the list dialogs.
pub const KNOWN_FORMATS: &[&str] = &[
    "decimal", "decimalZero", "upperRoman", "lowerRoman", "upperLetter", "lowerLetter", "ordinal", "cardinalText", "ordinalText", "hex", "chicago",
    "decimalEnclosedCircle", "decimalEnclosedFullstop", "decimalEnclosedParen", "decimalFullWidth", "decimalHalfWidth", "japaneseCounting",
    "japaneseDigitalTenThousand", "aiueo", "iroha", "chineseCounting", "chineseCountingThousand", "chineseLegalSimplified", "ideographDigital",
    "ideographTraditional", "ideographZodiac", "koreanDigital", "koreanCounting", "ganada", "chosung", "hebrew1", "hebrew2", "arabicAlpha", "arabicAbjad",
    "hindiVowels", "hindiConsonants", "hindiNumbers", "thaiLetters", "thaiNumbers", "thaiCounting", "vietnameseCounting", "russianLower", "russianUpper",
    "numberInDash", "bullet", "none",
];

fn cp(c: u32) -> String {
    char::from_u32(c).map(String::from).unwrap_or_default()
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Latin
// ---------------------------------------------------------------------------

/// Word repeats the letter: a..z, aa..zz, aaa...
fn repeated_letters(n: i64, base: u8) -> String {
    if n <= 0 {
        return String::new();
    }
    let idx = ((n - 1) % 26) as u8;
    let reps = ((n - 1) / 26 + 1) as usize;
    std::iter::repeat_n((base + idx) as char, reps).collect()
}

fn roman(n: i64) -> String {
    if n <= 0 {
        return n.to_string();
    }
    let mut n = n;
    let table = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut s = String::new();
    for (v, r) in table {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

fn chicago(n: i64) -> String {
    if n <= 0 {
        return String::new();
    }
    let sym = ["*", "\u{2020}", "\u{2021}", "\u{A7}"][((n - 1) % 4) as usize];
    sym.repeat(((n - 1) / 4 + 1) as usize)
}

fn ordinal(n: i64, lang: &str) -> String {
    match lang {
        "es" | "pt" => format!("{n}.\u{BA}"),
        "it" => format!("{n}\u{BA}"),
        "fr" => {
            if n == 1 {
                "1er".into()
            } else {
                format!("{n}e")
            }
        }
        "de" | "nl" | "da" | "fi" | "sv" | "no" | "nb" | "pl" | "cs" | "ru" => format!("{n}."),
        _ => {
            let suffix = match (n % 10, n % 100) {
                (_, 11..=13) => "th",
                (1, _) => "st",
                (2, _) => "nd",
                (3, _) => "rd",
                _ => "th",
            };
            format!("{n}{suffix}")
        }
    }
}

fn enclosed_circle(n: i64) -> String {
    match n {
        0 => cp(0x24EA),
        1..=20 => cp(0x2460 + (n as u32 - 1)),
        21..=35 => cp(0x3251 + (n as u32 - 21)),
        36..=50 => cp(0x32B1 + (n as u32 - 36)),
        _ => n.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Alphabets and digit systems
// ---------------------------------------------------------------------------

/// A letter sequence that repeats the letter when it runs out, as Word does for letters: a, b ... z, aa, bb.
fn alphabet(n: i64, letters: &[&str]) -> String {
    if n <= 0 {
        return String::new();
    }
    let idx = ((n - 1) as usize) % letters.len();
    letters[idx].repeat((n as usize - 1) / letters.len() + 1)
}

/// A sequence that starts again from the first symbol (zodiac signs).
fn cycle(n: i64, items: &[&str]) -> String {
    if n <= 0 {
        return n.to_string();
    }
    items[((n - 1) as usize) % items.len()].to_string()
}

/// Each decimal digit of `n` replaced by an entry of `digits`.
fn digits_in(n: i64, digits: &[&str; 10]) -> String {
    n.to_string().chars().map(|c| c.to_digit(10).map(|d| digits[d as usize].to_string()).unwrap_or_else(|| c.to_string())).collect()
}

/// Decimal digits drawn from another block of digits that starts at code point `zero`.
fn shift_digits(n: i64, zero: u32) -> String {
    n.to_string().chars().map(|c| c.to_digit(10).map(|d| cp(zero + d)).unwrap_or_else(|| c.to_string())).collect()
}

const AIUEO_HALF: [&str; 46] = [
    "\u{FF71}", "\u{FF72}", "\u{FF73}", "\u{FF74}", "\u{FF75}", "\u{FF76}", "\u{FF77}", "\u{FF78}", "\u{FF79}", "\u{FF7A}", "\u{FF7B}", "\u{FF7C}", "\u{FF7D}", "\u{FF7E}", "\u{FF7F}",
    "\u{FF80}", "\u{FF81}", "\u{FF82}", "\u{FF83}", "\u{FF84}", "\u{FF85}", "\u{FF86}", "\u{FF87}", "\u{FF88}", "\u{FF89}", "\u{FF8A}", "\u{FF8B}", "\u{FF8C}", "\u{FF8D}", "\u{FF8E}",
    "\u{FF8F}", "\u{FF90}", "\u{FF91}", "\u{FF92}", "\u{FF93}", "\u{FF94}", "\u{FF95}", "\u{FF96}", "\u{FF97}", "\u{FF98}", "\u{FF99}", "\u{FF9A}", "\u{FF9B}", "\u{FF9C}", "\u{FF66}",
    "\u{FF9D}",
];
const AIUEO_FULL: [&str; 46] = [
    "ア", "イ", "ウ", "エ", "オ", "カ", "キ", "ク", "ケ", "コ", "サ", "シ", "ス", "セ", "ソ", "タ", "チ", "ツ", "テ", "ト", "ナ", "ニ", "ヌ", "ネ", "ノ", "ハ", "ヒ", "フ", "ヘ", "ホ", "マ", "ミ", "ム",
    "メ", "モ", "ヤ", "ユ", "ヨ", "ラ", "リ", "ル", "レ", "ロ", "ワ", "ヲ", "ン",
];
const IROHA_FULL: [&str; 47] = [
    "イ", "ロ", "ハ", "ニ", "ホ", "ヘ", "ト", "チ", "リ", "ヌ", "ル", "ヲ", "ワ", "カ", "ヨ", "タ", "レ", "ソ", "ツ", "ネ", "ナ", "ラ", "ム", "ウ", "ヰ", "ノ", "オ", "ク", "ヤ", "マ", "ケ", "フ", "コ",
    "エ", "テ", "ア", "サ", "キ", "ユ", "メ", "ミ", "シ", "ヱ", "ヒ", "モ", "セ", "ス",
];
const IROHA_HALF: [&str; 47] = [
    "\u{FF72}", "\u{FF9B}", "\u{FF8A}", "\u{FF86}", "\u{FF8E}", "\u{FF8D}", "\u{FF84}", "\u{FF81}", "\u{FF98}", "\u{FF87}", "\u{FF99}", "\u{FF66}", "\u{FF9C}", "\u{FF76}", "\u{FF96}", "\u{FF80}",
    "\u{FF9A}", "\u{FF7F}", "\u{FF82}", "\u{FF88}", "\u{FF85}", "\u{FF97}", "\u{FF91}", "\u{FF73}", "\u{FF72}", "\u{FF89}", "\u{FF75}", "\u{FF78}", "\u{FF94}", "\u{FF8F}", "\u{FF79}", "\u{FF8C}",
    "\u{FF7A}", "\u{FF74}", "\u{FF83}", "\u{FF71}", "\u{FF7B}", "\u{FF77}", "\u{FF95}", "\u{FF92}", "\u{FF90}", "\u{FF7C}", "\u{FF74}", "\u{FF8B}", "\u{FF93}", "\u{FF7E}", "\u{FF7D}",
];
const RUSSIAN_LOWER: [&str; 28] = [
    "а", "б", "в", "г", "д", "е", "ж", "з", "и", "к", "л", "м", "н", "о", "п", "р", "с", "т", "у", "ф", "х", "ц", "ч", "ш", "щ", "э", "ю", "я",
];
const RUSSIAN_UPPER: [&str; 28] = [
    "А", "Б", "В", "Г", "Д", "Е", "Ж", "З", "И", "К", "Л", "М", "Н", "О", "П", "Р", "С", "Т", "У", "Ф", "Х", "Ц", "Ч", "Ш", "Щ", "Э", "Ю", "Я",
];

fn hebrew_numeral(n: i64) -> String {
    if !(1..=999).contains(&n) {
        return n.to_string();
    }
    let table = [
        (400, "ת"), (300, "ש"), (200, "ר"), (100, "ק"), (90, "צ"), (80, "פ"), (70, "ע"), (60, "ס"), (50, "נ"), (40, "מ"), (30, "ל"), (20, "כ"), (10, "י"), (9, "ט"), (8, "ח"), (7, "ז"), (6, "ו"),
        (5, "ה"), (4, "ד"), (3, "ג"), (2, "ב"), (1, "א"),
    ];
    let mut n = n;
    let mut s = String::new();
    for (v, l) in table {
        // 15 and 16 are written ט״ו and ט״ז, so the name of God is not spelled.
        if n == 15 {
            s.push_str("טו");
            n = 0;
        } else if n == 16 {
            s.push_str("טז");
            n = 0;
        }
        while n >= v {
            s.push_str(l);
            n -= v;
        }
    }
    s
}

// ---------------------------------------------------------------------------
// Counting in ideographs
// ---------------------------------------------------------------------------

struct Cjk {
    digits: [&'static str; 10],
    ten: &'static str,
    hundred: &'static str,
    thousand: &'static str,
    myriad: &'static str,
    hundred_million: &'static str,
    /// Chinese counting writes 十, not 一十, at the start of a number.
    short_ten: bool,
}

const CJK_JAPANESE: Cjk = Cjk { digits: ["〇", "一", "二", "三", "四", "五", "六", "七", "八", "九"], ten: "十", hundred: "百", thousand: "千", myriad: "万", hundred_million: "億" , short_ten: false };
const CJK_JAPANESE_LEGAL: Cjk = Cjk { digits: ["零", "壱", "弐", "参", "四", "五", "六", "七", "八", "九"], ten: "拾", hundred: "百", thousand: "千", myriad: "万", hundred_million: "億" , short_ten: false };
const CJK_CHINESE: Cjk = Cjk { digits: ["零", "一", "二", "三", "四", "五", "六", "七", "八", "九"], ten: "十", hundred: "百", thousand: "千", myriad: "万", hundred_million: "亿" , short_ten: true };
const CJK_TAIWANESE: Cjk = Cjk { digits: ["零", "一", "二", "三", "四", "五", "六", "七", "八", "九"], ten: "十", hundred: "百", thousand: "千", myriad: "萬", hundred_million: "億" , short_ten: true };
const CJK_LEGAL_SIMPLIFIED: Cjk = Cjk { digits: ["零", "壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌", "玖"], ten: "拾", hundred: "佰", thousand: "仟", myriad: "万", hundred_million: "亿" , short_ten: false };
const CJK_LEGAL_TRADITIONAL: Cjk = Cjk { digits: ["零", "壹", "貳", "參", "肆", "伍", "陸", "柒", "捌", "玖"], ten: "拾", hundred: "佰", thousand: "仟", myriad: "萬", hundred_million: "億" , short_ten: false };
const CJK_KOREAN: Cjk = Cjk { digits: ["영", "일", "이", "삼", "사", "오", "육", "칠", "팔", "구"], ten: "십", hundred: "백", thousand: "천", myriad: "만", hundred_million: "억" , short_ten: false };

/// A number in the positional units of Chinese, Japanese and Korean. `omit_one` drops the 1 before a unit
/// (Japanese and Korean write 百, not 一百); Chinese keeps it, except before a leading 十.
fn cjk(n: i64, c: &Cjk, omit_one: bool) -> String {
    if !(0..100_000_000_000).contains(&n) {
        return n.to_string();
    }
    if n == 0 {
        return c.digits[0].to_string();
    }
    cjk_part(n, c, omit_one, true)
}

fn cjk_part(n: i64, c: &Cjk, omit_one: bool, leading: bool) -> String {
    for (size, unit) in [(100_000_000, c.hundred_million), (10_000, c.myriad)] {
        if n >= size {
            let (hi, lo) = (n / size, n % size);
            let mut s = cjk_part(hi, c, omit_one, leading) + unit;
            if lo > 0 {
                // A gap in the middle (10001) is bridged with a zero in Chinese.
                if lo < size / 10 && !omit_one {
                    s.push_str(c.digits[0]);
                }
                s.push_str(&cjk_part(lo, c, omit_one, false));
            }
            return s;
        }
    }
    cjk_group(n, c, omit_one, leading)
}

/// 1 to 9999.
fn cjk_group(n: i64, c: &Cjk, omit_one: bool, leading: bool) -> String {
    let digits = [n / 1000, n / 100 % 10, n / 10 % 10, n % 10];
    let units = [c.thousand, c.hundred, c.ten, ""];
    let mut s = String::new();
    let mut gap = false;
    for (i, d) in digits.iter().enumerate() {
        let d = *d as usize;
        if d == 0 {
            if !s.is_empty() {
                gap = true;
            }
            continue;
        }
        if gap && !omit_one {
            s.push_str(c.digits[0]);
        }
        gap = false;
        let leading_ten = i == 2 && s.is_empty() && leading && c.short_ten;
        if i < 3 && d == 1 && (omit_one || leading_ten) {
            s.push_str(units[i]);
        } else {
            s.push_str(c.digits[d]);
            s.push_str(units[i]);
        }
    }
    s
}

// ---------------------------------------------------------------------------
// Thai and Vietnamese counting
// ---------------------------------------------------------------------------

fn thai_words(n: i64) -> String {
    if !(0..1_000_000_000).contains(&n) {
        return n.to_string();
    }
    if n == 0 {
        return "ศูนย์".into();
    }
    let digits = ["", "หนึ่ง", "สอง", "สาม", "สี่", "ห้า", "หก", "เจ็ด", "แปด", "เก้า"];
    if n >= 1_000_000 {
        let rest = n % 1_000_000;
        return thai_words(n / 1_000_000) + "ล้าน" + &if rest > 0 { thai_words(rest) } else { String::new() };
    }
    let units = ["", "สิบ", "ร้อย", "พัน", "หมื่น", "แสน"];
    let ds: Vec<usize> = n.to_string().chars().rev().map(|c| c.to_digit(10).unwrap() as usize).collect();
    let mut s = String::new();
    for pos in (0..ds.len()).rev() {
        let d = ds[pos];
        if d == 0 {
            continue;
        }
        let word = match (pos, d) {
            (1, 1) => "",
            (1, 2) => "ยี่",
            (0, 1) if ds.len() > 1 => "เอ็ด",
            _ => digits[d],
        };
        s.push_str(word);
        s.push_str(units[pos]);
    }
    s
}

fn vietnamese(n: i64) -> String {
    if !(1..1000).contains(&n) {
        return n.to_string();
    }
    let d = ["không", "một", "hai", "ba", "bốn", "năm", "sáu", "bảy", "tám", "chín"];
    fn below_hundred(n: i64, d: &[&str; 10]) -> String {
        match n {
            0..=9 => d[n as usize].to_string(),
            10 => "mười".into(),
            11..=19 => format!("mười {}", if n % 10 == 5 { "lăm" } else { d[(n % 10) as usize] }),
            _ => {
                let (t, u) = (n / 10, n % 10);
                let tail = match u {
                    0 => String::new(),
                    1 => " mốt".into(),
                    5 => " lăm".into(),
                    _ => format!(" {}", d[u as usize]),
                };
                format!("{} mươi{tail}", d[t as usize])
            }
        }
    }
    let s = if n >= 100 {
        let (h, rest) = (n / 100, n % 100);
        let tail = match rest {
            0 => String::new(),
            1..=9 => format!(" lẻ {}", d[rest as usize]),
            _ => format!(" {}", below_hundred(rest, &d)),
        };
        format!("{} trăm{tail}", d[h as usize])
    } else {
        below_hundred(n, &d)
    };
    capitalise(&s)
}

// ---------------------------------------------------------------------------
// Numbers in words
// ---------------------------------------------------------------------------

fn cardinal_words(n: i64, lang: &str) -> Option<String> {
    if n < 0 {
        return None;
    }
    match lang {
        "es" => (n < 1000).then(|| es_cardinal(n)),
        "fr" => (n < 1000).then(|| fr_cardinal(n)),
        "de" => (n < 1000).then(|| de_cardinal(n)),
        _ => (n < 1_000_000_000_000).then(|| en_cardinal(n)),
    }
}

fn ordinal_words(n: i64, lang: &str) -> Option<String> {
    if n < 1 {
        return None;
    }
    match lang {
        "es" => es_ordinal(n),
        "fr" => (n < 1000).then(|| fr_ordinal(n)),
        "de" => (n < 1000).then(|| de_ordinal(n)),
        _ => (n < 1_000_000_000_000).then(|| en_ordinal(n)),
    }
}

const EN_ONES: [&str; 20] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
];
const EN_TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];

fn en_cardinal(n: i64) -> String {
    if n < 20 {
        return EN_ONES[n as usize].into();
    }
    if n < 100 {
        let (t, u) = (n / 10, n % 10);
        return if u == 0 { EN_TENS[t as usize].into() } else { format!("{}-{}", EN_TENS[t as usize], EN_ONES[u as usize]) };
    }
    for (size, name) in [(1_000_000_000, "billion"), (1_000_000, "million"), (1000, "thousand"), (100, "hundred")] {
        if n >= size {
            let rest = n % size;
            let head = format!("{} {name}", en_cardinal(n / size));
            return if rest == 0 { head } else { format!("{head} {}", en_cardinal(rest)) };
        }
    }
    unreachable!()
}

fn en_ordinal(n: i64) -> String {
    let words = en_cardinal(n);
    // Only the last word changes: twenty-one becomes twenty-first.
    let split = words.rfind([' ', '-']).map(|i| i + 1).unwrap_or(0);
    let (head, last) = words.split_at(split);
    let ord = match last {
        "one" => "first",
        "two" => "second",
        "three" => "third",
        "five" => "fifth",
        "eight" => "eighth",
        "nine" => "ninth",
        "twelve" => "twelfth",
        w if w.ends_with('y') => return format!("{head}{}ieth", &w[..w.len() - 1]),
        w => return format!("{head}{w}th"),
    };
    format!("{head}{ord}")
}

const ES_ONES: [&str; 30] = [
    "cero", "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve", "diez", "once", "doce", "trece", "catorce", "quince", "dieciséis", "diecisiete", "dieciocho", "diecinueve",
    "veinte", "veintiuno", "veintidós", "veintitrés", "veinticuatro", "veinticinco", "veintiséis", "veintisiete", "veintiocho", "veintinueve",
];

fn es_cardinal(n: i64) -> String {
    let tens = ["", "", "", "treinta", "cuarenta", "cincuenta", "sesenta", "setenta", "ochenta", "noventa"];
    let hundreds = ["", "ciento", "doscientos", "trescientos", "cuatrocientos", "quinientos", "seiscientos", "setecientos", "ochocientos", "novecientos"];
    match n {
        0..=29 => ES_ONES[n as usize].into(),
        30..=99 => {
            let (t, u) = (n / 10, n % 10);
            if u == 0 {
                tens[t as usize].into()
            } else {
                format!("{} y {}", tens[t as usize], ES_ONES[u as usize])
            }
        }
        100 => "cien".into(),
        _ => {
            let (h, rest) = (n / 100, n % 100);
            if rest == 0 {
                hundreds[h as usize].into()
            } else {
                format!("{} {}", hundreds[h as usize], es_cardinal(rest))
            }
        }
    }
}

fn es_ordinal(n: i64) -> Option<String> {
    let names = ["", "primero", "segundo", "tercero", "cuarto", "quinto", "sexto", "séptimo", "octavo", "noveno", "décimo"];
    names.get(n as usize).filter(|s| !s.is_empty()).map(|s| s.to_string())
}

const FR_ONES: [&str; 20] = [
    "zéro", "un", "deux", "trois", "quatre", "cinq", "six", "sept", "huit", "neuf", "dix", "onze", "douze", "treize", "quatorze", "quinze", "seize", "dix-sept", "dix-huit", "dix-neuf",
];

fn fr_cardinal(n: i64) -> String {
    let tens = ["", "", "vingt", "trente", "quarante", "cinquante", "soixante"];
    match n {
        0..=19 => FR_ONES[n as usize].into(),
        20..=69 => {
            let (t, u) = (n / 10, n % 10);
            match u {
                0 => tens[t as usize].into(),
                1 => format!("{} et un", tens[t as usize]),
                _ => format!("{}-{}", tens[t as usize], FR_ONES[u as usize]),
            }
        }
        70..=79 => {
            if n == 71 {
                "soixante et onze".into()
            } else {
                format!("soixante-{}", FR_ONES[(n - 60) as usize])
            }
        }
        80 => "quatre-vingts".into(),
        81..=99 => format!("quatre-vingt-{}", FR_ONES[(n - 80) as usize]),
        100 => "cent".into(),
        _ => {
            let (h, rest) = (n / 100, n % 100);
            let head = if h == 1 { "cent".to_string() } else { format!("{} cent{}", FR_ONES[h as usize], if rest == 0 { "s" } else { "" }) };
            if rest == 0 {
                head
            } else {
                format!("{head} {}", fr_cardinal(rest))
            }
        }
    }
}

fn fr_ordinal(n: i64) -> String {
    if n == 1 {
        return "premier".into();
    }
    let c = fr_cardinal(n);
    let c = c.strip_suffix('s').filter(|_| n == 80 || n % 100 == 0).unwrap_or(&c);
    let stem = if let Some(s) = c.strip_suffix('e') {
        s.to_string()
    } else if let Some(s) = c.strip_suffix("cinq") {
        format!("{s}cinqu")
    } else if let Some(s) = c.strip_suffix("neuf") {
        format!("{s}neuv")
    } else {
        c.to_string()
    };
    format!("{stem}ième")
}

const DE_ONES: [&str; 20] = [
    "null", "eins", "zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun", "zehn", "elf", "zwölf", "dreizehn", "vierzehn", "fünfzehn", "sechzehn", "siebzehn", "achtzehn", "neunzehn",
];

fn de_cardinal(n: i64) -> String {
    let tens = ["", "", "zwanzig", "dreißig", "vierzig", "fünfzig", "sechzig", "siebzig", "achtzig", "neunzig"];
    match n {
        0..=19 => DE_ONES[n as usize].into(),
        20..=99 => {
            let (t, u) = (n / 10, n % 10);
            if u == 0 {
                tens[t as usize].into()
            } else {
                let unit = if u == 1 { "ein" } else { DE_ONES[u as usize] };
                format!("{unit}und{}", tens[t as usize])
            }
        }
        _ => {
            let (h, rest) = (n / 100, n % 100);
            let head = format!("{}hundert", if h == 1 { "ein" } else { DE_ONES[h as usize] });
            if rest == 0 {
                head
            } else {
                format!("{head}{}", de_cardinal(rest))
            }
        }
    }
}

fn de_ordinal(n: i64) -> String {
    match n {
        1 => "erste".into(),
        3 => "dritte".into(),
        7 => "siebte".into(),
        8 => "achte".into(),
        _ => {
            let c = de_cardinal(n);
            // The last word takes -te up to 19 and -ste after that.
            let small = n % 100 < 20 && n % 100 != 0;
            let c = if n % 100 == 1 { c.replacen("eins", "ers", 1) } else { c };
            let c = if n % 100 == 3 && n > 3 { c.trim_end_matches("drei").to_string() + "drit" } else { c };
            let c = if n % 100 == 7 && n > 7 { c.trim_end_matches("sieben").to_string() + "sieb" } else { c };
            let c = if n % 100 == 8 && n > 8 { c.trim_end_matches("acht").to_string() + "ach" } else { c };
            format!("{c}{}te", if small { "" } else { "s" })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(n: i64, fmt: &str) -> String {
        format(n, fmt, None)
    }

    #[test]
    fn latin_formats() {
        assert_eq!(f(4, "lowerRoman"), "iv");
        assert_eq!(f(14, "upperRoman"), "XIV");
        assert_eq!(f(28, "upperLetter"), "BB");
        assert_eq!(f(3, "decimalZero"), "03");
        assert_eq!(f(12, "decimalZero"), "12");
        assert_eq!(f(255, "hex"), "FF");
        assert_eq!(f(1, "chicago"), "*");
        assert_eq!(f(2, "chicago"), "\u{2020}");
        assert_eq!(f(4, "chicago"), "\u{A7}");
        assert_eq!(f(5, "chicago"), "**");
        assert_eq!(f(9, "chicago"), "***");
        assert_eq!(f(7, "numberInDash"), "- 7 -");
        assert_eq!(f(3, "none"), "");
    }

    #[test]
    fn ordinals_and_words() {
        assert_eq!(f(1, "ordinal"), "1st");
        assert_eq!(f(11, "ordinal"), "11th");
        assert_eq!(f(22, "ordinal"), "22nd");
        assert_eq!(f(1, "cardinalText"), "One");
        assert_eq!(f(21, "cardinalText"), "Twenty-one");
        assert_eq!(f(101, "cardinalText"), "One hundred one");
        assert_eq!(f(1234, "cardinalText"), "One thousand two hundred thirty-four");
        assert_eq!(f(1, "ordinalText"), "First");
        assert_eq!(f(12, "ordinalText"), "Twelfth");
        assert_eq!(f(20, "ordinalText"), "Twentieth");
        assert_eq!(f(21, "ordinalText"), "Twenty-first");
        assert_eq!(f(100, "ordinalText"), "One hundredth");
        assert_eq!(f(8, "ordinalText"), "Eighth");
    }

    #[test]
    fn words_follow_the_language() {
        let w = |n, fmt, lang| format(n, fmt, Some(lang));
        assert_eq!(w(1, "cardinalText", "es-ES"), "Uno");
        assert_eq!(w(21, "cardinalText", "es"), "Veintiuno");
        assert_eq!(w(45, "cardinalText", "es"), "Cuarenta y cinco");
        assert_eq!(w(1, "ordinalText", "es"), "Primero");
        assert_eq!(w(2, "cardinalText", "fr-FR"), "Deux");
        assert_eq!(w(21, "cardinalText", "fr"), "Vingt et un");
        assert_eq!(w(80, "cardinalText", "fr"), "Quatre-vingts");
        assert_eq!(w(1, "ordinalText", "fr"), "Premier");
        assert_eq!(w(2, "ordinalText", "fr"), "Deuxième");
        assert_eq!(w(4, "ordinalText", "fr"), "Quatrième");
        assert_eq!(w(5, "ordinalText", "fr"), "Cinquième");
        assert_eq!(w(9, "ordinalText", "fr"), "Neuvième");
        assert_eq!(w(1, "cardinalText", "de-DE"), "Eins");
        assert_eq!(w(21, "cardinalText", "de"), "Einundzwanzig");
        assert_eq!(w(1, "ordinalText", "de"), "Erste");
        assert_eq!(w(2, "ordinalText", "de"), "Zweite");
        assert_eq!(w(20, "ordinalText", "de"), "Zwanzigste");
        assert_eq!(w(3, "ordinalText", "en-GB"), "Third");
        // A language Word has no words for stays English.
        assert_eq!(w(2, "cardinalText", "ja-JP"), "Two");
        assert_eq!(w(1, "ordinal", "es"), "1.\u{BA}");
        assert_eq!(w(1, "ordinal", "fr"), "1er");
        assert_eq!(w(3, "ordinal", "de"), "3.");
    }

    #[test]
    fn enclosed_and_wide_digits() {
        assert_eq!(f(1, "decimalEnclosedCircle"), "\u{2460}");
        assert_eq!(f(20, "decimalEnclosedCircle"), "\u{2473}");
        assert_eq!(f(21, "decimalEnclosedCircle"), "\u{3251}");
        assert_eq!(f(51, "decimalEnclosedCircle"), "51");
        assert_eq!(f(1, "decimalEnclosedFullstop"), "\u{2488}");
        assert_eq!(f(1, "decimalEnclosedParen"), "\u{2474}");
        assert_eq!(f(21, "decimalEnclosedParen"), "(21)");
        assert_eq!(f(12, "decimalFullWidth"), "\u{FF11}\u{FF12}");
        assert_eq!(f(12, "decimalHalfWidth"), "12");
    }

    #[test]
    fn counting_in_ideographs() {
        assert_eq!(f(1, "japaneseCounting"), "一");
        assert_eq!(f(10, "japaneseCounting"), "十");
        assert_eq!(f(11, "japaneseCounting"), "十一");
        assert_eq!(f(20, "japaneseCounting"), "二十");
        assert_eq!(f(100, "japaneseCounting"), "百");
        assert_eq!(f(1234, "japaneseCounting"), "千二百三十四");
        assert_eq!(f(1, "chineseCounting"), "一");
        assert_eq!(f(10, "chineseCounting"), "十");
        assert_eq!(f(15, "chineseCounting"), "十五");
        assert_eq!(f(100, "chineseCounting"), "一百");
        assert_eq!(f(101, "chineseCounting"), "一百零一");
        assert_eq!(f(110, "chineseCounting"), "一百一十");
        assert_eq!(f(10000, "chineseCounting"), "一万");
        assert_eq!(f(10001, "chineseCounting"), "一万零一");
        assert_eq!(f(2, "chineseLegalSimplified"), "贰");
        assert_eq!(f(10, "chineseLegalSimplified"), "壹拾");
        assert_eq!(f(12, "chineseLegalSimplified"), "壹拾贰");
        assert_eq!(f(10, "ideographDigital"), "一〇");
        assert_eq!(f(11, "ideographTraditional"), "甲甲");
        assert_eq!(f(3, "ideographTraditional"), "丙");
        assert_eq!(f(1, "ideographZodiac"), "子");
        assert_eq!(f(13, "ideographZodiac"), "子");
        assert_eq!(f(1, "koreanDigital"), "일");
        assert_eq!(f(10, "koreanDigital"), "일영");
        assert_eq!(f(12, "koreanCounting"), "십이");
        assert_eq!(f(1, "ganada"), "가");
        assert_eq!(f(15, "ganada"), "가가");
        assert_eq!(f(2, "chosung"), "ㄴ");
    }

    #[test]
    fn japanese_syllables() {
        assert_eq!(f(1, "aiueoFullWidth"), "ア");
        assert_eq!(f(6, "aiueoFullWidth"), "カ");
        assert_eq!(f(1, "aiueo"), "\u{FF71}");
        assert_eq!(f(1, "irohaFullWidth"), "イ");
        assert_eq!(f(3, "irohaFullWidth"), "ハ");
    }

    #[test]
    fn right_to_left_and_indic_scripts() {
        assert_eq!(f(1, "hebrew1"), "א");
        assert_eq!(f(10, "hebrew1"), "י");
        assert_eq!(f(11, "hebrew1"), "יא");
        assert_eq!(f(15, "hebrew1"), "טו");
        assert_eq!(f(16, "hebrew1"), "טז");
        assert_eq!(f(500, "hebrew1"), "תק");
        assert_eq!(f(3, "hebrew2"), "ג");
        assert_eq!(f(23, "hebrew2"), "אא");
        assert_eq!(f(1, "arabicAlpha"), "أ");
        assert_eq!(f(3, "arabicAbjad"), "ج");
        assert_eq!(f(1, "hindiVowels"), "अ");
        assert_eq!(f(2, "hindiConsonants"), "ख");
        assert_eq!(f(23, "hindiNumbers"), "२३");
    }

    #[test]
    fn thai_vietnamese_and_russian() {
        assert_eq!(f(1, "thaiLetters"), "ก");
        assert_eq!(f(12, "thaiNumbers"), "๑๒");
        assert_eq!(f(1, "thaiCounting"), "หนึ่ง");
        assert_eq!(f(10, "thaiCounting"), "สิบ");
        assert_eq!(f(11, "thaiCounting"), "สิบเอ็ด");
        assert_eq!(f(20, "thaiCounting"), "ยี่สิบ");
        assert_eq!(f(25, "thaiCounting"), "ยี่สิบห้า");
        assert_eq!(f(100, "thaiCounting"), "หนึ่งร้อย");
        assert_eq!(f(1, "vietnameseCounting"), "Một");
        assert_eq!(f(10, "vietnameseCounting"), "Mười");
        assert_eq!(f(15, "vietnameseCounting"), "Mười lăm");
        assert_eq!(f(21, "vietnameseCounting"), "Hai mươi mốt");
        assert_eq!(f(105, "vietnameseCounting"), "Một trăm lẻ năm");
        assert_eq!(f(1, "russianLower"), "а");
        assert_eq!(f(3, "russianUpper"), "В");
        assert_eq!(f(29, "russianLower"), "аа");
    }

    #[test]
    fn every_known_format_gives_something() {
        for fmt in KNOWN_FORMATS {
            for n in [1, 2, 9, 10, 25] {
                let s = format(n, fmt, None);
                if *fmt != "none" && *fmt != "bullet" {
                    assert!(!s.is_empty(), "{fmt} {n}");
                }
            }
        }
    }
}
