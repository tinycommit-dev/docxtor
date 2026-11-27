//! Tests of the citation styles.

use super::*;

fn book() -> Source {
    Source {
        tag: "Smi20".into(),
        kind: SourceType::Book,
        authors: vec![Name::person("Smith", "John Q.")],
        title: "The Art of Citing".into(),
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
        authors: vec![Name::person("Lee", "Karen"), Name::person("Kim", "Min-Jun")],
        title: "A study of things".into(),
        container_title: Some("Journal of Stuff".into()),
        year: Some("2021".into()),
        month: Some("3".into()),
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
        authors: vec![Name::person("Adams", "Ruth")],
        editors: vec![Name::person("Baker", "Ed"), Name::person("Cole", "Fran")],
        title: "On chapters".into(),
        container_title: Some("Book of Essays".into()),
        year: Some("2019".into()),
        pages: Some("12-34".into()),
        publisher: Some("Routledge".into()),
        place: Some("London".into()),
        ..Default::default()
    }
}

fn conference() -> Source {
    Source {
        tag: "Poe18".into(),
        kind: SourceType::Conference,
        authors: vec![Name::person("Poe", "Edgar")],
        title: "Ravens at scale".into(),
        container_title: Some("Proceedings of the Gothic Conference".into()),
        year: Some("2018".into()),
        pages: Some("1-10".into()),
        publisher: Some("ACM".into()),
        place: Some("Boston".into()),
        ..Default::default()
    }
}

fn report() -> Source {
    Source {
        tag: "Wor20".into(),
        kind: SourceType::Report,
        authors: vec![Name::Literal("World Health Organization".into())],
        title: "Global health report".into(),
        year: Some("2020".into()),
        publisher: Some("World Health Organization".into()),
        place: Some("Geneva".into()),
        genre: Some("Technical report".into()),
        ..Default::default()
    }
}

fn website() -> Source {
    Source {
        tag: "Ric22".into(),
        kind: SourceType::Website,
        authors: vec![Name::person("Rice", "Anne")],
        title: "How to cite a web page".into(),
        container_title: Some("Citation Weekly".into()),
        year: Some("2022".into()),
        month: Some("May".into()),
        day: Some("9".into()),
        url: Some("https://example.com/cite".into()),
        accessed: Some("2023-01-15".into()),
        ..Default::default()
    }
}

fn thesis() -> Source {
    Source {
        tag: "Tur15".into(),
        kind: SourceType::Thesis,
        authors: vec![Name::person("Turing", "Alan")],
        title: "Computable numbers".into(),
        year: Some("2015".into()),
        publisher: Some("University of Cambridge".into()),
        place: Some("Cambridge".into()),
        genre: Some("Doctoral dissertation".into()),
        ..Default::default()
    }
}

fn misc() -> Source {
    Source { tag: "Dat20".into(), kind: SourceType::Misc, authors: vec![Name::person("Data", "Dana")], title: "Open dataset".into(), year: Some("2020".into()), publisher: Some("Zenodo".into()), ..Default::default() }
}

fn entry(style: Style, s: &Source) -> String {
    let cited = [s];
    Plan::new(style, &cited).reference(s).marked()
}

fn cite(style: Style, srcs: &[&Source], page: Option<&str>) -> String {
    let plan = Plan::new(style, srcs);
    let items: Vec<Item> = srcs.iter().enumerate().map(|(i, s)| Item { page: if i == 0 { page } else { None }, ..Item::new(s) }).collect();
    plan.in_text(&items).marked()
}

#[test]
fn initials_work() {
    assert_eq!(initials("John Q."), "J. Q.");
    assert_eq!(initials("Jean-Paul"), "J.-P.");
    assert_eq!(initials("J.K."), "J. K.");
    assert_eq!(initials("JK"), "J. K.");
    assert_eq!(initials("Ludwig van"), "L. van");
    assert_eq!(initials(""), "");
    assert_eq!(initials("Élodie"), "É.");
}

#[test]
fn page_helpers() {
    assert_eq!(pages_with("45-67", "\u{2013}"), "45\u{2013}67");
    assert_eq!(pages_with("45 -- 67", "-"), "45-67");
    assert_eq!(p_label("23", "-"), "p. 23");
    assert_eq!(p_label("23-25", "-"), "pp. 23-25");
    assert_eq!(p_label("e1234", "-"), "e1234");
    assert_eq!(edition_base("2"), "2nd");
    assert_eq!(edition_base("3rd edition"), "3rd");
    assert_eq!(edition_base("Revised"), "Revised");
    assert_eq!(edition_base("11"), "11th");
    assert_eq!(edition_base("22"), "22nd");
    assert_eq!(doi_of("https://doi.org/10.1/x"), "10.1/x");
    assert_eq!(doi_of("doi:10.1/x"), "10.1/x");
}

// ----- APA -----------------------------------------------------------

#[test]
fn apa_references() {
    assert_eq!(entry(Style::Apa, &book()), "Smith, J. Q. (2020). *The Art of Citing*. Penguin.");
    assert_eq!(
        entry(Style::Apa, &article()),
        "Lee, K., & Kim, M.-J. (2021). A study of things. *Journal of Stuff, 12*(3), 45\u{2013}67. https://doi.org/10.1000/xyz123"
    );
    assert_eq!(
        entry(Style::Apa, &chapter()),
        "Adams, R. (2019). On chapters. In E. Baker & F. Cole (Eds.), *Book of Essays* (pp. 12\u{2013}34). Routledge."
    );
    assert_eq!(entry(Style::Apa, &conference()), "Poe, E. (2018). Ravens at scale. In *Proceedings of the Gothic Conference* (pp. 1\u{2013}10). ACM.");
    assert_eq!(entry(Style::Apa, &report()), "World Health Organization. (2020). *Global health report* (Technical report).");
    assert_eq!(entry(Style::Apa, &website()), "Rice, A. (2022, May 9). *How to cite a web page*. Citation Weekly. https://example.com/cite");
    assert_eq!(entry(Style::Apa, &thesis()), "Turing, A. (2015). *Computable numbers* [Doctoral dissertation, University of Cambridge].");
    assert_eq!(entry(Style::Apa, &misc()), "Data, D. (2020). *Open dataset*. Zenodo.");
}

