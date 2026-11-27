//! The CSL processor against real styles from the Zotero style repository (CC BY-SA 3.0, see
//! THIRD_PARTY_NOTICES.md), with sources of each kind.

use docxtor_core::citations::{Name, Source, SourceType};
use docxtor_core::csl::{process, CiteItem, CiteSite, Output, Position, Style};

fn style(file: &str) -> Style {
    let path = format!("{}/../../web/src/csl/{file}", env!("CARGO_MANIFEST_DIR"));
    Style::parse(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn p(family: &str, given: &str) -> Name {
    Name::person(family, given)
}

fn book() -> Source {
    Source {
        tag: "Smi20".into(),
        kind: SourceType::Book,
        authors: vec![p("Smith", "John Q.")],
        title: "The Art of Citing: A Practical Guide".into(),
        year: Some("2020".into()),
        publisher: Some("Penguin".into()),
        place: Some("New York".into()),
        ..Default::default()
    }
}

fn article() -> Source {
    Source {
        tag: "Lee21".into(),
        kind: SourceType::Article,
        authors: vec![p("Lee", "Karen"), p("Kim", "Min-Jun")],
        title: "A study of things".into(),
        container_title: Some("Journal of Stuff".into()),
        year: Some("2021".into()),
        month: Some("March".into()),
        volume: Some("12".into()),
        issue: Some("3".into()),
        pages: Some("45-67".into()),
        doi: Some("10.1000/xyz123".into()),
        ..Default::default()
    }
}

fn chapter() -> Source {
    Source {
        tag: "Ada19".into(),
        kind: SourceType::Chapter,
        authors: vec![p("Adams", "Rob")],
        editors: vec![p("Baker", "Ed"), p("Cole", "Fay")],
        title: "On chapters".into(),
        container_title: Some("Book of Essays".into()),
        year: Some("2019".into()),
        publisher: Some("Routledge".into()),
        place: Some("London".into()),
        pages: Some("12-34".into()),
        ..Default::default()
    }
}

fn webpage() -> Source {
    Source {
        tag: "Ric23".into(),
        kind: SourceType::Website,
        authors: vec![p("Rice", "Anne")],
        title: "How to cite a web page".into(),
        container_title: Some("Citation Weekly".into()),
        year: Some("2023".into()),
        month: Some("May".into()),
        day: Some("2".into()),
        url: Some("https://example.com/cite".into()),
        accessed: Some("2024-01-15".into()),
        ..Default::default()
    }
}

fn many() -> Source {
    Source {
        tag: "Alp22".into(),
        kind: SourceType::Article,
        authors: ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta"].iter().map(|f| p(f, "Ann")).collect(),
        title: "Many hands".into(),
        container_title: Some("Teamwork Review".into()),
        year: Some("2022".into()),
        volume: Some("5".into()),
        pages: Some("100-110".into()),
        ..Default::default()
    }
}

fn all() -> (Source, Source, Source, Source, Source) {
    (book(), article(), chapter(), webpage(), many())
}

fn cite<'a>(s: &'a Source, locator: Option<&str>) -> CiteSite<'a> {
    CiteSite { items: vec![CiteItem { locator: locator.map(String::from), ..CiteItem::new(s) }], note: None }
}

fn note<'a>(s: &'a Source, locator: Option<&str>, n: usize) -> CiteSite<'a> {
    CiteSite { items: vec![CiteItem { locator: locator.map(String::from), ..CiteItem::new(s) }], note: Some(n) }
}

fn cites(o: &Output) -> Vec<String> {
    o.citations.iter().map(|r| r.marked()).collect()
}

fn bib(o: &Output) -> Vec<String> {
    o.bibliography.iter().map(|e| e.text.marked()).collect()
}

#[test]
fn vancouver_numbers_and_entries() {
    let (b, a, c, w, m) = all();
    let o = process(&style("elsevier-vancouver.csl"), &[cite(&b, None), cite(&a, None), cite(&c, None), cite(&w, None), cite(&m, None), cite(&b, None)]);
    assert_eq!(cites(&o), ["[1]", "[2]", "[3]", "[4]", "[5]", "[1]"]);
    assert_eq!(
        bib(&o),
        [
            "[1]\tSmith JQ. The Art of Citing: A Practical Guide. New York: Penguin; 2020.",
            "[2]\tLee K, Kim M-J. A study of things. Journal of Stuff 2021;12:45\u{2013}67. https://doi.org/10.1000/xyz123.",
            "[3]\tAdams R. On chapters. In: Baker E, Cole F, editors. Book of Essays, London: Routledge; 2019, p. 12\u{2013}34.",
            "[4]\tRice A. How to cite a web page. Citation Weekly 2023. https://example.com/cite (accessed January 15, 2024).",
            // Seven or more authors: the first six and "et al.".
            "[5]\tAlpha A, Beta A, Gamma A, Delta A, Epsilon A, Zeta A, et al. Many hands. Teamwork Review 2022;5:100\u{2013}10.",
        ]
    );
}

