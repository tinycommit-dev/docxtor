//! CSL-JSON in: what Zotero and Mendeley export, and what doi.org returns for
//! `Accept: application/vnd.citationstyles.csl+json`.

use crate::citations::{month_number, Name, Source, SourceType, MONTH_NAMES};
use serde_json::Value;

/// Text with the markup Crossref leaves in titles (`<i>`, `&amp;`) turned into plain text.
fn clean(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&apos;", "'");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A string, a number, or the first item of an array of them.
fn text(v: Option<&Value>) -> Option<String> {
    let t = match v? {
        Value::String(s) => clean(s),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => return text(a.first()),
        _ => return None,
    };
    Some(t).filter(|t| !t.is_empty())
}

fn names(v: Option<&Value>) -> Vec<Name> {
    let Some(Value::Array(list)) = v else { return Vec::new() };
    list.iter()
        .filter_map(|n| {
            let o = n.as_object()?;
            let get = |k: &str| o.get(k).and_then(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
            if let Some(lit) = get("literal").or_else(|| get("name")) {
                return Some(Name::Literal(lit));
            }
            let family = get("family");
            let given = get("given");
            match (family, given) {
                (None, None) => None,
                (None, Some(g)) => Some(Name::Literal(g)),
                (Some(f), g) => {
                    // Particles: "van" + "Beethoven", "Ludwig" + "van" belong to the surname or the given names.
                    let mut family = f;
                    if let Some(p) = get("non-dropping-particle") {
                        family = format!("{p} {family}");
                    }
                    if let Some(sfx) = get("suffix") {
                        family = format!("{family}, {sfx}");
                    }
                    let mut given = g.unwrap_or_default();
                    if let Some(p) = get("dropping-particle") {
                        given = format!("{given} {p}").trim().to_string();
                    }
                    Some(Name::person(&family, &given))
                }
            }
        })
        .collect()
}

/// `{"date-parts": [[2020, 3, 5]]}`, `{"raw": "2020-03-05"}` or `{"literal": "Spring 2020"}` as (year, month, day).
fn date(v: Option<&Value>) -> (Option<String>, Option<String>, Option<String>) {
    let Some(v) = v else { return (None, None, None) };
    let month = |m: &str| month_number(m).map(|n| MONTH_NAMES[n as usize - 1].to_string());
    if let Some(parts) = v.get("date-parts").and_then(|p| p.as_array()).and_then(|a| a.first()).and_then(|a| a.as_array()) {
        let n = |i: usize| text(parts.get(i));
        return (n(0), n(1).and_then(|m| month(&m)), n(2));
    }
    let raw = v.get("raw").or_else(|| v.get("literal")).and_then(|r| r.as_str()).or_else(|| v.as_str());
    if let Some(raw) = raw {
        let nums: Vec<&str> = raw.split(['-', '/']).collect();
        if !nums.is_empty() && nums[0].trim().len() == 4 && nums[0].trim().chars().all(|c| c.is_ascii_digit()) {
            return (Some(nums[0].trim().to_string()), nums.get(1).and_then(|m| month(m.trim())), nums.get(2).map(|d| d.trim().trim_start_matches('0').to_string()).filter(|d| !d.is_empty()));
        }
        let year = raw.split(|c: char| !c.is_ascii_digit()).find(|w| w.len() == 4).map(String::from);
        return (year, None, None);
    }
    (None, None, None)
}

fn iso_date(v: Option<&Value>) -> Option<String> {
    let (y, m, d) = date(v);
    let mut out = y?;
    if let Some(m) = m.and_then(|m| month_number(&m)) {
        out.push_str(&format!("-{m:02}"));
        if let Some(d) = d.and_then(|d| d.parse::<u32>().ok()) {
            out.push_str(&format!("-{d:02}"));
        }
    }
    Some(out)
}

fn item(o: &Value) -> Option<Source> {
    o.as_object()?;
    let t = |k: &str| text(o.get(k));
    let kind = match t("type").unwrap_or_default().as_str() {
        "article-journal" | "article" | "article-magazine" | "article-newspaper" | "review" | "review-book" => SourceType::Article,
        "book" | "edited-book" | "monograph" | "reference-book" | "proceedings" => SourceType::Book,
        "chapter" | "entry-encyclopedia" | "entry-dictionary" | "entry" | "book-chapter" | "book-section" | "reference-entry" => SourceType::Chapter,
        "paper-conference" | "proceedings-article" => SourceType::Conference,
        "report" | "standard" | "report-component" => SourceType::Report,
        "webpage" | "post-weblog" | "post" | "website" => SourceType::Website,
        "thesis" | "dissertation" => SourceType::Thesis,
        _ => SourceType::Misc,
    };
    let (year, month, day) = date(o.get("issued"));
    let mut s = Source {
        kind,
        authors: names(o.get("author")),
        editors: names(o.get("editor")),
        title: t("title").unwrap_or_default(),
        container_title: t("container-title").or_else(|| t("event-title")).or_else(|| t("event")).or_else(|| t("collection-title")),
        year,
        month,
        day,
        publisher: t("publisher"),
        place: t("publisher-place").or_else(|| t("event-place")),
        volume: t("volume"),
        issue: t("issue").or_else(|| t("number")),
        pages: t("page").map(|p| p.replace(['–', '—'], "-")).or_else(|| t("number-of-pages").filter(|_| false)),
        edition: t("edition"),
        url: t("URL"),
        doi: t("DOI"),
        accessed: iso_date(o.get("accessed")),
        genre: t("genre"),
        ..Default::default()
    };
    // Crossref's URL is just the DOI link again.
    if s.doi.is_some() && s.url.as_deref().is_some_and(|u| u.contains("doi.org/")) {
        s.url = None;
    }
    if kind == SourceType::Thesis && s.genre.is_none() {
        s.genre = Some("Thesis".into());
    }
    // A book with a container title is a part of a series, not a chapter.
    if matches!(kind, SourceType::Book | SourceType::Report | SourceType::Thesis) {
        s.container_title = None;
    }
    if s.title.is_empty() && s.authors.is_empty() && s.year.is_none() {
        return None;
    }
    Some(s)
}

/// Read CSL-JSON: an array of items, one item (doi.org), or an object with `items`.
pub fn parse(text: &str) -> Result<Vec<Source>, String> {
    crate::budget::within(text.len(), crate::budget::caps::LIST_BYTES).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("That is not valid JSON: {e}"))?;
    let list: Vec<&Value> = match &v {
        Value::Array(a) => a.iter().collect(),
        Value::Object(o) => match o.get("items").and_then(|i| i.as_array()) {
            Some(a) => a.iter().collect(),
            None => vec![&v],
        },
        _ => return Err("CSL-JSON is an object or a list of objects.".into()),
    };
    let sources: Vec<Source> = list.into_iter().filter_map(item).collect();
    if sources.is_empty() {
        return Err("No references found in that JSON. CSL-JSON items have a type, title, author and issued date.".into());
    }
    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doi_org_response() {
        let src = r#"{"indexed":{"date-parts":[[2023,9,1]],"timestamp":1693526400000},"reference-count":61,"publisher":"Springer Science and Business Media LLC","issue":"7553","license":[],"content-domain":{"domain":[]},"short-container-title":["Nature"],"published-print":{"date-parts":[[2015,5,28]]},"DOI":"10.1038/nature14539","type":"article-journal","created":{"date-parts":[[2015,5,27]]},"page":"436-444","source":"Crossref","title":"Deep learning","prefix":"10.1038","volume":"521","author":[{"given":"Yann","family":"LeCun","sequence":"first","affiliation":[]},{"given":"Yoshua","family":"Bengio","sequence":"additional","affiliation":[]},{"given":"Geoffrey","family":"Hinton","sequence":"additional","affiliation":[]}],"member":"297","container-title":"Nature","original-title":[],"language":"en","link":[],"deposited":{"date-parts":[[2022,1,1]]},"score":1,"subtitle":[],"short-title":[],"issued":{"date-parts":[[2015,5,28]]},"references-count":61,"journal-issue":{"issue":"7553","published-print":{"date-parts":[[2015,5,28]]}},"alternative-id":["BFnature14539"],"URL":"http://dx.doi.org/10.1038/nature14539","relation":{},"ISSN":["0028-0836","1476-4687"],"issn-type":[{"value":"0028-0836","type":"print"}],"subject":[]}"#;
        let v = parse(src).unwrap();
        assert_eq!(v.len(), 1);
        let s = &v[0];
        assert_eq!(s.kind, SourceType::Article);
        assert_eq!(s.title, "Deep learning");
        assert_eq!(s.authors.len(), 3);
        assert_eq!(s.authors[0], Name::person("LeCun", "Yann"));
        assert_eq!(s.container_title.as_deref(), Some("Nature"));
        assert_eq!((s.year.as_deref(), s.month.as_deref(), s.day.as_deref()), (Some("2015"), Some("May"), Some("28")));
        assert_eq!((s.volume.as_deref(), s.issue.as_deref(), s.pages.as_deref()), (Some("521"), Some("7553"), Some("436-444")));
        assert_eq!(s.doi.as_deref(), Some("10.1038/nature14539"));
        assert_eq!(s.url, None, "the doi.org link is the DOI again");
        assert_eq!(s.publisher.as_deref(), Some("Springer Science and Business Media LLC"));
    }

    #[test]
    fn zotero_export() {
        let src = r#"[
          {"id":"smith2020","type":"book","title":"The <i>Art</i> of Citing &amp; More","author":[{"family":"Smith","given":"John"},{"literal":"Citing Group"}],
           "publisher":"Penguin","publisher-place":"New York","issued":{"date-parts":[["2020"]]},"edition":2,"ISBN":"123"},
          {"id":"b","type":"chapter","title":"On chapters","container-title":"Book of Essays","editor":[{"family":"Baker","given":"Ed"}],"page":"12–34","issued":{"raw":"2019-04"}},
          {"id":"c","type":"webpage","title":"A page","URL":"https://example.com","accessed":{"date-parts":[[2023,1,15]]},"issued":{"date-parts":[[2022,5]]},"container-title":"Citation Weekly"},
          {"id":"d","type":"paper-conference","title":"Ravens","event-title":"Gothic Conference","issued":{"literal":"2018"}},
          {"id":"e","type":"thesis","title":"Computable numbers","author":[{"family":"Turing","given":"Alan"}],"publisher":"Cambridge","genre":"PhD thesis","issued":{"date-parts":[[1938]]}},
          {"id":"f","type":"software","title":"Tool","author":[{"family":"Beethoven","given":"Ludwig","non-dropping-particle":"van"}]}
        ]"#;
        let v = parse(src).unwrap();
        assert_eq!(v.len(), 6);
        assert_eq!(v[0].title, "The Art of Citing & More");
        assert_eq!(v[0].authors, vec![Name::person("Smith", "John"), Name::Literal("Citing Group".into())]);
        assert_eq!(v[0].edition.as_deref(), Some("2"));
        assert_eq!(v[0].year.as_deref(), Some("2020"));
        assert_eq!(v[1].kind, SourceType::Chapter);
        assert_eq!(v[1].pages.as_deref(), Some("12-34"));
        assert_eq!((v[1].year.as_deref(), v[1].month.as_deref()), (Some("2019"), Some("April")));
        assert_eq!(v[2].kind, SourceType::Website);
        assert_eq!(v[2].accessed.as_deref(), Some("2023-01-15"));
        assert_eq!(v[2].month.as_deref(), Some("May"));
        assert_eq!(v[3].container_title.as_deref(), Some("Gothic Conference"));
        assert_eq!(v[3].year.as_deref(), Some("2018"));
        assert_eq!(v[4].kind, SourceType::Thesis);
        assert_eq!(v[4].genre.as_deref(), Some("PhD thesis"));
        assert_eq!(v[5].kind, SourceType::Misc);
        assert_eq!(v[5].authors, vec![Name::person("van Beethoven", "Ludwig")]);
    }

    #[test]
    fn errors_are_plain() {
        assert!(parse("not json").unwrap_err().contains("not valid JSON"));
        assert!(parse("[]").is_err());
        assert!(parse("42").is_err());
        assert_eq!(parse(r#"{"items":[{"type":"book","title":"X"}]}"#).unwrap().len(), 1);
        assert_eq!(parse("\u{feff}{\"type\":\"book\",\"title\":\"X\"}").unwrap().len(), 1);
    }
}