#[test]
fn apa_edition_and_no_author() {
    let mut b = book();
    b.edition = Some("2".into());
    assert_eq!(entry(Style::Apa, &b), "Smith, J. Q. (2020). *The Art of Citing* (2nd ed.). Penguin.");
    b.authors.clear();
    assert_eq!(entry(Style::Apa, &b), "*The Art of Citing* (2nd ed.). (2020). Penguin.");
    let mut e = book();
    e.authors.clear();
    e.editors = vec![Name::person("Baker", "Ed")];
    assert_eq!(entry(Style::Apa, &e), "Baker, E. (Ed.). (2020). *The Art of Citing*. Penguin.");
}

#[test]
fn apa_missing_fields_degrade() {
    let s = Source { tag: "x".into(), kind: SourceType::Article, title: "Only a title".into(), ..Default::default() };
    let out = entry(Style::Apa, &s);
    assert_eq!(out, "Only a title. (n.d.).");
    assert!(!out.contains("undefined") && !out.contains("()"));
    let s = Source { tag: "x".into(), kind: SourceType::Book, authors: vec![Name::person("Roe", "")], ..Default::default() };
    assert_eq!(entry(Style::Apa, &s), "Roe. (n.d.).");
}

#[test]
fn apa_author_lists() {
    let n = |f: &str, g: &str| Name::person(f, g);
    assert_eq!(apa_authors(&[n("A", "Al"), n("B", "Bo")]), "A, A., & B, B.");
    assert_eq!(apa_authors(&[n("A", "Al"), n("B", "Bo"), n("C", "Cy")]), "A, A., B, B., & C, C.");
    let many: Vec<Name> = (1..=21).map(|i| n(&format!("N{i}"), "X")).collect();
    let s = apa_authors(&many);
    assert!(s.starts_with("N1, X., N2, X."), "{s}");
    assert!(s.ends_with("N19, X., . . . N21, X."), "{s}");
    let twenty: Vec<Name> = (1..=20).map(|i| n(&format!("N{i}"), "X")).collect();
    assert!(apa_authors(&twenty).ends_with(", & N20, X."));
}

#[test]
fn apa_in_text() {
    let (b, a) = (book(), article());
    assert_eq!(cite(Style::Apa, &[&b], None), "(Smith, 2020)");
    assert_eq!(cite(Style::Apa, &[&b], Some("23")), "(Smith, 2020, p. 23)");
    assert_eq!(cite(Style::Apa, &[&b], Some("23-25")), "(Smith, 2020, pp. 23\u{2013}25)");
    assert_eq!(cite(Style::Apa, &[&a], None), "(Lee & Kim, 2021)");
    let mut three = b.clone();
    three.authors = vec![Name::person("Smith", "J"), Name::person("Lee", "K"), Name::person("Kim", "M")];
    assert_eq!(cite(Style::Apa, &[&three], None), "(Smith et al., 2020)");
    assert_eq!(cite(Style::Apa, &[&b, &a], None), "(Smith, 2020; Lee & Kim, 2021)");
    let mut nd = b.clone();
    nd.year = None;
    assert_eq!(cite(Style::Apa, &[&nd], None), "(Smith, n.d.)");
}

#[test]
fn apa_in_text_suppress_and_title() {
    let b = book();
    let plan = Plan::new(Style::Apa, &[&b]);
    let item = |f: &dyn Fn(&mut Item)| {
        let mut i = Item::new(&b);
        f(&mut i);
        plan.in_text(&[i]).marked()
    };
    assert_eq!(item(&|i| i.no_author = true), "(2020)");
    assert_eq!(item(&|i| i.no_year = true), "(Smith)");
    assert_eq!(item(&|i| {
        i.no_author = true;
        i.no_year = true
    }), "(Smith, 2020)", "everything suppressed falls back");
    let mut anon = b.clone();
    anon.authors.clear();
    assert_eq!(cite(Style::Apa, &[&anon], None), "(*The Art of Citing*, 2020)");
    let mut art = article();
    art.authors.clear();
    assert_eq!(cite(Style::Apa, &[&art], None), "(\"A study of things\", 2021)");
}

#[test]
fn apa_disambiguation() {
    let mut a = book();
    a.tag = "S1".into();
    a.title = "Zebras".into();
    let mut b = book();
    b.tag = "S2".into();
    b.title = "Aardvarks".into();
    let plan = Plan::new(Style::Apa, &[&a, &b]);
    assert_eq!(plan.in_text(&[Item::new(&a)]).plain(), "(Smith, 2020b)");
    assert_eq!(plan.in_text(&[Item::new(&b)]).plain(), "(Smith, 2020a)");
    assert_eq!(plan.reference(&b).plain(), "Smith, J. Q. (2020a). Aardvarks. Penguin.");
    // Sorted by title when author and year match.
    assert_eq!(plan.entries()[0].tag, "S2");
    // Different first authors don't clash.
    let c = article();
    let plan = Plan::new(Style::Apa, &[&a, &c]);
    assert_eq!(plan.in_text(&[Item::new(&a)]).plain(), "(Smith, 2020)");
}

