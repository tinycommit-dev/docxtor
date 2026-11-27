//! Field switches: case, number words, numeric and date-time pictures.
//!
//! Word lets a field carry formatting switches: `\@` for dates and times, `\#` for numbers and `\*` for case and
//! number styles. This module holds the pure functions behind them. Nothing here touches the document model.

use crate::numbering::format_number;

mod date;
mod numbers;
mod words;

pub use date::{date_formats, date_picture, default_date_picture, default_time_picture, names_for, DateNames, Dt};
pub use numbers::{general_number, numeric_picture, parse_number};
use numbers::*;
pub use words::{apply_case, number_format};

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> Dt {
        Dt { year: y, month: mo, day: d, hour: h, minute: mi, second: s }
    }

    // --- Dt ---

    #[test]
    fn unix_round_trip() {
        assert_eq!(Dt::from_unix(0), dt(1970, 1, 1, 0, 0, 0));
        assert_eq!(Dt::from_unix(1_760_347_800), dt(2025, 10, 13, 9, 30, 0));
        for secs in [-86_400_i64, -1, 0, 1, 951_782_400, 1_709_251_199, 1_709_251_200, 4_102_444_800, 1_760_347_800] {
            assert_eq!(Dt::from_unix(secs).to_unix(), secs);
        }
        assert_eq!(dt(2000, 2, 29, 12, 0, 0).to_unix(), 951_825_600);
    }

    #[test]
    fn before_epoch() {
        assert_eq!(Dt::from_unix(-1), dt(1969, 12, 31, 23, 59, 59));
        assert_eq!(dt(1900, 3, 1, 0, 0, 0).weekday(), 4);
    }

    #[test]
    fn weekdays() {
        assert_eq!(dt(1970, 1, 1, 0, 0, 0).weekday(), 4);
        assert_eq!(dt(2026, 10, 13, 0, 0, 0).weekday(), 2);
        assert_eq!(dt(2024, 2, 29, 0, 0, 0).weekday(), 4);
        assert_eq!(dt(2000, 1, 1, 0, 0, 0).weekday(), 6);
        assert_eq!(dt(2023, 10, 15, 0, 0, 0).weekday(), 0);
    }

    #[test]
    fn iso_and_parse() {
        let d = dt(2026, 10, 13, 9, 30, 0);
        assert_eq!(d.iso(), "2026-10-13T09:30:00Z");
        assert_eq!(Dt::parse_iso("2026-10-13T09:30:00Z"), Some(d));
        assert_eq!(Dt::parse_iso("2026-10-13T09:30:00"), Some(d));
        assert_eq!(Dt::parse_iso("2026-10-13T09:30"), Some(d));
        assert_eq!(Dt::parse_iso("2026-10-13"), Some(dt(2026, 10, 13, 0, 0, 0)));
        assert_eq!(Dt::parse_iso("2026-10-13T09:30:00+02:00"), Some(d));
        assert_eq!(Dt::parse_iso("2026-10-13T09:30:00-05:00"), Some(d));
        assert_eq!(Dt::parse_iso("2026-10-13T09:30:15.250Z"), Some(dt(2026, 10, 13, 9, 30, 15)));
        assert_eq!(Dt::parse_iso("  2026-10-13  "), Some(dt(2026, 10, 13, 0, 0, 0)));
    }

    #[test]
    fn parse_rejects_bad_dates() {
        for bad in [
            "", "2026", "2026-13-01", "2026-00-10", "2026-02-30", "2023-02-29", "2026-10-13T24:00", "2026-10-13T09:60", "2026-10-13T09",
            "26-10-13", "2026-1-13", "abcd-10-13", "2026-10-13T09:30:00:00",
        ] {
            assert_eq!(Dt::parse_iso(bad), None, "{bad}");
        }
        assert_eq!(Dt::parse_iso("2024-02-29"), Some(dt(2024, 2, 29, 0, 0, 0)));
    }

    // --- names ---

    #[test]
    fn names_by_language() {
        assert_eq!(names_for(None).months[0], "January");
        assert_eq!(names_for(Some("en-US")).days[0], "Sunday");
        assert_eq!(names_for(Some("ES-es")).months[0], "enero");
        assert_eq!(names_for(Some("fr")).days[1], "lundi");
        assert_eq!(names_for(Some("de-DE")).months[2], "März");
        assert_eq!(names_for(Some("it-IT")).days[0], "domenica");
        assert_eq!(names_for(Some("pt-BR")).months[2], "março");
        assert_eq!(names_for(Some("nl-NL")).months[2], "maart");
        assert_eq!(names_for(Some("ja-JP")).months[0], "January");
        assert_eq!(names_for(Some("")).months[0], "January");
    }

    #[test]
    fn names_are_complete() {
        for l in ["en", "es", "fr", "de", "it", "pt", "nl"] {
            let n = names_for(Some(l));
            assert!(n.months.iter().chain(&n.months_short).chain(&n.days).chain(&n.days_short).all(|s| !s.is_empty()), "{l}");
            assert!(!n.am.is_empty() && !n.pm.is_empty());
        }
    }

    // --- date_picture ---

    #[test]
    fn date_basic_pictures() {
        let d = dt(2026, 10, 13, 9, 5, 7);
        assert_eq!(date_picture(&d, "M/d/yyyy", None), "10/13/2026");
        assert_eq!(date_picture(&d, "dddd, MMMM d, yyyy", None), "Tuesday, October 13, 2026");
        assert_eq!(date_picture(&d, "ddd d MMM yy", None), "Tue 13 Oct 26");
        assert_eq!(date_picture(&d, "yyyy-MM-dd", None), "2026-10-13");
        assert_eq!(date_picture(&d, "MMM. d, yy", None), "Oct. 13, 26");
        assert_eq!(date_picture(&d, "y", None), "26");
        assert_eq!(date_picture(&d, "yyy", None), "2026");
    }

    #[test]
    fn date_zero_padding() {
        let d = dt(2026, 3, 4, 7, 8, 9);
        assert_eq!(date_picture(&d, "d M", None), "4 3");
        assert_eq!(date_picture(&d, "dd MM", None), "04 03");
        assert_eq!(date_picture(&d, "H:m:s", None), "7:8:9");
        assert_eq!(date_picture(&d, "HH:mm:ss", None), "07:08:09");
        assert_eq!(date_picture(&d, "h:mm", None), "7:08");
        assert_eq!(date_picture(&d, "hh:mm", None), "07:08");
    }

    #[test]
    fn date_minutes_versus_month() {
        let d = dt(2026, 10, 13, 9, 30, 0);
        assert_eq!(date_picture(&d, "MM mm", None), "10 30");
        assert_eq!(date_picture(&d, "M m", None), "10 30");
    }

    #[test]
    fn date_twelve_hour() {
        assert_eq!(date_picture(&dt(2026, 1, 1, 0, 5, 0), "h:mm am/pm", None), "12:05 am");
        assert_eq!(date_picture(&dt(2026, 1, 1, 12, 0, 0), "h:mm am/pm", None), "12:00 pm");
        assert_eq!(date_picture(&dt(2026, 1, 1, 13, 45, 0), "h:mm AM/PM", None), "1:45 PM");
        assert_eq!(date_picture(&dt(2026, 1, 1, 23, 59, 0), "hh:mm am/pm", None), "11:59 pm");
        assert_eq!(date_picture(&dt(2026, 1, 1, 9, 0, 0), "h:mm a/p", None), "9:00 a");
        assert_eq!(date_picture(&dt(2026, 1, 1, 21, 0, 0), "h:mm A/P", None), "9:00 P");
        assert_eq!(date_picture(&dt(2026, 1, 1, 21, 0, 0), "H:mm", None), "21:00");
    }

    #[test]
    fn date_marker_in_other_languages() {
        let d = dt(2026, 1, 1, 15, 0, 0);
        assert_eq!(date_picture(&d, "h:mm am/pm", Some("es-ES")), "3:00 p. m.");
        assert_eq!(date_picture(&d, "h:mm AM/PM", Some("es-ES")), "3:00 P. M.");
        assert_eq!(date_picture(&d, "h:mm am/pm", Some("nl-NL")), "3:00 p.m.");
        assert_eq!(date_picture(&d, "h:mm a/p", Some("es-ES")), "3:00 p");
    }

    #[test]
    fn date_literals_and_escapes() {
        let d = dt(2026, 10, 13, 9, 30, 0);
        assert_eq!(date_picture(&d, "'Date:' d MMMM", None), "Date: 13 October");
        assert_eq!(date_picture(&d, "d 'o''clock'", None), "13 o'clock");
        assert_eq!(date_picture(&d, "\\d d", None), "d 13");
        assert_eq!(date_picture(&d, "'a\\'b' d", None), "a'b 13");
        assert_eq!(date_picture(&d, "yyyy-MM-dd'T'HH:mm", None), "2026-10-13T09:30");
        assert_eq!(date_picture(&d, "d 'de' MMMM 'de' yyyy", Some("es-ES")), "13 de octubre de 2026");
        assert_eq!(date_picture(&d, "'unterminated d", None), "unterminated d");
        assert_eq!(date_picture(&d, "d\\", None), "13");
        assert_eq!(date_picture(&d, "", None), "");
    }

    #[test]
    fn date_other_characters_copied() {
        let d = dt(2026, 10, 13, 9, 30, 0);
        assert_eq!(date_picture(&d, "d.M.yyyy, HH:mm", None), "13.10.2026, 09:30");
        assert_eq!(date_picture(&d, "(d) [M] {y}", None), "(13) [10] {26}");
        // Letters that are not picture letters stay.
        assert_eq!(date_picture(&d, "Q d", None), "Q 13");
        assert_eq!(date_picture(&d, "a d", None), "a 13");
    }

    #[test]
    fn date_in_languages() {
        let d = dt(2026, 10, 13, 9, 30, 0);
        assert_eq!(date_picture(&d, "dddd, d. MMMM yyyy", Some("de-DE")), "Dienstag, 13. Oktober 2026");
        assert_eq!(date_picture(&d, "dddd d MMMM yyyy", Some("fr-FR")), "mardi 13 octobre 2026");
        assert_eq!(date_picture(&d, "dddd d MMMM yyyy", Some("it-IT")), "martedì 13 ottobre 2026");
        assert_eq!(date_picture(&d, "dddd, d 'de' MMMM", Some("pt-BR")), "terça-feira, 13 de outubro");
        assert_eq!(date_picture(&d, "ddd d MMM", Some("nl-NL")), "di 13 okt");
        assert_eq!(date_picture(&d, "ddd d MMM", Some("fr")), "mar. 13 oct.");
    }

    #[test]
    fn date_midnight_and_year_padding() {
        assert_eq!(date_picture(&dt(2026, 1, 1, 0, 0, 0), "h am/pm", None), "12 am");
        assert_eq!(date_picture(&dt(5, 1, 1, 0, 0, 0), "yyyy", None), "0005");
    }

    // --- date formats ---

    #[test]
    fn formats_en_us_match_word() {
        let f = date_formats(Some("en-US"));
        assert_eq!(f.len(), 17);
        assert_eq!(f[0], "M/d/yyyy");
        assert_eq!(f[1], "dddd, MMMM d, yyyy");
        assert_eq!(f[11], "M/d/yyyy h:mm am/pm");
        assert_eq!(f[16], "HH:mm:ss");
        assert_eq!(date_formats(None), f);
        assert_eq!(date_formats(Some("ja")), f);
    }

    #[test]
    fn formats_en_gb_and_others() {
        let f = date_formats(Some("en-GB"));
        assert_eq!(f.len(), 15);
        assert_eq!(f[0], "dd/MM/yyyy");
        assert_eq!(f[2], "d MMMM yyyy");
        assert_eq!(f[9], "dd/MM/yyyy HH:mm");
        assert_eq!(f[14], "h:mm:ss am/pm");
        assert_eq!(date_formats(Some("de-DE"))[0], "dd.MM.yyyy");
        assert_eq!(date_formats(Some("de-DE"))[1], "dddd, d. MMMM yyyy");
        assert_eq!(date_formats(Some("es-ES"))[1], "dddd, d 'de' MMMM 'de' yyyy");
        assert_eq!(date_formats(Some("es-MX"))[2], "d 'de' MMMM 'de' yyyy");
        assert_eq!(date_formats(Some("fr-FR"))[0], "dd/MM/yyyy");
        assert_eq!(date_formats(Some("nl-NL"))[0], "d-M-yyyy");
    }

    #[test]
    fn every_listed_format_renders() {
        let d = dt(2026, 10, 13, 15, 30, 45);
        for l in [None, Some("en-GB"), Some("de"), Some("es"), Some("fr"), Some("it"), Some("pt-BR"), Some("nl")] {
            for p in date_formats(l) {
                let s = date_picture(&d, p, l);
                assert!(!s.is_empty(), "{p}");
                assert!(!s.contains("yyyy") && !s.contains("MM"), "{p} gave {s}");
            }
            assert!(date_formats(l).contains(&default_date_picture(l)) || l == Some("pt-BR") || l == Some("nl"));
        }
    }

    #[test]
    fn default_pictures() {
        assert_eq!(default_date_picture(None), "M/d/yyyy");
        assert_eq!(default_time_picture(None), "h:mm am/pm");
        assert_eq!(default_date_picture(Some("en-GB")), "dd/MM/yyyy");
        assert_eq!(default_time_picture(Some("en-GB")), "HH:mm");
        assert_eq!(default_date_picture(Some("en-US")), "M/d/yyyy");
        assert_eq!(default_date_picture(Some("de-DE")), "dd.MM.yyyy");
        assert_eq!(default_time_picture(Some("de-DE")), "HH:mm");
        assert_eq!(default_date_picture(Some("nl")), "d-M-yyyy");
        assert_eq!(default_time_picture(Some("es")), "H:mm");
        assert_eq!(default_date_picture(Some("xx")), "M/d/yyyy");
    }

    // --- numeric pictures ---

    #[test]
    fn numeric_spec_examples() {
        assert_eq!(numeric_picture(1234.5, "#,##0.00"), "1,234.50");
        assert_eq!(numeric_picture(0.5, "0%"), "1%");
        assert_eq!(numeric_picture(-3.14169, "0.00"), "-3.14");
        assert_eq!(numeric_picture(5.0, "00"), "05");
        assert_eq!(numeric_picture(5.0, "#"), "5");
        assert_eq!(numeric_picture(0.0, "#"), "");
        assert_eq!(numeric_picture(12345.0, "x##"), "345");
        assert_eq!(numeric_picture(-5.0, "0;(0)"), "(5)");
        assert_eq!(numeric_picture(0.0, "0;(0);zero"), "zero");
        assert_eq!(numeric_picture(1500.0, "$#,##0.00"), "$1,500.00");
        assert_eq!(numeric_picture(1234.0, "'Total: '#,##0"), "Total: 1,234");
    }

    #[test]
    fn numeric_x_placeholder() {
        assert_eq!(numeric_picture(1234.5678, "x.##"), "4.57");
        assert_eq!(numeric_picture(7.0, "x##"), "7");
        assert_eq!(numeric_picture(12345.0, "xx"), "45");
    }

    #[test]
    fn numeric_rounding() {
        assert_eq!(numeric_picture(2.5, "0"), "3");
        assert_eq!(numeric_picture(-2.5, "0"), "-3");
        assert_eq!(numeric_picture(1.005, "0.00"), "1.01");
        assert_eq!(numeric_picture(0.995, "0.00"), "1.00");
        assert_eq!(numeric_picture(9.999, "0.00"), "10.00");
        assert_eq!(numeric_picture(99.5, "0"), "100");
        assert_eq!(numeric_picture(0.4, "0"), "0");
        assert_eq!(numeric_picture(1.0 / 3.0, "0.000"), "0.333");
        assert_eq!(numeric_picture(2.0 / 3.0, "0.00"), "0.67");
    }

    #[test]
    fn numeric_placeholders() {
        assert_eq!(numeric_picture(3.5, "0.00"), "3.50");
        assert_eq!(numeric_picture(3.5, "0.##"), "3.5");
        assert_eq!(numeric_picture(3.0, "0.##"), "3");
        assert_eq!(numeric_picture(3.25, "0.#0"), "3.25");
        assert_eq!(numeric_picture(3.0, "0.#0"), "3.00");
        assert_eq!(numeric_picture(0.5, "#.00"), ".50");
        assert_eq!(numeric_picture(0.5, "0.00"), "0.50");
        assert_eq!(numeric_picture(0.0, "0.00"), "0.00");
        assert_eq!(numeric_picture(0.0, "0"), "0");
        assert_eq!(numeric_picture(42.0, "0000"), "0042");
        assert_eq!(numeric_picture(42.0, "####"), "42");
        assert_eq!(numeric_picture(12345.0, "0"), "12345");
        assert_eq!(numeric_picture(12345.0, "00"), "12345");
        assert_eq!(numeric_picture(1234567.0, "0"), "1234567");
    }

    #[test]
    fn numeric_thousands() {
        assert_eq!(numeric_picture(1234567.891, "#,##0.00"), "1,234,567.89");
        assert_eq!(numeric_picture(999.0, "#,##0"), "999");
        assert_eq!(numeric_picture(1000.0, "#,##0"), "1,000");
        assert_eq!(numeric_picture(1234567.0, "0,0"), "1,234,567");
        assert_eq!(numeric_picture(5.0, "#,##0"), "5");
        assert_eq!(numeric_picture(-1234.5, "#,##0.0"), "-1,234.5");
        assert_eq!(numeric_picture(1_000_000.0, "#,##0"), "1,000,000");
    }

    #[test]
    fn numeric_signs() {
        assert_eq!(numeric_picture(5.0, "-0"), " 5");
        assert_eq!(numeric_picture(-5.0, "-0"), "-5");
        assert_eq!(numeric_picture(5.0, "+0"), "+5");
        assert_eq!(numeric_picture(-5.0, "+0"), "-5");
        assert_eq!(numeric_picture(0.0, "+0"), " 0");
        assert_eq!(numeric_picture(-5.0, "$0"), "-$5");
    }

    #[test]
    fn numeric_sections() {
        assert_eq!(numeric_picture(5.0, "0;(0)"), "5");
        assert_eq!(numeric_picture(0.0, "0;(0)"), "0");
        assert_eq!(numeric_picture(5.0, "0;(0);zero"), "5");
        assert_eq!(numeric_picture(-1234.5, "#,##0.00;(#,##0.00)"), "(1,234.50)");
        assert_eq!(numeric_picture(-5.0, "+0;-0"), "-5");
        assert_eq!(numeric_picture(5.0, "+0;-0"), "+5");
        assert_eq!(numeric_picture(-5.0, "0;'minus '0;none"), "minus 5");
        assert_eq!(numeric_picture(0.0, "0;0;'none'"), "none");
    }

    #[test]
    fn numeric_literals() {
        assert_eq!(numeric_picture(50.0, "0%"), "50%");
        assert_eq!(numeric_picture(12.5, "0.0' kg'"), "12.5 kg");
        assert_eq!(numeric_picture(12.0, "No. 0"), "No. 12");
        assert_eq!(numeric_picture(5551234.0, "000-0000"), "555-1234");
        assert_eq!(numeric_picture(5.0, "0\\%"), "5%");
        assert_eq!(numeric_picture(5.0, "'it''s '0"), "it's 5");
        assert_eq!(numeric_picture(5.0, "€0.00"), "€5.00");
        assert_eq!(numeric_picture(5.0, "0 USD"), "5 USD");
        assert_eq!(numeric_picture(5.0, "'a;b' 0"), "a;b 5");
    }

    #[test]
    fn numeric_no_placeholders() {
        assert_eq!(numeric_picture(5.0, "hello"), "hello");
        assert_eq!(numeric_picture(5.0, ""), "");
    }

    #[test]
    fn numeric_big_and_small() {
        assert_eq!(numeric_picture(1e15, "0"), "1000000000000000");
        assert_eq!(numeric_picture(1e-7, "0.00"), "0.00");
        assert_eq!(numeric_picture(-1e-7, "0.00"), "0.00");
        assert_eq!(numeric_picture(0.000_001_5, "0.000000"), "0.000002");
        assert_eq!(numeric_picture(123456789012.0, "#,##0"), "123,456,789,012");
    }

    #[test]
    fn numeric_non_finite() {
        assert_eq!(numeric_picture(f64::NAN, "0"), "NaN");
        assert_eq!(numeric_picture(f64::INFINITY, "0"), "inf");
    }

    // --- general_number, parse_number ---

    #[test]
    fn general_numbers() {
        assert_eq!(general_number(0.0), "0");
        assert_eq!(general_number(-0.0), "0");
        assert_eq!(general_number(5.0), "5");
        assert_eq!(general_number(-5.0), "-5");
        assert_eq!(general_number(2.50), "2.5");
        assert_eq!(general_number(0.333_333_333_333_333_3), "0.333333333333333");
        assert_eq!(general_number(1e21), "1000000000000000000000");
        assert_eq!(general_number(1234.5678), "1234.5678");
        assert_eq!(general_number(0.1 + 0.2), "0.3");
        assert_eq!(general_number(-0.25), "-0.25");
        assert_eq!(general_number(1e-5), "0.00001");
        assert_eq!(general_number(100.0), "100");
        assert_eq!(general_number(123456789012345.0), "123456789012345");
        assert_eq!(general_number(f64::NAN), "NaN");
    }

    #[test]
    fn parse_numbers() {
        assert_eq!(parse_number("42"), Some(42.0));
        assert_eq!(parse_number(" 42 "), Some(42.0));
        assert_eq!(parse_number("-3.5"), Some(-3.5));
        assert_eq!(parse_number("+3.5"), Some(3.5));
        assert_eq!(parse_number("(1,234.50)"), Some(-1234.5));
        assert_eq!(parse_number("$1,234.50"), Some(1234.5));
        assert_eq!(parse_number("-$5"), Some(-5.0));
        assert_eq!(parse_number("$-5"), Some(-5.0));
        assert_eq!(parse_number("€ 10"), Some(10.0));
        assert_eq!(parse_number("£7.25"), Some(7.25));
        assert_eq!(parse_number("50%"), Some(50.0));
        assert_eq!(parse_number("1,000,000"), Some(1_000_000.0));
        assert_eq!(parse_number(".5"), Some(0.5));
        assert_eq!(parse_number("5."), Some(5.0));
        assert_eq!(parse_number("10€"), Some(10.0));
    }

    #[test]
    fn parse_number_rejects_text() {
        for bad in ["", "abc", "$", "-", "1.2.3", ",5", "12 apples", "1e5", "()", "--5", "5,,0"] {
            assert_eq!(parse_number(bad), None, "{bad}");
        }
    }

    // --- case ---

    #[test]
    fn case_formats() {
        assert_eq!(apply_case("hello World", "Upper").as_deref(), Some("HELLO WORLD"));
        assert_eq!(apply_case("Hello World", "LOWER").as_deref(), Some("hello world"));
        assert_eq!(apply_case("hELLO wORLD", "firstcap").as_deref(), Some("Hello world"));
        assert_eq!(apply_case("hello big world", "Caps").as_deref(), Some("Hello Big World"));
        assert_eq!(apply_case("HELLO BIG WORLD", "CAPS").as_deref(), Some("Hello Big World"));
        assert_eq!(apply_case("don't stop", "Caps").as_deref(), Some("Don't Stop"));
        assert_eq!(apply_case("a-b c_d", "Caps").as_deref(), Some("A-B C_D"));
        assert_eq!(apply_case("  hello", "FirstCap").as_deref(), Some("  Hello"));
        assert_eq!(apply_case("3 apples", "FirstCap").as_deref(), Some("3 apples"));
        assert_eq!(apply_case("", "Upper").as_deref(), Some(""));
        assert_eq!(apply_case("straße", "Upper").as_deref(), Some("STRASSE"));
    }

    #[test]
    fn case_unknown_names() {
        assert_eq!(apply_case("x", "Arabic"), None);
        assert_eq!(apply_case("x", "MERGEFORMAT"), None);
        assert_eq!(apply_case("x", ""), None);
    }

    // --- number formats ---

    fn nf(v: f64, how: &str) -> String {
        number_format(v, how).unwrap()
    }

    #[test]
    fn number_formats_simple() {
        assert_eq!(nf(7.0, "Arabic"), "7");
        assert_eq!(nf(7.0, "arabic"), "7");
        assert_eq!(nf(4.0, "roman"), "iv");
        assert_eq!(nf(1994.0, "ROMAN"), "MCMXCIV");
        assert_eq!(nf(1.0, "alphabetic"), "a");
        assert_eq!(nf(27.0, "alphabetic"), "aa");
        assert_eq!(nf(28.0, "ALPHABETIC"), "BB");
        assert_eq!(nf(255.0, "Hex"), "FF");
        assert_eq!(nf(255.0, "HEX"), "FF");
        assert_eq!(nf(0.0, "Hex"), "0");
        assert_eq!(nf(2.6, "Arabic"), "3");
    }

    #[test]
    fn number_formats_case_sensitivity() {
        assert_eq!(number_format(4.0, "Roman"), None);
        assert_eq!(number_format(4.0, "rOMAN"), None);
        assert_eq!(number_format(4.0, "Alphabetic"), None);
        assert_eq!(nf(4.0, "ORDINAL"), "4th");
        assert_eq!(nf(4.0, "cardtext"), "four");
    }

    #[test]
    fn number_formats_ordinal() {
        let cases = [(1, "1st"), (2, "2nd"), (3, "3rd"), (4, "4th"), (11, "11th"), (12, "12th"), (13, "13th"), (21, "21st"), (22, "22nd"), (101, "101st"), (111, "111th")];
        for (n, s) in cases {
            assert_eq!(nf(f64::from(n), "Ordinal"), s);
        }
        assert_eq!(nf(-1.0, "Ordinal"), "-1st");
    }

    #[test]
    fn card_text() {
        let cases = [
            (0.0, "zero"),
            (1.0, "one"),
            (13.0, "thirteen"),
            (20.0, "twenty"),
            (21.0, "twenty-one"),
            (99.0, "ninety-nine"),
            (100.0, "one hundred"),
            (101.0, "one hundred one"),
            (123.0, "one hundred twenty-three"),
            (1000.0, "one thousand"),
            (1001.0, "one thousand one"),
            (12_345.0, "twelve thousand three hundred forty-five"),
            (100_000.0, "one hundred thousand"),
            (1_000_000.0, "one million"),
            (1_234_567.0, "one million two hundred thirty-four thousand five hundred sixty-seven"),
            (1_000_000_000.0, "one billion"),
            (999_999_999_999.0, "nine hundred ninety-nine billion nine hundred ninety-nine million nine hundred ninety-nine thousand nine hundred ninety-nine"),
            (-5.0, "minus five"),
        ];
        for (v, s) in cases {
            assert_eq!(nf(v, "CardText"), s, "{v}");
        }
        assert_eq!(nf(2.5, "cardtext"), "three");
    }

    #[test]
    fn ord_text() {
        let cases = [
            (0.0, "zeroth"),
            (1.0, "first"),
            (2.0, "second"),
            (3.0, "third"),
            (4.0, "fourth"),
            (5.0, "fifth"),
            (8.0, "eighth"),
            (9.0, "ninth"),
            (12.0, "twelfth"),
            (13.0, "thirteenth"),
            (20.0, "twentieth"),
            (21.0, "twenty-first"),
            (23.0, "twenty-third"),
            (40.0, "fortieth"),
            (100.0, "one hundredth"),
            (101.0, "one hundred first"),
            (1000.0, "one thousandth"),
            (1_000_000.0, "one millionth"),
            (-1.0, "minus first"),
        ];
        for (v, s) in cases {
            assert_eq!(nf(v, "OrdText"), s, "{v}");
        }
    }

    #[test]
    fn dollar_text() {
        assert_eq!(nf(123.45, "DollarText"), "one hundred twenty-three and 45/100");
        assert_eq!(nf(0.0, "DollarText"), "zero and 00/100");
        assert_eq!(nf(5.0, "dollartext"), "five and 00/100");
        assert_eq!(nf(1.5, "DollarText"), "one and 50/100");
        assert_eq!(nf(0.07, "DollarText"), "zero and 07/100");
        assert_eq!(nf(1000.999, "DollarText"), "one thousand one and 00/100");
        assert_eq!(nf(0.285, "DollarText"), "zero and 29/100");
        assert_eq!(nf(-12.34, "DollarText"), "minus twelve and 34/100");
    }

    #[test]
    fn not_number_formats() {
        for n in ["CHARFORMAT", "MERGEFORMAT", "Upper", "Caps", "", "Bogus"] {
            assert_eq!(number_format(1.0, n), None, "{n}");
        }
    }
}
