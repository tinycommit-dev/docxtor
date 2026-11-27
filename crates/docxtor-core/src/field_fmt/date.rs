//! Dates and times: the calendar arithmetic, month and day names by language, and `\@` pictures.


/// A calendar date and time of day, no time zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dt {
    pub year: i32,
    /// 1 to 12.
    pub month: u32,
    /// 1 to 31.
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// Days since 1970-01-01 for a civil date (proleptic Gregorian).
pub(super) fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date for a count of days since 1970-01-01.
pub(super) fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

pub(super) fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub(super) fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(y) => 29,
        _ => 28,
    }
}

impl Dt {
    /// The UTC civil time for a Unix timestamp.
    pub fn from_unix(secs: i64) -> Dt {
        // Far outside any calendar the year stays a number; the arithmetic below cannot overflow.
        let days = secs.div_euclid(86_400).clamp(-100_000_000_000, 100_000_000_000);
        let rem = secs.rem_euclid(86_400) as u32;
        let (year, month, day) = civil_from_days(days);
        Dt { year, month, day, hour: rem / 3600, minute: rem % 3600 / 60, second: rem % 60 }
    }

    /// The Unix timestamp, reading the fields as UTC.
    pub fn to_unix(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * 86_400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
    }

    /// Parses `2026-10-13T09:30:00Z`, `2026-10-13T09:30:00`, `2026-10-13T09:30` and `2026-10-13` (time 00:00:00).
    /// A trailing `Z`, a `+hh:mm` or `-hh:mm` offset and fractional seconds are ignored.
    pub fn parse_iso(s: &str) -> Option<Dt> {
        let s = s.trim();
        let (date, time) = match s.find(['T', 't', ' ']) {
            Some(i) => (&s[..i], Some(&s[i + 1..])),
            None => (s, None),
        };
        let mut dp = date.split('-');
        let (ys, ms, ds) = (dp.next()?, dp.next()?, dp.next()?);
        if dp.next().is_some() || ys.len() != 4 || ms.len() != 2 || ds.len() != 2 {
            return None;
        }
        let year: i32 = num_only(ys)?.parse().ok()?;
        let month: u32 = num_only(ms)?.parse().ok()?;
        let day: u32 = num_only(ds)?.parse().ok()?;
        if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return None;
        }
        let (mut hour, mut minute, mut second) = (0, 0, 0);
        if let Some(t) = time {
            let t = t.trim_end_matches(['Z', 'z']);
            // Cut an offset or fractional seconds; the time itself never has a sign.
            let t = t.split(['+', '-', '.', ',']).next().unwrap_or("");
            let mut tp = t.split(':');
            let (hs, mins) = (tp.next()?, tp.next()?);
            let secs = tp.next();
            if tp.next().is_some() || hs.len() != 2 || mins.len() != 2 {
                return None;
            }
            hour = num_only(hs)?.parse().ok()?;
            minute = num_only(mins)?.parse().ok()?;
            if let Some(sv) = secs {
                if sv.len() != 2 {
                    return None;
                }
                second = num_only(sv)?.parse().ok()?;
            }
            if hour > 23 || minute > 59 || second > 59 {
                return None;
            }
        }
        Some(Dt { year, month, day, hour, minute, second })
    }

    /// The day of the week, 0 for Sunday.
    pub fn weekday(&self) -> u32 {
        let days = days_from_civil(self.year, self.month, self.day);
        // 1970-01-01 was a Thursday.
        (days + 4).rem_euclid(7) as u32
    }

    /// `2026-10-13T09:30:00Z`.
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", self.year, self.month, self.day, self.hour, self.minute, self.second)
    }
}

/// The text if it is all ASCII digits.
pub(super) fn num_only(s: &str) -> Option<&str> {
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then_some(s)
}

