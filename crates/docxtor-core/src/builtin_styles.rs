//! Word's built-in styles that a file may not define yet (its latent styles), written as Word's default template
//! writes them: the headings, Title and Subtitle, Quote and Intense Quote, the emphasis styles and No Spacing. The
//! colours name theme colours, so they follow the document's theme.

/// The styles the style gallery offers even when the document does not define them: `(style id, display name)`,
/// in the order Word's gallery shows them.
pub const GALLERY: &[(&str, &str)] = &[
    ("Normal", "Normal"),
    ("NoSpacing", "No Spacing"),
    ("Heading1", "Heading 1"),
    ("Heading2", "Heading 2"),
    ("Heading3", "Heading 3"),
    ("Heading4", "Heading 4"),
    ("Heading5", "Heading 5"),
    ("Heading6", "Heading 6"),
    ("Heading7", "Heading 7"),
    ("Heading8", "Heading 8"),
    ("Heading9", "Heading 9"),
    ("Title", "Title"),
    ("Subtitle", "Subtitle"),
    ("SubtleEmphasis", "Subtle Emphasis"),
    ("Emphasis", "Emphasis"),
    ("IntenseEmphasis", "Intense Emphasis"),
    ("Strong", "Strong"),
    ("Quote", "Quote"),
    ("IntenseQuote", "Intense Quote"),
    ("SubtleReference", "Subtle Reference"),
    ("IntenseReference", "Intense Reference"),
    ("BookTitle", "Book Title"),
    ("ListParagraph", "List Paragraph"),
];

const ACCENT: &str = r#"<w:color w:val="0F4761" w:themeColor="accent1" w:themeShade="BF"/>"#;
const GREY: &str = r#"<w:color w:val="595959" w:themeColor="text1" w:themeTint="A6"/>"#;
const DARK: &str = r#"<w:color w:val="272727" w:themeColor="text1" w:themeTint="D8"/>"#;
const HEADING_FONT: &str = r#"<w:rFonts w:asciiTheme="majorHAnsi" w:eastAsiaTheme="majorEastAsia" w:hAnsiTheme="majorHAnsi" w:cstheme="majorBidi"/>"#;