#[test]
fn numbers_collapse_into_ranges() {
    let (b, a, c, w, _) = all();
    let multi = CiteSite { items: vec![CiteItem::new(&w), CiteItem::new(&b), CiteItem::new(&a), CiteItem::new(&c)], note: None };
    // Numbers come from the order of first citation: b=1, a=2, c=3, w=4.
    let first = [cite(&b, None), cite(&a, None), cite(&c, None), cite(&w, None), multi];
    let o = process(&style("elsevier-vancouver.csl"), &first);
    assert_eq!(o.citations[4].plain(), "[1\u{2013}4]", "four in a row collapse to a range");
    let nature = process(&style("nature.csl"), &first);
    assert_eq!(nature.citations[4].marked(), "^1\u{2013}4^");
    let gap = CiteSite { items: vec![CiteItem::new(&b), CiteItem::new(&c), CiteItem::new(&w)], note: None };
    let o = process(&style("elsevier-vancouver.csl"), &[cite(&b, None), cite(&a, None), cite(&c, None), cite(&w, None), gap]);
    assert_eq!(o.citations[4].plain(), "[1,3,4]", "a run of two stays a list");
}

#[test]
fn nature_marks_superscripts_italics_and_bold() {
    let (b, a, c, w, m) = all();
    let o = process(&style("nature.csl"), &[cite(&b, None), cite(&a, None), cite(&c, None), cite(&w, None), cite(&m, None)]);
    assert_eq!(cites(&o), ["^1^", "^2^", "^3^", "^4^", "^5^"]);
    assert_eq!(
        bib(&o),
        [
            "1.\tSmith, J. Q. *The Art of Citing: A Practical Guide*. (Penguin, New York, 2020).",
            "2.\tLee, K. & Kim, M.-J. A study of things. *Journal of Stuff* **12**, 45\u{2013}67 (2021).",
            "3.\tAdams, R. On chapters. in *Book of Essays* (eds. Baker, E. & Cole, F.) 12\u{2013}34 (Routledge, London, 2019).",
            "4.\tRice, A. How to cite a web page. *Citation Weekly* https://example.com/cite (2023).",
            "5.\tAlpha, A. *et al.* Many hands. *Teamwork Review* **5**, 100\u{2013}110 (2022).",
        ]
    );
}

#[test]
fn ama_puts_the_locator_after_the_number() {
    let (b, a, c, w, m) = all();
    let o = process(&style("american-medical-association.csl"), &[cite(&b, None), cite(&a, Some("46")), cite(&c, None), cite(&w, None), cite(&m, None), cite(&b, Some("23"))]);
    assert_eq!(cites(&o), ["^1^", "^2(p46)^", "^3^", "^4^", "^5^", "^1(p23)^"]);
    assert_eq!(
        bib(&o),
        [
            "1.\tSmith JQ. *The Art of Citing: A Practical Guide*. Penguin; 2020.",
            "2.\tLee K, Kim MJ. A study of things. *Journal of Stuff*. 2021;12(3):45-67. doi:10.1000/xyz123",
            "3.\tAdams R. On chapters. In: Baker E, Cole F, eds. *Book of Essays*. Routledge; 2019:12-34.",
            "4.\tRice A. How to cite a web page. Citation Weekly. May 2, 2023. Accessed January 15, 2024. https://example.com/cite",
            "5.\tAlpha A, Beta A, Gamma A, et al. Many hands. *Teamwork Review*. 2022;5:100-110.",
        ]
    );
}

