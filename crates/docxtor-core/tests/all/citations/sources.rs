//! Tests of: The sources part of a new document and updating and deleting sources.

use super::*;

#[test]
fn the_first_source_creates_the_part_with_its_properties_and_relationships() {
    let mut doc = doc_with("Hello.");
    let before = doc.save().unwrap();
    assert!(!has_part(&before, "customXml/item1.xml"));
    let tag = add(&mut doc, smith());
    assert_eq!(tag, "Smi20");
    assert!(doc.is_modified());
    let out = doc.save().unwrap();
    assert_valid(&out);
    let item = part(&out, "customXml/item1.xml");
    assert!(item.contains(&format!(r#"<b:Sources SelectedStyle="\APASixthEditionOfficeOnline.xsl" StyleName="APA" Version="6" xmlns:b="{BIB}" xmlns="{BIB}">"#)), "{item}");
    assert!(item.contains("<b:Tag>Smi20</b:Tag><b:SourceType>Book</b:SourceType><b:Guid>{"), "{item}");
    assert!(item.contains("<b:Author><b:Author><b:NameList><b:Person><b:Last>Smith</b:Last><b:First>John</b:First></b:Person></b:NameList></b:Author></b:Author>"), "{item}");
    assert!(item.contains("<b:Title>The Art of Citing</b:Title><b:Year>2020</b:Year>"), "{item}");
    assert!(item.contains("<b:City>New York</b:City><b:Publisher>Penguin</b:Publisher>"), "{item}");
    assert!(item.contains("<b:RefOrder>1</b:RefOrder></b:Source></b:Sources>"), "{item}");
    // The properties part.
    let props = part(&out, "customXml/itemProps1.xml");
    assert!(props.contains(r#"<ds:datastoreItem ds:itemID="{"#) && props.contains(r#"xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml""#), "{props}");
    assert!(props.contains(&format!(r#"<ds:schemaRef ds:uri="{BIB}"/>"#)), "{props}");
    // Relationships: item -> props, main document -> item.
    let item_rels = part(&out, "customXml/_rels/item1.xml.rels");
    assert!(item_rels.contains("relationships/customXmlProps") && item_rels.contains(r#"Target="itemProps1.xml""#), "{item_rels}");
    let main_rels = part(&out, "word/_rels/document.xml.rels");
    assert!(main_rels.contains(r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml""#) && main_rels.contains(r#"Target="../customXml/item1.xml""#), "{main_rels}");
    // Content type: the props part by override; the item is plain xml.
    let ct = part(&out, "[Content_Types].xml");
    assert!(ct.contains(r#"PartName="/customXml/itemProps1.xml" ContentType="application/vnd.openxmlformats-officedocument.customXmlProperties+xml""#), "{ct}");
    assert!(!ct.contains("/customXml/item1.xml"), "the item itself uses the xml default");
    // Other parts are as they were.
    let (a, b) = (Package::open(&before).unwrap(), Package::open(&out).unwrap());
    for p in &a.parts {
        if !matches!(p.name.as_str(), "word/_rels/document.xml.rels" | "[Content_Types].xml") {
            assert_eq!(b.get(&p.name), Some(p.data.as_slice()), "{} changed", p.name);
        }
    }
    // It reads back, and a second save doesn't add anything.
    let mut again = Document::open(&out).unwrap();
    assert_eq!(again.sources(), doc.sources());
    let twice = again.save().unwrap();
    assert_eq!(Package::open(&twice).unwrap().parts.len(), Package::open(&out).unwrap().parts.len());
}

#[test]
fn the_part_is_found_by_what_it_contains_not_its_name() {
    let (_, bytes) = word_made();
    let mut pkg = Package::open(&bytes).unwrap();
    // Rename the sources part and its relationships; add an unrelated custom XML part first.
    let data = pkg.get("customXml/item1.xml").unwrap().to_vec();
    pkg.parts.retain(|p| p.name != "customXml/item1.xml");
    pkg.set("customXml/item1.xml", br#"<root xmlns="urn:other"><x/></root>"#.to_vec());
    pkg.set("customXml/sources-main.xml", data);
    let rels = part(&pkg.to_bytes().unwrap(), "word/_rels/document.xml.rels").replace("../customXml/item1.xml", "../customXml/sources-main.xml");
    pkg.set("word/_rels/document.xml.rels", rels.into_bytes());
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    assert_eq!(doc.sources().len(), 2);
}

#[test]
fn a_document_with_other_custom_xml_gets_a_free_name() {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    pkg.set("customXml/item1.xml", br#"<root xmlns="urn:other"/>"#.to_vec());
    pkg.set("customXml/itemProps1.xml", br#"<ds:datastoreItem xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml" ds:itemID="{AAAA}"/>"#.to_vec());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    add(&mut doc, smith());
    let out = doc.save().unwrap();
    assert_valid(&out);
    assert!(has_part(&out, "customXml/item2.xml") && has_part(&out, "customXml/itemProps2.xml"));
    assert_eq!(part(&out, "customXml/item1.xml"), r#"<root xmlns="urn:other"/>"#);
    assert_eq!(Document::open(&out).unwrap().sources().len(), 1);
}

#[test]
fn tags_are_made_like_words_and_checked() {
    let mut doc = doc_with("Hello.");
    assert_eq!(add(&mut doc, smith()), "Smi20");
    assert_eq!(add(&mut doc, smith()), "Smi20a");
    assert_eq!(add(&mut doc, smith()), "Smi20b");
    assert_eq!(add(&mut doc, json!({"type":"misc","title":"The Web Thing"})), "The");
    let mut custom = smith();
    custom["tag"] = json!("MyTag");
    assert_eq!(add(&mut doc, custom.clone()), "MyTag");
    assert!(err(&mut doc, json!({"op":"add_source","source":custom})).contains("already exists"));
    let mut spaced = smith();
    spaced["tag"] = json!("has space");
    assert!(err(&mut doc, json!({"op":"add_source","source":spaced})).contains("can't be a source tag"));
    assert!(err(&mut doc, json!({"op":"add_source","source":{"type":"book"}})).contains("at least a title or an author"));
    // Ids are real GUIDs and differ.
    let guids: Vec<String> = doc.sources().iter().map(|s| s.guid.clone().unwrap()).collect();
    assert!(guids.iter().all(|g| g.len() == 38 && g.starts_with('{') && g.ends_with('}')));
    let mut uniq = guids.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), guids.len());
}

#[test]
fn every_source_type_round_trips_through_the_part() {
    let mut doc = doc_with("Hello.");
    let kinds = [
        json!({"type":"book","authors":["Smith, John Q."],"title":"B","year":"2001","publisher":"P","place":"C","edition":"2"}),
        json!({"type":"article","authors":["Lee, Karen"],"title":"A","containerTitle":"J","year":"2002","month":"May","day":"4","volume":"1","issue":"2","pages":"1-2","doi":"10.1/a"}),
        json!({"type":"chapter","authors":["Adams, Ruth"],"editors":["Baker, Ed","Cole, Fran"],"title":"Ch","containerTitle":"Book","year":"2003","pages":"3-4","publisher":"P"}),
        json!({"type":"conference","authors":["Poe, Edgar"],"title":"Paper","containerTitle":"Proceedings","year":"2004","pages":"5-6"}),
        json!({"type":"report","authors":[{"literal":"WHO"}],"title":"Rep","year":"2005","publisher":"WHO","genre":"Technical report"}),
        json!({"type":"website","authors":["Rice, Anne"],"title":"Page","containerTitle":"Site","year":"2006","url":"https://example.com","accessed":"2007-01-15"}),
        json!({"type":"website","title":"Bare page","url":"https://example.com/b"}),
        json!({"type":"thesis","authors":["Turing, Alan"],"title":"Th","year":"1938","publisher":"Cambridge","genre":"Doctoral dissertation"}),
        json!({"type":"misc","authors":["Data, Dana"],"title":"Dataset","containerTitle":"Zenodo","year":"2008"}),
    ];
    for k in &kinds {
        add(&mut doc, k.clone());
    }
    let written = doc.sources();
    let out = doc.save().unwrap();
    assert_valid(&out);
    let back = Document::open(&out).unwrap().sources();
    assert_eq!(back.len(), kinds.len());
    for (a, b) in written.iter().zip(&back) {
        assert_eq!(a.kind, b.kind, "{}", a.title);
        assert_eq!(a.authors, b.authors, "{}", a.title);
        assert_eq!(a.editors, b.editors, "{}", a.title);
        assert_eq!(a.title, b.title);
        assert_eq!(a.container_title, b.container_title, "{}", a.title);
        assert_eq!((&a.year, &a.pages, &a.doi, &a.url, &a.publisher, &a.genre, &a.accessed), (&b.year, &b.pages, &b.doi, &b.url, &b.publisher, &b.genre, &b.accessed), "{}", a.title);
    }
    let item = part(&out, "customXml/item1.xml");
    for word in ["Book", "JournalArticle", "BookSection", "ConferenceProceedings", "Report", "DocumentFromInternetSite", "InternetSite", "Misc"] {
        assert!(item.contains(&format!("<b:SourceType>{word}</b:SourceType>")), "{word}: {item}");
    }
    assert!(item.contains("<b:Author><b:Editor><b:NameList>") || item.contains("<b:Editor><b:NameList>"), "editors are written");
    assert!(item.contains("<b:Corporate>WHO</b:Corporate>"), "{item}");
    assert!(item.contains("<b:Month>May</b:Month>"), "months are written as names");
    assert!(item.contains("<b:YearAccessed>2007</b:YearAccessed><b:MonthAccessed>January</b:MonthAccessed><b:DayAccessed>15</b:DayAccessed>"), "{item}");
    assert!(item.contains("<b:ThesisType>Doctoral dissertation</b:ThesisType>") && item.contains("<b:Institution>Cambridge</b:Institution>"), "{item}");
}

#[test]
fn updating_a_source_rewrites_what_uses_it() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    ops(&mut doc, json!({"op":"insert_bibliography"}));
    assert_eq!(texts(&doc)[0], "Citing is hard(Smith, 2020) here.");
    let mut s = doc.sources()[0].clone();
    s.authors = vec![docxtor_core::citations::Name::person("Smythe", "John")];
    s.year = Some("2021".into());
    ops(&mut doc, json!({"op":"update_source","tag":"Smi20","source":s}));
    assert_eq!(texts(&doc)[0], "Citing is hard(Smythe, 2021) here.");
    assert!(bib_texts(&doc).contains(&"Smythe, J. (2021). The Art of Citing. Penguin.".to_string()), "{:?}", bib_texts(&doc));
    assert!(doc.sources()[0].guid.is_some(), "the id stays");
    // An unknown tag is an error that names it.
    assert!(err(&mut doc, json!({"op":"update_source","tag":"Nope","source":smith()})).contains("no source with the tag 'Nope'"));
    // Undo puts it all back.
    assert!(doc.undo());
    assert_eq!(texts(&doc)[0], "Citing is hard(Smith, 2020) here.");
    assert_eq!(doc.sources()[0].authors[0].family(), "Smith");
}

#[test]
fn renaming_a_tag_follows_into_the_citations() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20", "Lee21"], None);
    let mut s = doc.sources()[0].clone();
    s.tag = "Smith2020".into();
    ops(&mut doc, json!({"op":"update_source","tag":"Smi20","source":s}));
    assert_eq!(doc.citations()[0].tags, ["Smith2020", "Lee21"]);
    assert_eq!(texts(&doc)[0], "Citing is hard(Smith, 2020; Lee & Kim, 2021) here.");
    let out = doc.save().unwrap();
    assert!(part(&out, "word/document.xml").contains("CITATION Smith2020 \\m Lee21 \\l 1033"));
    assert!(err(&mut doc, json!({"op":"update_source","tag":"Lee21","source":{"tag":"Smith2020","title":"x","type":"misc"}})).contains("already exists"));
}

#[test]
fn deleting_a_cited_source_is_refused_in_plain_words() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    cite(&mut doc, "Second paragraph", "things", &["Smi20"], None);
    let e = err(&mut doc, json!({"op":"delete_source","tag":"Smi20"}));
    assert_eq!(e, "The source 'Smi20' is cited 2 times in the document. Remove those citations first, or delete the source anyway to turn them into plain text.");
    assert_eq!(doc.sources().len(), 2, "nothing changed");
    // An uncited one goes.
    ops(&mut doc, json!({"op":"delete_source","tag":"Lee21"}));
    assert_eq!(doc.sources().len(), 1);
    assert!(err(&mut doc, json!({"op":"delete_source","tag":"Lee21"})).contains("no source with the tag"));
    // Forced: the citations become the text they showed.
    ops(&mut doc, json!({"op":"delete_source","tag":"Smi20","force":true}));
    assert_eq!(doc.sources().len(), 0);
    assert_eq!(doc.citations().len(), 0);
    assert_eq!(texts(&doc), ["Citing is hard(Smith, 2020) here.", "Second paragraph about things(Smith, 2020)."]);
    // And it all undoes.
    assert!(doc.undo());
    assert_eq!(doc.citations().len(), 2);
    assert_eq!(doc.sources().len(), 1);
    assert!(doc.undo());
    assert_eq!(doc.sources().len(), 2);
}

#[test]
fn forcing_the_delete_of_one_of_two_sources_keeps_the_other_in_the_citation() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20", "Lee21"], None);
    ops(&mut doc, json!({"op":"delete_source","tag":"Smi20","force":true}));
    assert_eq!(doc.citations()[0].tags, ["Lee21"]);
    assert_eq!(texts(&doc)[0], "Citing is hard(Lee & Kim, 2021) here.");
}