// ----- MLA -----------------------------------------------------------

#[test]
fn mla_references() {
    assert_eq!(entry(Style::Mla, &book()), "Smith, John Q. *The Art of Citing*. Penguin, 2020.");
    assert_eq!(
        entry(Style::Mla, &article()),
        "Lee, Karen, and Min-Jun Kim. \"A study of things.\" *Journal of Stuff*, vol. 12, no. 3, Mar. 2021, pp. 45-67, https://doi.org/10.1000/xyz123."
    );
    assert_eq!(
        entry(Style::Mla, &chapter()),
        "Adams, Ruth. \"On chapters.\" *Book of Essays*, edited by Ed Baker and Fran Cole, Routledge, 2019, pp. 12-34."
    );
    assert_eq!(entry(Style::Mla, &conference()), "Poe, Edgar. \"Ravens at scale.\" *Proceedings of the Gothic Conference*, ACM, 2018, pp. 1-10.");
    assert_eq!(entry(Style::Mla, &report()), "World Health Organization. *Global health report*. Technical report, World Health Organization, 2020.");
    assert_eq!(entry(Style::Mla, &website()), "Rice, Anne. \"How to cite a web page.\" *Citation Weekly*, 9 May 2022, example.com/cite. Accessed 15 Jan. 2023.");
    assert_eq!(entry(Style::Mla, &thesis()), "Turing, Alan. \"Computable numbers.\" 2015. University of Cambridge, PhD dissertation.");
    assert_eq!(entry(Style::Mla, &misc()), "Data, Dana. *Open dataset*. Zenodo, 2020.");
}

#[test]
fn mla_authors_and_edition() {
    let mut b = book();
    b.edition = Some("2nd".into());
    assert_eq!(entry(Style::Mla, &b), "Smith, John Q. *The Art of Citing*. 2nd ed., Penguin, 2020.");
    b.authors = vec![Name::person("Smith", "John"), Name::person("Lee", "Karen"), Name::person("Kim", "Min")];
    assert_eq!(entry(Style::Mla, &b), "Smith, John, et al. *The Art of Citing*. 2nd ed., Penguin, 2020.");
    b.authors.clear();
    assert_eq!(entry(Style::Mla, &b), "*The Art of Citing*. 2nd ed., Penguin, 2020.");
}

#[test]
fn mla_in_text() {
    let (b, a) = (book(), article());
    assert_eq!(cite(Style::Mla, &[&b], None), "(Smith)");
    assert_eq!(cite(Style::Mla, &[&b], Some("23")), "(Smith 23)");
    assert_eq!(cite(Style::Mla, &[&a], Some("50-51")), "(Lee and Kim 50-51)");
    let mut three = b.clone();
    three.authors = vec![Name::person("Smith", "J"), Name::person("Lee", "K"), Name::person("Kim", "M")];
    assert_eq!(cite(Style::Mla, &[&three], Some("7")), "(Smith et al. 7)");
    assert_eq!(cite(Style::Mla, &[&b, &a], None), "(Smith; Lee and Kim)");
    // Two works by the same author need the title.
    let mut other = book();
    other.tag = "Smi21".into();
    other.title = "Citing: A sequel".into();
    let plan = Plan::new(Style::Mla, &[&b, &other]);
    assert_eq!(plan.in_text(&[Item { page: Some("4"), ..Item::new(&b) }]).marked(), "(Smith, *The Art of Citing* 4)");
    assert_eq!(plan.in_text(&[Item::new(&other)]).marked(), "(Smith, *Citing*)");
    let mut anon = b.clone();
    anon.authors.clear();
    assert_eq!(cite(Style::Mla, &[&anon], Some("3")), "(*The Art of Citing* 3)");
    assert_eq!(Plan::new(Style::Mla, &[&b]).in_text(&[Item { no_author: true, page: Some("9"), ..Item::new(&b) }]).plain(), "(9)");
}

// ----- Chicago -------------------------------------------------------

#[test]
fn chicago_references() {
    assert_eq!(entry(Style::Chicago, &book()), "Smith, John Q. 2020. *The Art of Citing*. New York: Penguin.");
    assert_eq!(
        entry(Style::Chicago, &article()),
        "Lee, Karen, and Min-Jun Kim. 2021. \"A study of things.\" *Journal of Stuff* 12 (3): 45\u{2013}67. https://doi.org/10.1000/xyz123."
    );
    assert_eq!(
        entry(Style::Chicago, &chapter()),
        "Adams, Ruth. 2019. \"On chapters.\" In *Book of Essays*, edited by Ed Baker and Fran Cole, 12\u{2013}34. London: Routledge."
    );
    assert_eq!(entry(Style::Chicago, &conference()), "Poe, Edgar. 2018. \"Ravens at scale.\" In *Proceedings of the Gothic Conference*, 1\u{2013}10. Boston: ACM.");
    assert_eq!(entry(Style::Chicago, &report()), "World Health Organization. 2020. *Global health report*. Technical report. Geneva: World Health Organization.");
    assert_eq!(entry(Style::Chicago, &website()), "Rice, Anne. 2022. \"How to cite a web page.\" Citation Weekly. May 9, 2022. Accessed January 15, 2023. https://example.com/cite.");
    assert_eq!(entry(Style::Chicago, &thesis()), "Turing, Alan. 2015. \"Computable numbers.\" PhD diss., University of Cambridge.");
}