/// `(id, name, size in half points, color, italic, before, after, extra attributes of the style)`
fn heading(n: u8) -> String {
    let (size, color, italic, before, after) = match n {
        1 => (40, ACCENT, false, 360, 80),
        2 => (32, ACCENT, false, 160, 80),
        3 => (28, ACCENT, false, 160, 80),
        4 => (24, ACCENT, true, 80, 40),
        5 => (24, ACCENT, false, 80, 40),
        6 => (24, GREY, true, 40, 0),
        7 => (24, GREY, false, 40, 0),
        8 => (24, DARK, true, 0, 0),
        _ => (24, DARK, false, 0, 0),
    };
    let hidden = if n >= 7 { "<w:semiHidden/><w:unhideWhenUsed/>" } else { "" };
    let sz = if n >= 6 { String::new() } else { format!(r#"<w:sz w:val="{size}"/><w:szCs w:val="{size}"/>"#) };
    let i = if italic { "<w:i/><w:iCs/>" } else { "" };
    format!(
        r#"<w:style w:type="paragraph" w:styleId="Heading{n}"><w:name w:val="heading {n}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:link w:val="Heading{n}Char"/><w:uiPriority w:val="9"/>{hidden}<w:qFormat/><w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before="{before}" w:after="{after}"/><w:outlineLvl w:val="{lvl}"/></w:pPr><w:rPr>{HEADING_FONT}{i}{color}{sz}</w:rPr></w:style>"#,
        lvl = n - 1
    )
}

/// The definition of a built-in style by its normalised key (lower case, no spaces), if Docxtor has one.
pub fn xml(key: &str) -> Option<String> {
    let s = |x: &str| Some(x.to_string());
    match key {
        "normal" => s(r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>"#),
        k if k.len() == 8 && k.starts_with("heading") && matches!(k.as_bytes()[7], b'1'..=b'9') => Some(heading(k.as_bytes()[7] - b'0')),
        "title" => s(&format!(
            r#"<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:link w:val="TitleChar"/><w:uiPriority w:val="10"/><w:qFormat/><w:pPr><w:spacing w:after="80" w:line="240" w:lineRule="auto"/><w:contextualSpacing/></w:pPr><w:rPr>{HEADING_FONT}<w:spacing w:val="-10"/><w:kern w:val="28"/><w:sz w:val="56"/><w:szCs w:val="56"/></w:rPr></w:style>"#
        )),
        "subtitle" => s(r#"<w:style w:type="paragraph" w:styleId="Subtitle"><w:name w:val="Subtitle"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:link w:val="SubtitleChar"/><w:uiPriority w:val="11"/><w:qFormat/><w:pPr><w:numPr><w:ilvl w:val="1"/></w:numPr><w:spacing w:after="160"/></w:pPr><w:rPr><w:rFonts w:eastAsiaTheme="majorEastAsia" w:cstheme="majorBidi"/><w:color w:val="595959" w:themeColor="text1" w:themeTint="A6"/><w:spacing w:val="15"/><w:sz w:val="28"/><w:szCs w:val="28"/></w:rPr></w:style>"#),
        "quote" => s(r#"<w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:link w:val="QuoteChar"/><w:uiPriority w:val="29"/><w:qFormat/><w:pPr><w:spacing w:before="160" w:after="160"/><w:ind w:left="864" w:right="864"/><w:jc w:val="center"/></w:pPr><w:rPr><w:i/><w:iCs/><w:color w:val="404040" w:themeColor="text1" w:themeTint="BF"/></w:rPr></w:style>"#),
        "intensequote" => s(&format!(
            r#"<w:style w:type="paragraph" w:styleId="IntenseQuote"><w:name w:val="Intense Quote"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:link w:val="IntenseQuoteChar"/><w:uiPriority w:val="30"/><w:qFormat/><w:pPr><w:pBdr><w:top w:val="single" w:sz="4" w:space="10" w:color="0F4761" w:themeColor="accent1" w:themeShade="BF"/><w:bottom w:val="single" w:sz="4" w:space="10" w:color="0F4761" w:themeColor="accent1" w:themeShade="BF"/></w:pBdr><w:spacing w:before="360" w:after="360"/><w:ind w:left="864" w:right="864"/><w:jc w:val="center"/></w:pPr><w:rPr><w:i/><w:iCs/>{ACCENT}</w:rPr></w:style>"#
        )),
        "emphasis" => s(r#"<w:style w:type="character" w:styleId="Emphasis"><w:name w:val="Emphasis"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="20"/><w:qFormat/><w:rPr><w:i/><w:iCs/></w:rPr></w:style>"#),
        "strong" => s(r#"<w:style w:type="character" w:styleId="Strong"><w:name w:val="Strong"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="22"/><w:qFormat/><w:rPr><w:b/><w:bCs/></w:rPr></w:style>"#),
        "subtleemphasis" => s(r#"<w:style w:type="character" w:styleId="SubtleEmphasis"><w:name w:val="Subtle Emphasis"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="19"/><w:qFormat/><w:rPr><w:i/><w:iCs/><w:color w:val="404040" w:themeColor="text1" w:themeTint="BF"/></w:rPr></w:style>"#),
        "intenseemphasis" => s(&format!(
            r#"<w:style w:type="character" w:styleId="IntenseEmphasis"><w:name w:val="Intense Emphasis"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="21"/><w:qFormat/><w:rPr><w:i/><w:iCs/>{ACCENT}</w:rPr></w:style>"#
        )),
        "subtlereference" => s(r#"<w:style w:type="character" w:styleId="SubtleReference"><w:name w:val="Subtle Reference"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="31"/><w:qFormat/><w:rPr><w:smallCaps/><w:color w:val="5A5A5A" w:themeColor="text1" w:themeTint="A5"/></w:rPr></w:style>"#),
        "intensereference" => s(&format!(
            r#"<w:style w:type="character" w:styleId="IntenseReference"><w:name w:val="Intense Reference"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="32"/><w:qFormat/><w:rPr><w:b/><w:bCs/><w:smallCaps/>{ACCENT}<w:spacing w:val="5"/></w:rPr></w:style>"#
        )),
        "booktitle" => s(r#"<w:style w:type="character" w:styleId="BookTitle"><w:name w:val="Book Title"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="33"/><w:qFormat/><w:rPr><w:b/><w:bCs/><w:i/><w:iCs/><w:spacing w:val="5"/></w:rPr></w:style>"#),
        "nospacing" => s(r#"<w:style w:type="paragraph" w:styleId="NoSpacing"><w:name w:val="No Spacing"/><w:uiPriority w:val="1"/><w:qFormat/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:style>"#),
        "bodytext" => s(r#"<w:style w:type="paragraph" w:styleId="BodyText"><w:name w:val="Body Text"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:spacing w:after="120"/></w:pPr></w:style>"#),
        _ => None,
    }
}

/// The character style that goes with a linked paragraph style, as Word writes it (`Heading 1 Char`).
pub fn linked_char_style(id: &str, name: &str, rpr_xml: &str) -> String {
    format!(
        r#"<w:style w:type="character" w:customStyle="1" w:styleId="{id}Char"><w:name w:val="{name} Char"/><w:basedOn w:val="DefaultParagraphFont"/><w:link w:val="{id}"/><w:uiPriority w:val="1"/><w:rPr>{rpr_xml}</w:rPr></w:style>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml;

    #[test]
    fn every_gallery_style_is_well_formed() {
        for (id, name) in GALLERY {
            let src = crate::styles::builtin_style_xml(name).unwrap_or_else(|| panic!("no definition for {name}"));
            let doc = xml::parse(format!(r#"<w:styles xmlns:w="w">{src}</w:styles>"#).as_bytes()).unwrap();
            let style = doc.root.elements().next().unwrap();
            assert_eq!(style.attr("w:styleId"), Some(*id), "{name}");
        }
    }
}