#[test]
fn chicago_16_notes_first_subsequent_and_ibid() {
    let (b, a, c, w, m) = all();
    let sites = [note(&b, None, 1), note(&a, Some("46"), 2), note(&c, None, 3), note(&w, None, 4), note(&m, None, 5), note(&b, Some("23"), 6), note(&b, Some("23"), 7), note(&b, Some("24"), 8), note(&b, None, 9), note(&a, None, 10)];
    let o = process(&style("chicago-notes-bibliography-16th-edition.csl"), &sites);
    assert_eq!(
        cites(&o),
        [
            "John Q. Smith, *The Art of Citing: A Practical Guide* (New York: Penguin, 2020).",
            "Karen Lee and Min-Jun Kim, \u{201c}A Study of Things,\u{201d} *Journal of Stuff* 12, no. 3 (March 2021): 46, doi:10.1000/xyz123.",
            "Rob Adams, \u{201c}On Chapters,\u{201d} in *Book of Essays*, ed. Ed Baker and Fay Cole (London: Routledge, 2019), 12\u{2013}34.",
            "Anne Rice, \u{201c}How to Cite a Web Page,\u{201d} *Citation Weekly*, May 2, 2023, https://example.com/cite.",
            "Ann Alpha et al., \u{201c}Many Hands,\u{201d} *Teamwork Review* 5 (2022): 100\u{2013}110.",
            // Cited again after other sources: the short form, with the page.
            "Smith, *The Art of Citing*, 23.",
            // The very next note, same page.
            "Ibid.",
            // The next note, another page.
            "Ibid., 24.",
            // The previous note had a page and this one has none, so it is not "ibid".
            "Smith, *The Art of Citing*.",
            "Lee and Kim, \u{201c}A Study of Things.\u{201d}",
        ]
    );
    assert_eq!(o.positions[0], [Position::First]);
    assert_eq!(o.positions[5], [Position::Subsequent]);
    assert_eq!(o.positions[6], [Position::Ibid]);
    assert_eq!(o.positions[7], [Position::IbidWithLocator]);
    assert_eq!(o.positions[8], [Position::Subsequent]);
    assert_eq!(
        bib(&o),
        [
            "Adams, Rob. \u{201c}On Chapters.\u{201d} In *Book of Essays*, edited by Ed Baker and Fay Cole, 12\u{2013}34. London: Routledge, 2019.",
            "Alpha, Ann, Ann Beta, Ann Gamma, Ann Delta, Ann Epsilon, Ann Zeta, Ann Eta, and Ann Theta. \u{201c}Many Hands.\u{201d} *Teamwork Review* 5 (2022): 100\u{2013}110.",
            "Lee, Karen, and Min-Jun Kim. \u{201c}A Study of Things.\u{201d} *Journal of Stuff* 12, no. 3 (March 2021): 45\u{2013}67. doi:10.1000/xyz123.",
            "Rice, Anne. \u{201c}How to Cite a Web Page.\u{201d} *Citation Weekly*, May 2, 2023. https://example.com/cite.",
            "Smith, John Q. *The Art of Citing: A Practical Guide*. New York: Penguin, 2020.",
        ]
    );
}

#[test]
fn chicago_18_has_no_ibid_and_drops_the_place() {
    let (b, a, ..) = all();
    let o = process(&style("chicago-notes-bibliography.csl"), &[note(&b, None, 1), note(&a, None, 2), note(&b, Some("23"), 3), note(&b, Some("23"), 4)]);
    assert_eq!(
        cites(&o),
        [
            "John Q. Smith, *The Art of Citing: A Practical Guide* (Penguin, 2020).",
            "Karen Lee and Min-Jun Kim, \u{201c}A Study of Things,\u{201d} *Journal of Stuff* 12, no. 3 (2021): 45\u{2013}67, https://doi.org/10.1000/xyz123.",
            "Smith, *The Art of Citing*, 23.",
            "Smith, *The Art of Citing*, 23.",
        ]
    );
    assert_eq!(o.positions[3], [Position::Ibid]);
}

#[test]
fn page_ranges_follow_the_style() {
    let (b, a, ..) = all();
    let mut long = a.clone();
    long.pages = Some("321-328".into());
    let o = process(&style("chicago-notes-bibliography-16th-edition.csl"), &[note(&b, Some("101-108"), 1), note(&long, None, 2)]);
    assert!(o.citations[0].plain().ends_with("2020), 101\u{2013}8."), "{}", o.citations[0].plain());
    assert!(o.citations[1].plain().contains(": 321\u{2013}28,"), "{}", o.citations[1].plain());
}