#[test]
fn chicago_in_text() {
    let (b, a) = (book(), article());
    assert_eq!(cite(Style::Chicago, &[&b], None), "(Smith 2020)");
    assert_eq!(cite(Style::Chicago, &[&b], Some("23")), "(Smith 2020, 23)");
    assert_eq!(cite(Style::Chicago, &[&a], None), "(Lee and Kim 2021)");
    let mut three = b.clone();
    three.authors = vec![Name::person("Smith", "J"), Name::person("Lee", "K"), Name::person("Kim", "M")];
    assert_eq!(cite(Style::Chicago, &[&three], None), "(Smith, Lee, and Kim 2020)");
    three.authors.push(Name::person("Doe", "D"));
    assert_eq!(cite(Style::Chicago, &[&three], None), "(Smith et al. 2020)");
    assert_eq!(cite(Style::Chicago, &[&b, &a], None), "(Smith 2020; Lee and Kim 2021)");
    let plan = Plan::new(Style::Chicago, &[&b]);
    assert_eq!(plan.in_text(&[Item { no_author: true, page: Some("5"), ..Item::new(&b) }]).plain(), "(2020, 5)");
    assert_eq!(plan.in_text(&[Item { no_year: true, page: Some("5"), ..Item::new(&b) }]).plain(), "(Smith, 5)");
}

// ----- IEEE ----------------------------------------------------------

#[test]
fn ieee_references() {
    let s = [book(), article(), chapter(), conference(), report(), website(), thesis(), misc()];
    let refs: Vec<&Source> = s.iter().collect();
    let plan = Plan::new(Style::Ieee, &refs);
    let get = |i: usize| plan.reference(&s[i]).marked();
    assert_eq!(get(0), "[1]\tJ. Q. Smith, *The Art of Citing*. New York: Penguin, 2020.");
    assert_eq!(get(1), "[2]\tK. Lee and M.-J. Kim, \"A study of things,\" *Journal of Stuff*, vol. 12, no. 3, pp. 45\u{2013}67, Mar. 2021, doi: 10.1000/xyz123.");
    assert_eq!(get(2), "[3]\tR. Adams, \"On chapters,\" in *Book of Essays*, E. Baker and F. Cole, Eds. London: Routledge, 2019, pp. 12\u{2013}34.");
    assert_eq!(get(3), "[4]\tE. Poe, \"Ravens at scale,\" in *Proceedings of the Gothic Conference*, Boston, 2018, pp. 1\u{2013}10.");
    assert_eq!(get(4), "[5]\tWorld Health Organization, \"Global health report,\" World Health Organization, Geneva, Technical report, 2020.");
    assert_eq!(get(5), "[6]\tA. Rice, \"How to cite a web page,\" Citation Weekly. [Online]. Available: https://example.com/cite (accessed Jan. 15, 2023).");
    assert_eq!(get(6), "[7]\tA. Turing, \"Computable numbers,\" Ph.D. dissertation, University of Cambridge, Cambridge, 2015.");
    assert_eq!(get(7), "[8]\tD. Data, *Open dataset*. Zenodo, 2020.");
}

#[test]
fn ieee_numbers_and_in_text() {
    let (a, b, c, d) = (book(), article(), chapter(), conference());
    // Cited in this order: article, chapter, book, conference.
    let plan = Plan::new(Style::Ieee, &[&b, &c, &a, &d]);
    assert_eq!(plan.number_of("Lee21"), Some(1));
    assert_eq!(plan.number_of("Smi20"), Some(3));
    assert_eq!(plan.in_text(&[Item::new(&a)]).plain(), "[3]");
    assert_eq!(plan.in_text(&[Item::new(&b), Item::new(&a)]).plain(), "[1], [3]");
    assert_eq!(plan.in_text(&[Item::new(&a), Item::new(&b), Item::new(&c)]).plain(), "[1]\u{2013}[3]");
    assert_eq!(plan.in_text(&[Item { page: Some("12"), ..Item::new(&a) }]).plain(), "[3, p. 12]");
    assert_eq!(plan.entries().iter().map(|s| s.tag.as_str()).collect::<Vec<_>>(), ["Lee21", "Ada19", "Smi20", "Poe18"]);
    let mut many: Vec<Name> = (1..=7).map(|i| Name::person(&format!("N{i}"), "X")).collect();
    let mut s = book();
    s.authors = many.clone();
    assert!(entry(Style::Ieee, &s).contains("X. N1 et al.,"), "{}", entry(Style::Ieee, &s));
    many.truncate(3);
    assert_eq!(ieee_authors(&many), "X. N1, X. N2, and X. N3");
}

// ----- Harvard -------------------------------------------------------

#[test]
fn harvard_references() {
    assert_eq!(entry(Style::Harvard, &book()), "Smith, J. Q. (2020) *The Art of Citing*. New York: Penguin.");
    assert_eq!(
        entry(Style::Harvard, &article()),
        "Lee, K. and Kim, M.-J. (2021) 'A study of things', *Journal of Stuff*, 12(3), pp. 45\u{2013}67. doi: 10.1000/xyz123."
    );
    assert_eq!(
        entry(Style::Harvard, &chapter()),
        "Adams, R. (2019) 'On chapters', in Baker, E. and Cole, F. (eds.) *Book of Essays*. London: Routledge, pp. 12\u{2013}34."
    );
    assert_eq!(entry(Style::Harvard, &conference()), "Poe, E. (2018) 'Ravens at scale', in *Proceedings of the Gothic Conference*. Boston: ACM, pp. 1\u{2013}10.");
    assert_eq!(entry(Style::Harvard, &report()), "World Health Organization (2020) *Global health report*. Technical report. Geneva: World Health Organization.");
    assert_eq!(entry(Style::Harvard, &website()), "Rice, A. (2022) *How to cite a web page*. Citation Weekly. Available at: https://example.com/cite (Accessed: 15 January 2023).");
    assert_eq!(entry(Style::Harvard, &thesis()), "Turing, A. (2015) *Computable numbers*. PhD thesis. University of Cambridge.");
    let mut b = book();
    b.edition = Some("3".into());
    assert_eq!(entry(Style::Harvard, &b), "Smith, J. Q. (2020) *The Art of Citing*. 3rd edn. New York: Penguin.");
}