/// Month and day names, and the am/pm words, of a language.
pub struct DateNames {
    pub months: [&'static str; 12],
    pub months_short: [&'static str; 12],
    /// Sunday first.
    pub days: [&'static str; 7],
    pub days_short: [&'static str; 7],
    pub am: &'static str,
    pub pm: &'static str,
}

pub(super) static EN: DateNames = DateNames {
    months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
    months_short: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
    days: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
    days_short: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
    am: "AM",
    pm: "PM",
};

pub(super) static ES: DateNames = DateNames {
    months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
    months_short: ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"],
    days: ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
    days_short: ["dom", "lun", "mar", "mié", "jue", "vie", "sáb"],
    am: "a. m.",
    pm: "p. m.",
};

pub(super) static FR: DateNames = DateNames {
    months: ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
    months_short: ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."],
    days: ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
    days_short: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
    am: "AM",
    pm: "PM",
};

pub(super) static DE: DateNames = DateNames {
    months: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
    months_short: ["Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez"],
    days: ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
    days_short: ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"],
    am: "AM",
    pm: "PM",
};

pub(super) static IT: DateNames = DateNames {
    months: ["gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre"],
    months_short: ["gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic"],
    days: ["domenica", "lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato"],
    days_short: ["dom", "lun", "mar", "mer", "gio", "ven", "sab"],
    am: "AM",
    pm: "PM",
};

pub(super) static PT: DateNames = DateNames {
    months: ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
    months_short: ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"],
    days: ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"],
    days_short: ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"],
    am: "AM",
    pm: "PM",
};

pub(super) static NL: DateNames = DateNames {
    months: ["januari", "februari", "maart", "april", "mei", "juni", "juli", "augustus", "september", "oktober", "november", "december"],
    months_short: ["jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec"],
    days: ["zondag", "maandag", "dinsdag", "woensdag", "donderdag", "vrijdag", "zaterdag"],
    days_short: ["zo", "ma", "di", "wo", "do", "vr", "za"],
    am: "a.m.",
    pm: "p.m.",
};

/// The locales Docxtor has date data for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Loc {
    EnUs,
    EnGb,
    Es,
    Fr,
    De,
    It,
    Pt,
    Nl,
}

pub(super) fn locale(lang: Option<&str>) -> Loc {
    let Some(tag) = lang else { return Loc::EnUs };
    let mut parts = tag.split(['-', '_']);
    let primary = parts.next().unwrap_or("").to_ascii_lowercase();
    let region = parts.find(|p| p.len() == 2).map(str::to_ascii_uppercase).unwrap_or_default();
    match primary.as_str() {
        "en" if matches!(region.as_str(), "GB" | "AU" | "NZ" | "IE" | "IN" | "ZA") => Loc::EnGb,
        "es" => Loc::Es,
        "fr" => Loc::Fr,
        "de" => Loc::De,
        "it" => Loc::It,
        "pt" => Loc::Pt,
        "nl" => Loc::Nl,
        _ => Loc::EnUs,
    }
}

/// `lang` is a BCP 47 tag such as `en-US`, `es-ES`, `fr-FR`, `de-DE`, `it-IT`, `pt-BR` or `nl-NL`. The primary
/// subtag decides, case-insensitively. Anything else, and `None`, gets English.
pub fn names_for(lang: Option<&str>) -> &'static DateNames {
    match locale(lang) {
        Loc::EnUs | Loc::EnGb => &EN,
        Loc::Es => &ES,
        Loc::Fr => &FR,
        Loc::De => &DE,
        Loc::It => &IT,
        Loc::Pt => &PT,
        Loc::Nl => &NL,
    }
}

/// Cases of a marker word: follows the picture (`AM/PM` upper, `am/pm` lower, otherwise as the language writes it).
pub(super) fn match_case(word: &str, sample: &str) -> String {
    let letters: Vec<char> = sample.chars().filter(|c| c.is_alphabetic()).collect();
    if !letters.is_empty() && letters.iter().all(|c| c.is_uppercase()) {
        word.to_uppercase()
    } else if letters.iter().all(|c| c.is_lowercase()) {
        word.to_lowercase()
    } else {
        word.to_string()
    }
}

/// Word's `\@` date-time picture.
///
/// Letters: `d` and `dd` (day), `ddd` and `dddd` (weekday, short and long), `M` and `MM` (month number), `MMM` and
/// `MMMM` (month name, short and long), `yy` and `yyyy` (a single `y` is the two-digit year too), `h` and `hh`
/// (12-hour), `H` and `HH` (24-hour), `m` and `mm` (minutes), `s` and `ss`, and the markers `am/pm`, `AM/PM`,
/// `a/p` and `A/P`. Text in single quotes is copied (a doubled quote stands for one quote). A backslash makes the
/// next character literal. Every other character is copied as is.
pub fn date_picture(dt: &Dt, picture: &str, lang: Option<&str>) -> String {
    let names = names_for(lang);
    let chars: Vec<char> = picture.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let run = chars[i..].iter().take_while(|x| **x == c).count();
        match c {
            '\'' => {
                i += 1;
                while i < chars.len() {
                    match chars[i] {
                        '\'' if chars.get(i + 1) == Some(&'\'') => {
                            out.push('\'');
                            i += 2;
                        }
                        '\'' => {
                            i += 1;
                            break;
                        }
                        '\\' if i + 1 < chars.len() => {
                            out.push(chars[i + 1]);
                            i += 2;
                        }
                        other => {
                            out.push(other);
                            i += 1;
                        }
                    }
                }
            }
            '\\' => {
                if let Some(n) = chars.get(i + 1) {
                    out.push(*n);
                }
                i += 2;
            }
            'd' => {
                match run {
                    1 => out.push_str(&dt.day.to_string()),
                    2 => out.push_str(&format!("{:02}", dt.day)),
                    3 => out.push_str(names.days_short[dt.weekday() as usize]),
                    _ => out.push_str(names.days[dt.weekday() as usize]),
                }
                i += run;
            }
            'M' => {
                let mi = (dt.month.clamp(1, 12) - 1) as usize;
                match run {
                    1 => out.push_str(&dt.month.to_string()),
                    2 => out.push_str(&format!("{:02}", dt.month)),
                    3 => out.push_str(names.months_short[mi]),
                    _ => out.push_str(names.months[mi]),
                }
                i += run;
            }
            'y' => {
                if run <= 2 {
                    out.push_str(&format!("{:02}", dt.year.rem_euclid(100)));
                } else {
                    out.push_str(&format!("{:04}", dt.year));
                }
                i += run;
            }
            'h' => {
                let h = if dt.hour.is_multiple_of(12) { 12 } else { dt.hour % 12 };
                if run == 1 {
                    out.push_str(&h.to_string());
                } else {
                    out.push_str(&format!("{h:02}"));
                }
                i += run;
            }
            'H' => {
                if run == 1 {
                    out.push_str(&dt.hour.to_string());
                } else {
                    out.push_str(&format!("{:02}", dt.hour));
                }
                i += run;
            }
            'm' => {
                if run == 1 {
                    out.push_str(&dt.minute.to_string());
                } else {
                    out.push_str(&format!("{:02}", dt.minute));
                }
                i += run;
            }
            's' => {
                if run == 1 {
                    out.push_str(&dt.second.to_string());
                } else {
                    out.push_str(&format!("{:02}", dt.second));
                }
                i += run;
            }
            'a' | 'A' => {
                let pm = dt.hour >= 12;
                let word = if pm { names.pm } else { names.am };
                let long: String = chars[i..].iter().take(5).collect();
                let short: String = chars[i..].iter().take(3).collect();
                if long.eq_ignore_ascii_case("am/pm") {
                    out.push_str(&match_case(word, &long));
                    i += 5;
                } else if short.eq_ignore_ascii_case("a/p") {
                    let first: String = word.chars().next().into_iter().collect();
                    out.push_str(&match_case(&first, &short[..1]));
                    i += 3;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    out
}

pub(super) const FORMATS_EN_US: &[&str] = &[
    "M/d/yyyy",
    "dddd, MMMM d, yyyy",
    "MMMM d, yyyy",
    "M/d/yy",
    "yyyy-MM-dd",
    "d-MMM-yy",
    "M.d.yyyy",
    "MMM. d, yy",
    "d MMMM yyyy",
    "MMMM yy",
    "MMM-yy",
    "M/d/yyyy h:mm am/pm",
    "M/d/yyyy h:mm:ss am/pm",
    "h:mm am/pm",
    "h:mm:ss am/pm",
    "HH:mm",
    "HH:mm:ss",
];

pub(super) const FORMATS_EN_GB: &[&str] = &[
    "dd/MM/yyyy",
    "dddd, d MMMM yyyy",
    "d MMMM yyyy",
    "dd/MM/yy",
    "yyyy-MM-dd",
    "d MMM yy",
    "d.M.yyyy",
    "MMMM yy",
    "MMM-yy",
    "dd/MM/yyyy HH:mm",
    "dd/MM/yyyy HH:mm:ss",
    "HH:mm",
    "HH:mm:ss",
    "h:mm am/pm",
    "h:mm:ss am/pm",
];

pub(super) const FORMATS_DE: &[&str] = &[
    "dd.MM.yyyy",
    "dddd, d. MMMM yyyy",
    "d. MMMM yyyy",
    "dd.MM.yy",
    "yyyy-MM-dd",
    "d. MMM yyyy",
    "MMMM yy",
    "dd.MM.yyyy HH:mm",
    "dd.MM.yyyy HH:mm:ss",
    "HH:mm",
    "HH:mm:ss",
];

pub(super) const FORMATS_ES: &[&str] = &[
    "dd/MM/yyyy",
    "dddd, d 'de' MMMM 'de' yyyy",
    "d 'de' MMMM 'de' yyyy",
    "dd/MM/yy",
    "yyyy-MM-dd",
    "d-MMM-yy",
    "d 'de' MMMM",
    "MMMM 'de' yyyy",
    "dd/MM/yyyy H:mm",
    "dd/MM/yyyy H:mm:ss",
    "H:mm",
    "H:mm:ss",
    "h:mm am/pm",
    "h:mm:ss am/pm",
];

pub(super) const FORMATS_FR: &[&str] = &[
    "dd/MM/yyyy",
    "dddd d MMMM yyyy",
    "d MMMM yyyy",
    "dd/MM/yy",
    "yyyy-MM-dd",
    "d MMM yy",
    "MMMM yyyy",
    "dd/MM/yyyy HH:mm",
    "dd/MM/yyyy HH:mm:ss",
    "HH:mm",
    "HH:mm:ss",
];

pub(super) const FORMATS_IT: &[&str] = &[
    "dd/MM/yyyy",
    "dddd d MMMM yyyy",
    "d MMMM yyyy",
    "dd/MM/yy",
    "yyyy-MM-dd",
    "d-MMM-yy",
    "MMMM yyyy",
    "dd/MM/yyyy HH:mm",
    "dd/MM/yyyy HH:mm:ss",
    "HH:mm",
    "HH:mm:ss",
];

pub(super) const FORMATS_PT: &[&str] = &[
    "dd/MM/yyyy",
    "dddd, d 'de' MMMM 'de' yyyy",
    "d 'de' MMMM 'de' yyyy",
    "dd/MM/yy",
    "yyyy-MM-dd",
    "d-MMM-yy",
    "MMMM 'de' yyyy",
    "dd/MM/yyyy HH:mm",
    "dd/MM/yyyy HH:mm:ss",
    "HH:mm",
    "HH:mm:ss",
];

pub(super) const FORMATS_NL: &[&str] = &[
    "d-M-yyyy",
    "dddd d MMMM yyyy",
    "d MMMM yyyy",
    "d-M-yy",
    "yyyy-MM-dd",
    "d-MMM-yy",
    "MMMM yyyy",
    "d-M-yyyy HH:mm",
    "d-M-yyyy HH:mm:ss",
    "HH:mm",
    "HH:mm:ss",
];

/// The pictures the Insert, Date and time dialog offers for a language, in the order Word lists them.
/// Languages without their own list get the en-US one.
pub fn date_formats(lang: Option<&str>) -> Vec<&'static str> {
    match locale(lang) {
        Loc::EnUs => FORMATS_EN_US,
        Loc::EnGb => FORMATS_EN_GB,
        Loc::De => FORMATS_DE,
        Loc::Es => FORMATS_ES,
        Loc::Fr => FORMATS_FR,
        Loc::It => FORMATS_IT,
        Loc::Pt => FORMATS_PT,
        Loc::Nl => FORMATS_NL,
    }
    .to_vec()
}

/// The short date picture a DATE field uses when it has no `\@` switch.
pub fn default_date_picture(lang: Option<&str>) -> &'static str {
    match locale(lang) {
        Loc::EnUs => "M/d/yyyy",
        Loc::EnGb | Loc::Es | Loc::Fr | Loc::It | Loc::Pt => "dd/MM/yyyy",
        Loc::De => "dd.MM.yyyy",
        Loc::Nl => "d-M-yyyy",
    }
}

/// The time picture a TIME field uses when it has no `\@` switch.
pub fn default_time_picture(lang: Option<&str>) -> &'static str {
    match locale(lang) {
        Loc::EnUs => "h:mm am/pm",
        Loc::Es => "H:mm",
        Loc::EnGb | Loc::Fr | Loc::De | Loc::It | Loc::Pt | Loc::Nl => "HH:mm",
    }
}