#[test]
fn several_sources_in_one_note_are_joined() {
    let (b, a, ..) = all();
    let both = CiteSite { items: vec![CiteItem::new(&b), CiteItem::new(&a)], note: Some(1) };
    let o = process(&style("chicago-notes-bibliography-16th-edition.csl"), &[both]);
    assert_eq!(
        o.citations[0].plain(),
        "John Q. Smith, The Art of Citing: A Practical Guide (New York: Penguin, 2020); Karen Lee and Min-Jun Kim, \u{201c}A Study of Things,\u{201d} Journal of Stuff 12, no. 3 (March 2021): 45\u{2013}67, doi:10.1000/xyz123."
    );
}

/// A small author-date style, to test sorting, year suffixes and the repeated-author dash.
const AUTHOR_DATE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0" default-locale="en-US">
  <info><title>Small author-date</title><id>http://example.com/small</id><category citation-format="author-date"/></info>
  <macro name="author">
    <names variable="author">
      <name name-as-sort-order="all" and="symbol" initialize-with=". " delimiter=", " sort-separator=", "/>
      <substitute><text variable="title"/></substitute>
    </names>
  </macro>
  <macro name="year"><date variable="issued"><date-part name="year"/></date></macro>
  <citation disambiguate-add-year-suffix="true">
    <sort><key macro="author"/><key macro="year"/></sort>
    <layout prefix="(" suffix=")" delimiter="; ">
      <group delimiter=", ">
        <names variable="author"><name form="short" and="symbol" delimiter=", "/><substitute><text variable="title" form="short"/></substitute></names>
        <group delimiter="">
          <text macro="year"/>
          <text variable="year-suffix"/>
        </group>
        <group delimiter=" "><label variable="locator" form="short"/><text variable="locator"/></group>
      </group>
    </layout>
  </citation>
  <bibliography subsequent-author-substitute="&#8212;&#8212;&#8212;" hanging-indent="true">
    <sort><key macro="author"/><key macro="year"/><key variable="title"/></sort>
    <layout suffix=".">
      <group delimiter=" ">
        <text macro="author" suffix="."/>
        <group delimiter="">
          <text macro="year" prefix="("/>
          <text variable="year-suffix"/>
          <text value=")."/>
        </group>
        <text variable="title" font-style="italic"/>
      </group>
    </layout>
  </bibliography>
</style>"#;

#[test]
fn author_date_with_year_suffixes_and_sorting() {
    let s = Style::parse(AUTHOR_DATE).unwrap();
    let (b, a, ..) = all();
    let mut b2 = b.clone();
    b2.tag = "Smi20b".into();
    b2.title = "Another book".into();
    let o = process(&s, &[cite(&a, Some("46")), cite(&b, None), cite(&b2, None), CiteSite { items: vec![CiteItem::new(&b2), CiteItem::new(&a)], note: None }]);
    // Two works by Smith in 2020 get letters in the order of the bibliography (by title).
    assert_eq!(cites(&o), ["(Lee & Kim, 2021, p. 46)", "(Smith, 2020b)", "(Smith, 2020a)", "(Lee & Kim, 2021; Smith, 2020a)"]);
    assert_eq!(
        bib(&o),
        [
            "Lee, K. & Kim, M.-J. (2021). *A study of things*.",
            "Smith, J. Q. (2020a). *Another book*.",
            "\u{2014}\u{2014}\u{2014} (2020b). *The Art of Citing: A Practical Guide*.",
        ]
    );
    assert_eq!(s.bibliography_indent(), (720, 720));
    assert!(!s.is_note() && !s.is_numeric());
}

#[test]
fn a_style_that_is_not_csl_is_refused_in_plain_words() {
    assert!(Style::parse("not xml at all <").unwrap_err().contains("not a CSL style"));
    assert!(Style::parse("<?xml version=\"1.0\"?><html/>").unwrap_err().contains("does not start with a style element"));
    assert!(Style::parse("<style xmlns=\"http://purl.org/net/xbiblio/csl\" version=\"1.0\"><info/></style>").unwrap_err().contains("no citation"));
}

#[test]
fn styles_report_what_they_are() {
    let n = style("chicago-notes-bibliography-16th-edition.csl");
    assert!(n.is_note());
    assert_eq!(n.title, "Chicago Manual of Style 16th edition (notes and bibliography)");
    assert_eq!(n.bibliography_indent(), (720, 720));
    let v = style("elsevier-vancouver.csl");
    assert!(v.is_numeric() && !v.is_note());
    assert_eq!(v.bibliography_indent(), (540, 540));
}