#[test]
fn harvard_in_text() {
    let (b, a) = (book(), article());
    assert_eq!(cite(Style::Harvard, &[&b], None), "(Smith, 2020)");
    assert_eq!(cite(Style::Harvard, &[&b], Some("23")), "(Smith, 2020, p. 23)");
    assert_eq!(cite(Style::Harvard, &[&a], None), "(Lee and Kim, 2021)");
    let mut three = b.clone();
    three.authors = vec![Name::person("Smith", "J"), Name::person("Lee", "K"), Name::person("Kim", "M")];
    assert_eq!(cite(Style::Harvard, &[&three], None), "(Smith, Lee and Kim, 2020)");
    three.authors.push(Name::person("Doe", "D"));
    assert_eq!(cite(Style::Harvard, &[&three], None), "(Smith et al., 2020)");
    assert_eq!(cite(Style::Harvard, &[&b, &a], None), "(Smith, 2020; Lee and Kim, 2021)");
}

// ----- Sorting and general ---------------------------------------------

#[test]
fn bibliography_is_alphabetical_for_author_styles() {
    let (b, a, c) = (book(), article(), chapter());
    let plan = Plan::new(Style::Apa, &[&b, &a, &c]);
    assert_eq!(plan.entries().iter().map(|s| s.tag.as_str()).collect::<Vec<_>>(), ["Ada19", "Lee21", "Smi20"]);
    let mut anon = book();
    anon.tag = "Anon".into();
    anon.authors.clear();
    anon.title = "The Beginning".into();
    let plan = Plan::new(Style::Mla, &[&b, &anon, &c]);
    // "The Beginning" sorts under B, between Adams and Smith.
    assert_eq!(plan.entries().iter().map(|s| s.tag.as_str()).collect::<Vec<_>>(), ["Ada19", "Anon", "Smi20"].iter().map(|s| s.to_string()).collect::<Vec<_>>());
}

#[test]
fn sorting_folds_accents() {
    let mut m = book();
    m.tag = "Mul".into();
    m.authors = vec![Name::person("M\u{fc}ller", "Hans")];
    let mut n = book();
    n.tag = "Nic".into();
    n.authors = vec![Name::person("Nichols", "Pat")];
    let mut l = book();
    l.tag = "Mur".into();
    l.authors = vec![Name::person("Murray", "Sue")];
    let plan = Plan::new(Style::Apa, &[&n, &m, &l]);
    assert_eq!(plan.entries().iter().map(|s| s.tag.as_str()).collect::<Vec<_>>(), ["Mul", "Mur", "Nic"]);
}

#[test]
fn word_style_names() {
    for st in Style::ALL {
        let (sel, name) = st.word();
        assert_eq!(Style::from_word(sel, name), st);
        assert_eq!(Style::parse(st.id()), Some(st));
    }
    assert_eq!(Style::from_word("\\ChicagoFifteenthEditionOfficeOnline.xsl", "Chicago"), Style::Chicago);
    assert_eq!(Style::from_word("", ""), Style::Apa);
    assert_eq!(Style::parse("APA 7"), Some(Style::Apa));
    assert_eq!(Style::parse("vancouver"), None);
    assert_eq!(Style::Mla.heading(), "Works Cited");
}

#[test]
fn no_trailing_junk() {
    for st in Style::ALL {
        for s in [book(), article(), chapter(), conference(), report(), website(), thesis(), misc(), Source::default()] {
            let plan = Plan::new(st, &[&s]);
            let e = plan.reference(&s).plain();
            assert!(!e.contains("  ") && !e.contains("..") && !e.contains(" ,") && !e.contains("()") && !e.contains("undefined"), "{st:?} {e:?}");
            assert!(!e.ends_with(' '), "{e:?}");
        }
    }
}

#[test]
fn corporate_authors_and_titles_with_question_marks() {
    let mut b = book();
    b.authors = vec![Name::Literal("American Psychological Association".into())];
    b.title = "Who cites?".into();
    assert_eq!(entry(Style::Apa, &b), "American Psychological Association. (2020). *Who cites?* Penguin.");
    assert_eq!(cite(Style::Apa, &[&b], None), "(American Psychological Association, 2020)");
    assert_eq!(entry(Style::Mla, &b), "American Psychological Association. *Who cites?* Penguin, 2020.");
}

/// What each style makes of a source with little data: just a title; an author, year and title; and a
/// container title and URL besides. Pinned so a change in any style shows up here.
#[test]
fn sparse_sources_in_every_style() {
    let kinds = [
        ("Book", SourceType::Book),
        ("Article", SourceType::Article),
        ("Chapter", SourceType::Chapter),
        ("Conference", SourceType::Conference),
        ("Report", SourceType::Report),
        ("Website", SourceType::Website),
        ("Thesis", SourceType::Thesis),
        ("Misc", SourceType::Misc),
    ];
    let styles = [("Apa", Style::Apa), ("Mla", Style::Mla), ("Chicago", Style::Chicago), ("Ieee", Style::Ieee), ("Harvard", Style::Harvard)];
    let expected: &[(&str, &str, &str, &str)] = &[
        ("Apa", "Book", "title only", "*Only a title*. (n.d.)."),
        ("Apa", "Book", "basic", "Doe, J. (2020). *A title*."),
        ("Apa", "Book", "+container+url", "Doe, J. (2020). *A title*. https://x.org/a"),
        ("Apa", "Article", "title only", "Only a title. (n.d.)."),
        ("Apa", "Article", "basic", "Doe, J. (2020). A title."),
        ("Apa", "Article", "+container+url", "Doe, J. (2020). A title. *Container*. https://x.org/a"),
        ("Apa", "Chapter", "title only", "Only a title. (n.d.)."),
        ("Apa", "Chapter", "basic", "Doe, J. (2020). A title."),
        ("Apa", "Chapter", "+container+url", "Doe, J. (2020). A title. In *Container*. https://x.org/a"),
        ("Apa", "Conference", "title only", "Only a title. (n.d.)."),
        ("Apa", "Conference", "basic", "Doe, J. (2020). A title."),
        ("Apa", "Conference", "+container+url", "Doe, J. (2020). A title. In *Container*. https://x.org/a"),
        ("Apa", "Report", "title only", "*Only a title*. (n.d.)."),
        ("Apa", "Report", "basic", "Doe, J. (2020). *A title*."),
        ("Apa", "Report", "+container+url", "Doe, J. (2020). *A title*. https://x.org/a"),
        ("Apa", "Website", "title only", "*Only a title*. (n.d.)."),
        ("Apa", "Website", "basic", "Doe, J. (2020). *A title*."),
        ("Apa", "Website", "+container+url", "Doe, J. (2020). *A title*. Container. https://x.org/a"),
        ("Apa", "Thesis", "title only", "*Only a title* [Doctoral dissertation]. (n.d.)."),
        ("Apa", "Thesis", "basic", "Doe, J. (2020). *A title* [Doctoral dissertation]."),
        ("Apa", "Thesis", "+container+url", "Doe, J. (2020). *A title* [Doctoral dissertation]. https://x.org/a"),
        ("Apa", "Misc", "title only", "*Only a title*. (n.d.)."),
        ("Apa", "Misc", "basic", "Doe, J. (2020). *A title*."),
        ("Apa", "Misc", "+container+url", "Doe, J. (2020). *A title*. https://x.org/a"),
        ("Mla", "Book", "title only", "*Only a title*."),
        ("Mla", "Book", "basic", "Doe, Jane. *A title*. 2020."),
        ("Mla", "Book", "+container+url", "Doe, Jane. *A title*. 2020, x.org/a."),
        ("Mla", "Article", "title only", "\"Only a title.\""),
        ("Mla", "Article", "basic", "Doe, Jane. \"A title.\" 2020."),
        ("Mla", "Article", "+container+url", "Doe, Jane. \"A title.\" *Container*, 2020, x.org/a."),
        ("Mla", "Chapter", "title only", "\"Only a title.\""),
        ("Mla", "Chapter", "basic", "Doe, Jane. \"A title.\" 2020."),
        ("Mla", "Chapter", "+container+url", "Doe, Jane. \"A title.\" *Container*, 2020, x.org/a."),
        ("Mla", "Conference", "title only", "\"Only a title.\""),
        ("Mla", "Conference", "basic", "Doe, Jane. \"A title.\" 2020."),
        ("Mla", "Conference", "+container+url", "Doe, Jane. \"A title.\" *Container*, 2020, x.org/a."),
        ("Mla", "Report", "title only", "*Only a title*."),
        ("Mla", "Report", "basic", "Doe, Jane. *A title*. 2020."),
        ("Mla", "Report", "+container+url", "Doe, Jane. *A title*. 2020, x.org/a."),
        ("Mla", "Website", "title only", "*Only a title*."),
        ("Mla", "Website", "basic", "Doe, Jane. *A title*. 2020."),
        ("Mla", "Website", "+container+url", "Doe, Jane. \"A title.\" *Container*, 2020, x.org/a."),
        ("Mla", "Thesis", "title only", "\"Only a title.\" PhD dissertation."),
        ("Mla", "Thesis", "basic", "Doe, Jane. \"A title.\" 2020. PhD dissertation."),
        ("Mla", "Thesis", "+container+url", "Doe, Jane. \"A title.\" 2020. PhD dissertation. x.org/a."),
        ("Mla", "Misc", "title only", "*Only a title*."),
        ("Mla", "Misc", "basic", "Doe, Jane. *A title*. 2020."),
        ("Mla", "Misc", "+container+url", "Doe, Jane. *A title*. 2020, x.org/a."),
        ("Chicago", "Book", "title only", "*Only a title*. n.d."),
        ("Chicago", "Book", "basic", "Doe, Jane. 2020. *A title*."),
        ("Chicago", "Book", "+container+url", "Doe, Jane. 2020. *A title*. https://x.org/a."),
        ("Chicago", "Article", "title only", "\"Only a title.\" n.d."),
        ("Chicago", "Article", "basic", "Doe, Jane. 2020. \"A title.\""),
        ("Chicago", "Article", "+container+url", "Doe, Jane. 2020. \"A title.\" *Container*. https://x.org/a."),
        ("Chicago", "Chapter", "title only", "\"Only a title.\" n.d."),
        ("Chicago", "Chapter", "basic", "Doe, Jane. 2020. \"A title.\""),
        ("Chicago", "Chapter", "+container+url", "Doe, Jane. 2020. \"A title.\" In *Container*. https://x.org/a."),
        ("Chicago", "Conference", "title only", "\"Only a title.\" n.d."),
        ("Chicago", "Conference", "basic", "Doe, Jane. 2020. \"A title.\""),
        ("Chicago", "Conference", "+container+url", "Doe, Jane. 2020. \"A title.\" In *Container*. https://x.org/a."),
        ("Chicago", "Report", "title only", "*Only a title*. n.d."),
        ("Chicago", "Report", "basic", "Doe, Jane. 2020. *A title*."),
        ("Chicago", "Report", "+container+url", "Doe, Jane. 2020. *A title*. https://x.org/a."),
        ("Chicago", "Website", "title only", "*Only a title*. n.d."),
        ("Chicago", "Website", "basic", "Doe, Jane. 2020. *A title*."),
        ("Chicago", "Website", "+container+url", "Doe, Jane. 2020. \"A title.\" Container. https://x.org/a."),
        ("Chicago", "Thesis", "title only", "\"Only a title.\" n.d. PhD diss."),
        ("Chicago", "Thesis", "basic", "Doe, Jane. 2020. \"A title.\" PhD diss."),
        ("Chicago", "Thesis", "+container+url", "Doe, Jane. 2020. \"A title.\" PhD diss. https://x.org/a."),
        ("Chicago", "Misc", "title only", "*Only a title*. n.d."),
        ("Chicago", "Misc", "basic", "Doe, Jane. 2020. *A title*."),
        ("Chicago", "Misc", "+container+url", "Doe, Jane. 2020. *A title*. https://x.org/a."),
        ("Ieee", "Book", "title only", "[1]\t*Only a title*."),
        ("Ieee", "Book", "basic", "[1]\tJ. Doe, *A title*, 2020."),
        ("Ieee", "Book", "+container+url", "[1]\tJ. Doe, *A title*, 2020. [Online]. Available: https://x.org/a"),
        ("Ieee", "Article", "title only", "[1]\t\"Only a title.\""),
        ("Ieee", "Article", "basic", "[1]\tJ. Doe, \"A title,\" 2020."),
        ("Ieee", "Article", "+container+url", "[1]\tJ. Doe, \"A title,\" *Container*, 2020. [Online]. Available: https://x.org/a"),
        ("Ieee", "Chapter", "title only", "[1]\t\"Only a title.\""),
        ("Ieee", "Chapter", "basic", "[1]\tJ. Doe, \"A title,\" 2020."),
        ("Ieee", "Chapter", "+container+url", "[1]\tJ. Doe, \"A title,\" in *Container*, 2020. [Online]. Available: https://x.org/a"),
        ("Ieee", "Conference", "title only", "[1]\t\"Only a title.\""),
        ("Ieee", "Conference", "basic", "[1]\tJ. Doe, \"A title,\" 2020."),
        ("Ieee", "Conference", "+container+url", "[1]\tJ. Doe, \"A title,\" in *Container*, 2020. [Online]. Available: https://x.org/a"),
        ("Ieee", "Report", "title only", "[1]\t\"Only a title.\""),
        ("Ieee", "Report", "basic", "[1]\tJ. Doe, \"A title,\" 2020."),
        ("Ieee", "Report", "+container+url", "[1]\tJ. Doe, \"A title,\" 2020. [Online]. Available: https://x.org/a"),
        ("Ieee", "Website", "title only", "[1]\t\"Only a title.\""),
        ("Ieee", "Website", "basic", "[1]\tJ. Doe, \"A title.\""),
        ("Ieee", "Website", "+container+url", "[1]\tJ. Doe, \"A title,\" Container. [Online]. Available: https://x.org/a"),
        ("Ieee", "Thesis", "title only", "[1]\t\"Only a title,\" Ph.D. dissertation."),
        ("Ieee", "Thesis", "basic", "[1]\tJ. Doe, \"A title,\" Ph.D. dissertation, 2020."),
        ("Ieee", "Thesis", "+container+url", "[1]\tJ. Doe, \"A title,\" Ph.D. dissertation, 2020. [Online]. Available: https://x.org/a"),
        ("Ieee", "Misc", "title only", "[1]\t*Only a title*."),
        ("Ieee", "Misc", "basic", "[1]\tJ. Doe, *A title*, 2020."),
        ("Ieee", "Misc", "+container+url", "[1]\tJ. Doe, *A title*, 2020. [Online]. Available: https://x.org/a"),
        ("Harvard", "Book", "title only", "*Only a title* (n.d.)."),
        ("Harvard", "Book", "basic", "Doe, J. (2020) *A title*."),
        ("Harvard", "Book", "+container+url", "Doe, J. (2020) *A title*. Available at: https://x.org/a."),
        ("Harvard", "Article", "title only", "'Only a title' (n.d.)."),
        ("Harvard", "Article", "basic", "Doe, J. (2020) 'A title'."),
        ("Harvard", "Article", "+container+url", "Doe, J. (2020) 'A title', *Container*. Available at: https://x.org/a."),
        ("Harvard", "Chapter", "title only", "'Only a title' (n.d.)."),
        ("Harvard", "Chapter", "basic", "Doe, J. (2020) 'A title'."),
        ("Harvard", "Chapter", "+container+url", "Doe, J. (2020) 'A title', in *Container*. Available at: https://x.org/a."),
        ("Harvard", "Conference", "title only", "'Only a title' (n.d.)."),
        ("Harvard", "Conference", "basic", "Doe, J. (2020) 'A title'."),
        ("Harvard", "Conference", "+container+url", "Doe, J. (2020) 'A title', in *Container*. Available at: https://x.org/a."),
        ("Harvard", "Report", "title only", "*Only a title* (n.d.)."),
        ("Harvard", "Report", "basic", "Doe, J. (2020) *A title*."),
        ("Harvard", "Report", "+container+url", "Doe, J. (2020) *A title*. Available at: https://x.org/a."),
        ("Harvard", "Website", "title only", "*Only a title* (n.d.)."),
        ("Harvard", "Website", "basic", "Doe, J. (2020) *A title*."),
        ("Harvard", "Website", "+container+url", "Doe, J. (2020) *A title*. Container. Available at: https://x.org/a."),
        ("Harvard", "Thesis", "title only", "*Only a title* (n.d.). PhD thesis."),
        ("Harvard", "Thesis", "basic", "Doe, J. (2020) *A title*. PhD thesis."),
        ("Harvard", "Thesis", "+container+url", "Doe, J. (2020) *A title*. PhD thesis. Available at: https://x.org/a."),
        ("Harvard", "Misc", "title only", "*Only a title* (n.d.)."),
        ("Harvard", "Misc", "basic", "Doe, J. (2020) *A title*."),
        ("Harvard", "Misc", "+container+url", "Doe, J. (2020) *A title*. Available at: https://x.org/a."),
    ];
    for (style_name, style) in styles {
        for (kind_name, kind) in kinds {
            let only = Source { tag: "X".into(), kind, title: "Only a title".into(), ..Default::default() };
            let basic = Source { tag: "X".into(), kind, title: "A title".into(), authors: vec![Name::person("Doe", "Jane")], year: Some("2020".into()), ..Default::default() };
            let full = Source { container_title: Some("Container".into()), url: Some("https://x.org/a".into()), ..basic.clone() };
            for (variant, s) in [("title only", &only), ("basic", &basic), ("+container+url", &full)] {
                let got = Plan::new(style, &[s]).reference(s).marked();
                let want = expected.iter().find(|(st, k, v, _)| *st == style_name && *k == kind_name && *v == variant).map(|e| e.3).unwrap();
                assert_eq!(got, want, "{style_name} {kind_name} {variant}");
            }
        }
    }
}

#[test]
fn more_reference_details() {
    // APA: an undated web page says when it was read; undated twins get n.d.-a and n.d.-b.
    let mut w = website();
    w.year = None;
    w.month = None;
    w.day = None;
    assert_eq!(entry(Style::Apa, &w), "Rice, A. (n.d.). *How to cite a web page*. Citation Weekly. Retrieved January 15, 2023, from https://example.com/cite");
    let (mut a, mut b) = (book(), book());
    a.year = None;
    b.year = None;
    a.tag = "A".into();
    b.tag = "B".into();
    b.title = "Another".into();
    let plan = Plan::new(Style::Apa, &[&a, &b]);
    assert_eq!(plan.in_text(&[Item::new(&a)]).plain(), "(Smith, n.d.-b)");
    assert_eq!(plan.in_text(&[Item::new(&b)]).plain(), "(Smith, n.d.-a)");
    // APA: more than twenty authors; a chapter with an edition and no editors.
    let mut c = chapter();
    c.editors.clear();
    c.edition = Some("3".into());
    assert_eq!(entry(Style::Apa, &c), "Adams, R. (2019). On chapters. In *Book of Essays* (3rd ed., pp. 12\u{2013}34). Routledge.");
    // MLA: editors only; a source with a URL but no DOI loses its protocol.
    let mut e = book();
    e.authors.clear();
    e.editors = vec![Name::person("Baker", "Ed"), Name::person("Cole", "Fran")];
    assert_eq!(entry(Style::Mla, &e), "Baker, Ed, and Fran Cole, editors. *The Art of Citing*. Penguin, 2020.");
    // Chicago: ten authors are listed, eleven are cut to seven and et al.
    let mut many = book();
    many.authors = (1..=10).map(|i| Name::person(&format!("N{i}"), "Xa")).collect();
    let ten = entry(Style::Chicago, &many);
    assert!(ten.starts_with("N1, Xa, Xa N2, Xa N3,") && ten.contains(", and Xa N10. 2020."), "{ten}");
    many.authors.push(Name::person("N11", "Xa"));
    assert!(entry(Style::Chicago, &many).starts_with("N1, Xa, Xa N2, Xa N3, Xa N4, Xa N5, Xa N6, Xa N7, et al. 2020."), "{}", entry(Style::Chicago, &many));
    // IEEE: seven authors become the first and et al.; editors-only gets "Ed.".
    let mut seven = book();
    seven.authors = (1..=7).map(|i| Name::person(&format!("N{i}"), "Xa")).collect();
    assert!(entry(Style::Ieee, &seven).contains("X. N1 et al., *The Art of Citing*"), "{}", entry(Style::Ieee, &seven));
    // Harvard: four authors are cut to the first and et al.
    let mut four = book();
    four.authors = (1..=4).map(|i| Name::person(&format!("N{i}"), "Xa")).collect();
    assert!(entry(Style::Harvard, &four).starts_with("N1, X. et al. (2020) *The Art"), "{}", entry(Style::Harvard, &four));
    // A page that is a single article number has no "p.".
    let mut art = article();
    art.pages = Some("e1234".into());
    assert!(entry(Style::Apa, &art).contains("(3), e1234. https"), "{}", entry(Style::Apa, &art));
    assert_eq!(cite(Style::Apa, &[&art], Some("e1234")), "(Lee & Kim, 2021, e1234)");
}
