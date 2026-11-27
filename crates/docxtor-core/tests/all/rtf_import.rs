//! The RTF reader: Word, WordPad and LibreOffice style documents into fragments.

use docxtor_core::fragment::{b64_decode, walk, Fragment};
use docxtor_core::rtf::{looks_like_rtf, rtf_bytes_to_fragment, rtf_to_fragment};
use docxtor_core::xml::{self, Element};

const RPR_ORDER: &[&str] = &[
    "w:rStyle", "w:rFonts", "w:b", "w:bCs", "w:i", "w:iCs", "w:caps", "w:smallCaps", "w:strike", "w:dstrike", "w:outline", "w:shadow", "w:emboss", "w:imprint",
    "w:noProof", "w:snapToGrid", "w:vanish", "w:webHidden", "w:color", "w:spacing", "w:w", "w:kern", "w:position", "w:sz", "w:szCs", "w:highlight", "w:u",
    "w:effect", "w:bdr", "w:shd", "w:fitText", "w:vertAlign", "w:rtl", "w:cs", "w:em", "w:lang", "w:eastAsianLayout", "w:specVanish", "w:oMath",
];

const PPR_ORDER: &[&str] = &[
    "w:pStyle", "w:keepNext", "w:keepLines", "w:pageBreakBefore", "w:framePr", "w:widowControl", "w:numPr", "w:suppressLineNumbers", "w:pBdr", "w:shd", "w:tabs",
    "w:suppressAutoHyphens", "w:kinsoku", "w:wordWrap", "w:overflowPunct", "w:topLinePunct", "w:autoSpaceDE", "w:autoSpaceDN", "w:bidi", "w:adjustRightInd",
    "w:snapToGrid", "w:spacing", "w:ind", "w:contextualSpacing", "w:mirrorIndents", "w:suppressOverlap", "w:jc", "w:textDirection", "w:textAlignment",
    "w:textboxTightWrap", "w:outlineLvl", "w:divId", "w:cnfStyle", "w:rPr", "w:sectPr", "w:pPrChange",
];

const TCPR_ORDER: &[&str] = &[
    "w:cnfStyle", "w:tcW", "w:gridSpan", "w:hMerge", "w:vMerge", "w:tcBorders", "w:shd", "w:noWrap", "w:tcMar", "w:textDirection", "w:tcFitText", "w:vAlign", "w:hideMark",
];

const TBLPR_ORDER: &[&str] = &[
    "w:tblStyle", "w:tblpPr", "w:tblOverlap", "w:bidiVisual", "w:tblStyleRowBandSize", "w:tblStyleColBandSize", "w:tblW", "w:jc", "w:tblCellSpacing", "w:tblInd",
    "w:tblBorders", "w:shd", "w:tblLayout", "w:tblCellMar", "w:tblLook",
];

fn assert_order(el: &Element, order: &[&str]) {
    let mut last = 0;
    for c in el.elements() {
        let rank = order.iter().position(|o| *o == c.name).unwrap_or_else(|| panic!("{} is not allowed in {}", c.name, el.name));
        assert!(rank >= last, "{} is out of order in {}: {}", c.name, el.name, el.to_xml());
        last = rank;
    }
}

/// Every part parses as XML, and property elements keep the schema order Word needs.
fn check(f: &Fragment) {
    for part in [&f.body, &f.styles, &f.numbering] {
        if !part.is_empty() {
            xml::parse(format!("<f>{part}</f>").as_bytes()).unwrap_or_else(|e| panic!("not xml: {e}\n{part}"));
        }
    }
    for n in f.notes.values() {
        xml::parse(n.xml.as_bytes()).unwrap_or_else(|e| panic!("note not xml: {e}\n{}", n.xml));
    }
    let parsed = f.parse();
    for el in parsed.body.iter().chain(parsed.styles.iter()) {
        walk(el, &mut |e| match e.name.as_str() {
            "w:rPr" => assert_order(e, RPR_ORDER),
            "w:pPr" => assert_order(e, PPR_ORDER),
            "w:tcPr" => assert_order(e, TCPR_ORDER),
            "w:tblPr" => assert_order(e, TBLPR_ORDER),
            _ => {}
        });
    }
    assert!(f.block);
}

fn read(rtf: &str) -> Fragment {
    let f = rtf_to_fragment(rtf);
    check(&f);
    f
}

fn paras(f: &Fragment) -> usize {
    f.parse().body.iter().filter(|e| e.name == "w:p").count()
}

fn png_hex() -> String {
    let png = b64_decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==").unwrap();
    png.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Documents as the three programs write them
// ---------------------------------------------------------------------------

const WORD_DOC: &str = r#"{\rtf1\ansi\ansicpg1252\deff0\nouicompat\deflang1033{\fonttbl{\f0\fswiss\fprq2\fcharset0 Calibri;}{\f1\fswiss\fprq2\fcharset0 Arial;}{\f2\fnil\fcharset2 Symbol;}{\f3\fmodern\fcharset0 Courier New;}}
{\colortbl ;\red255\green0\blue0;\red0\green0\blue255;\red255\green255\blue0;}
{\*\generator Riched20 10.0.19041}
{\stylesheet{\ql \li0\ri0\sa160\sl259\slmult1\widctlpar\wrapdefault\aspalpha\aspnum\faauto\adjustright\rin0\lin0\itap0 \rtlch\fcs1 \af0\afs22\alang1025 \ltrch\fcs0 \f0\fs22\lang1033\langfe1033\cgrid\langnp1033\langfenp1033 \snext0 \sqformat \spriority0 Normal;}
{\s1\ql \li0\ri0\sb240\sa0\keepn\widctlpar\outlinelevel0 \b\f1\fs32\cf2 \sbasedon0 \snext0 \sqformat heading 1;}
{\s2\ql \li720\ri0\sb120\sa120\widctlpar \i\fs20\cf1 \sbasedon0 \snext2 \sqformat My Note;}
{\*\cs10 \additive \ssemihidden \sunhideWhenUsed \spriority1 Default Paragraph Font;}
{\*\cs11 \additive \b\cf1 \sbasedon10 Strong Red;}}
{\info{\author Somebody}{\title Not shown}}
\paperw12240\paperh15840\margl1440\margr1440\margt1440\margb1440\gutter0\deftab720\viewkind1\uc1
\pard\plain\s1\ql\li0\ri0\sb240\sa0\keepn\widctlpar\outlinelevel0\b\f1\fs32\cf2 Heading One\par
\pard\plain\ql\li0\ri0\sa160\sl259\slmult1\widctlpar\f0\fs22\lang1033 Normal text with {\b bold}, {\i italic}, {\ul underlined} and {\cf1\fs28 big red} and {\f1 Arial text}.\par
\pard\plain\s2\ql\li720\ri0\sb120\sa120\widctlpar\i\fs20\cf1 A note paragraph.\par
\pard\plain\qc\fi360\li720\sb200\sa100\sl360\slmult1 Centered {\highlight3 highlighted} text\par
\pard\plain\ql\li0\ri0\sa160\sl259\slmult1\widctlpar\f0\fs22 With a {\cs11\b\cf1 character style} here.\par
}"#;

#[test]
fn word_document_text_and_structure() {
    let f = read(WORD_DOC);
    assert_eq!(
        f.plain_text(),
        "Heading One\nNormal text with bold, italic, underlined and big red and Arial text.\nA note paragraph.\nCentered highlighted text\nWith a character style here."
    );
    assert_eq!(paras(&f), 5);
    // The document's own default font is reported, and Normal is not redefined.
    assert_eq!(f.defaults.as_ref().and_then(|d| d.font.as_deref()), Some("Calibri"));
    assert_eq!(f.defaults.as_ref().and_then(|d| d.size), Some(11.0));
    assert!(!f.styles.contains(r#"w:styleId="Normal""#));
}

#[test]
fn word_document_character_formatting() {
    let f = read(WORD_DOC);
    assert!(f.body.contains("<w:b/>"), "{}", f.body);
    assert!(f.body.contains("<w:i/>"));
    assert!(f.body.contains(r#"<w:u w:val="single"/>"#));
    assert!(f.body.contains(r#"<w:color w:val="FF0000"/>"#));
    assert!(f.body.contains(r#"<w:sz w:val="28"/>"#));
    assert!(f.body.contains(r#"<w:szCs w:val="28"/>"#));
    assert!(f.body.contains(r#"w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:cs="Arial""#));
    assert!(f.body.contains(r#"<w:highlight w:val="yellow"/>"#));
    // Direct formatting that repeats Normal (Calibri 11 pt) is not written again.
    assert!(!f.body.contains(r#"w:val="22""#), "{}", f.body);
    assert!(!f.body.contains("Calibri"));
}

#[test]
fn word_document_paragraph_formatting() {
    let f = read(WORD_DOC);
    assert!(f.body.contains(r#"<w:jc w:val="center"/>"#));
    assert!(f.body.contains(r#"w:left="720""#));
    assert!(f.body.contains(r#"w:firstLine="360""#));
    assert!(f.body.contains(r#"<w:spacing w:before="200" w:after="100" w:line="360" w:lineRule="auto"/>"#), "{}", f.body);
    // Normal's own spacing (sa160, sl259) is the destination's business, so it is not repeated.
    assert!(!f.body.contains(r#"w:line="259""#));
}

#[test]
fn word_document_styles() {
    let f = read(WORD_DOC);
    // Heading 1 maps to Word's own definition and the paragraph uses it.
    assert!(f.styles.contains(r#"w:styleId="Heading1""#), "{}", f.styles);
    assert!(f.body.contains(r#"<w:pStyle w:val="Heading1"/>"#));
    // The heading's direct formatting equals the style's, so the heading run carries none.
    let first = &f.body[..f.body.find("</w:p>").unwrap()];
    assert!(!first.contains("<w:b/>") && !first.contains("w:sz"), "{first}");
    // A custom style is defined with its name, base, next style and formatting.
    assert!(f.styles.contains(r#"w:styleId="MyNote""#));
    assert!(f.styles.contains(r#"<w:name w:val="My Note"/>"#));
    assert!(f.styles.contains(r#"<w:basedOn w:val="Normal"/>"#));
    assert!(f.styles.contains(r#"<w:next w:val="MyNote"/>"#));
    assert!(f.styles.contains(r#"<w:ind w:left="720"/>"#));
    assert!(f.styles.contains("<w:i/>"));
    assert!(f.body.contains(r#"<w:pStyle w:val="MyNote"/>"#));
    // A character style used with \cs.
    assert!(f.styles.contains(r#"w:type="character""#));
    assert!(f.styles.contains(r#"w:styleId="StrongRed""#));
    assert!(f.body.contains(r#"<w:rStyle w:val="StrongRed"/>"#));
    // Default Paragraph Font is not a style to bring along.
    assert!(!f.styles.contains("DefaultParagraphFont\"/><w:name"));
    assert!(!f.styles.contains("Default Paragraph Font"));
}

#[test]
fn heading_style_without_a_stylesheet_definition_still_maps_by_name() {
    let f = read(r#"{\rtf1\ansi{\fonttbl{\f0 Arial;}}{\stylesheet{\s1 heading 1;}{\s2\sbasedon1 \snext1 Title;}{\s3 Quote;}}\pard\s1 H\par\pard\s2 T\par\pard\s3 Q\par}"#);
    assert!(f.styles.contains(r#"w:styleId="Heading1""#));
    assert!(f.styles.contains(r#"w:styleId="Title""#));
    assert!(f.styles.contains(r#"w:styleId="Quote""#));
    assert!(f.body.contains(r#"<w:pStyle w:val="Title"/>"#));
}

#[test]
fn wordpad_document() {
    let f = read(
        r#"{\rtf1\ansi\ansicpg1252\deff0\nouicompat\deflang1033{\fonttbl{\f0\fnil\fcharset0 Calibri;}}
{\*\generator Riched20 10.0.19041}\viewkind4\uc1
\pard\sa200\sl276\slmult1\f0\fs22\lang9 Hello \b world\b0  and more\par
Second line\par
}"#,
    );
    assert_eq!(f.plain_text(), "Hello world and more\nSecond line");
    assert_eq!(paras(&f), 2);
    assert!(f.body.contains(r#"<w:spacing w:after="200" w:line="276" w:lineRule="auto"/>"#), "{}", f.body);
    assert!(f.body.contains("<w:b/>"));
    // The font is the document default, so it is not written as direct formatting.
    assert!(!f.body.contains("Calibri"));
    assert!(f.body.contains(r#"w:sz w:val="22""#));
}

#[test]
fn libreoffice_document() {
    let f = read(
        r#"{\rtf1\ansi\deff4\adeflang1025
{\fonttbl{\f0\froman\fprq2\fcharset0 Times New Roman;}{\f1\froman\fprq2\fcharset2 Symbol;}{\f2\fswiss\fprq2\fcharset0 Arial;}{\f3\fnil\fprq2\fcharset0 Liberation Sans;}{\f4\fswiss\fprq2\fcharset0 Liberation Sans;}}
{\colortbl;\red0\green0\blue0;\red128\green128\blue128;}
{\stylesheet{\s0\snext0\ql\nowidctlpar\hyphpar0\ltrpar\cf0\kerning1\dbch\af7\langfe2052\dbch\af8\afs24\alang1081\loch\f4\hich\af4\fs24\lang1033 Normal;}
{\s15\sbasedon0\snext16\ql\nowidctlpar\hyphpar0\sb240\sa120\keepn\ltrpar\cf0\b\dbch\af7\afs28\loch\f4\fs28\lang1033 Heading;}
{\s16\sbasedon0\snext16\sl276\slmult1\ql\nowidctlpar\hyphpar0\sb0\sa140\ltrpar\cf0\dbch\af7\afs24\loch\f4\fs24\lang1033 Text Body;}}
{\info{\creatim\yr2024\mo1\dy2}}
\deftab709
\pard\plain \s15\sbasedon0\snext16\ql\nowidctlpar\hyphpar0\sb240\sa120\keepn\ltrpar\cf0\b\dbch\af7\afs28\loch\f4\fs28\lang1033 {\b\loch\f4\fs28\lang1033\i0\strike0\ltrch Title text}
\par \pard\plain \s16\sbasedon0\snext16\sl276\slmult1\ql\nowidctlpar\hyphpar0\sb0\sa140\ltrpar\cf0\dbch\af7\afs24\loch\f4\fs24\lang1033 {\rtlch \ltrch\loch\f4\fs24\lang1033\i0\b0 Body text with }{\rtlch \ltrch\loch\f4\fs24\lang1033\i0\b0\i italic}{\rtlch \ltrch\loch\f4\fs24\lang1033\i0\b0 .}
\par }"#,
    );
    assert_eq!(f.plain_text(), "Title text\nBody text with italic.");
    assert!(f.body.contains("<w:i/>"));
    // Text Body is a custom style, Heading is too (it is not Word's "heading 1").
    assert!(f.styles.contains(r#"w:styleId="TextBody""#), "{}", f.styles);
    assert!(f.styles.contains(r#"w:styleId="Heading""#));
    // The runs repeat the style's formatting, and the font is the default one, so they carry almost nothing.
    assert!(!f.body.contains("Liberation"));
    // The language is the style's, so the runs do not repeat it.
    assert!(!f.body.contains("w:lang"));
}

// ---------------------------------------------------------------------------
// Text, characters and code pages
// ---------------------------------------------------------------------------

#[test]
fn escapes_unicode_and_code_pages() {
    let f = read(
        r#"{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fswiss Arial;}{\f1\fswiss\fcharset204 Arial;}{\f2\fswiss\fcharset161 Arial;}{\f3\fswiss\fcharset162 Arial;}{\f4\fswiss\fcharset238 Arial;}}
\pard Price \u8364? 5, caf\'e9, \'93quoted\'94 \u233\'e9 ok\par
\pard {\uc2\u8364\'80\'80x} \u-10179\u-8704? {\f1 \'cf\'f0\'e8\'e2\'e5\'f2} {\f2 \'c1\'f1\'e9} {\f3 \'fd\'f0\'d0} {\f4 \'e8\'b3}\par
\pard a\~b\-c\_d \emdash \endash \lquote x\rquote \ldblquote y\rdblquote \bullet \'80 \'96\par
\pard back\\slash \{brace\} \u233? ok\par
}"#,
    );
    let lines: Vec<&str> = vec![
        "Price \u{20AC} 5, caf\u{e9}, \u{201C}quoted\u{201D} \u{e9} ok",
        "\u{20AC}x \u{1F600} \u{41F}\u{440}\u{438}\u{432}\u{435}\u{442} \u{391}\u{3C1}\u{3B9} \u{131}\u{11F}\u{11E} \u{10D}\u{142}",
        // A space after a control word ends the word; the characters are not spaced.
        "a\u{a0}b\u{ad}c\u{2011}d \u{2014}\u{2013}\u{2018}x\u{2019}\u{201C}y\u{201D}\u{2022}\u{20AC} \u{2013}",
        "back\\slash {brace} \u{e9} ok",
    ];
    assert_eq!(f.plain_text(), lines.join("\n"));
}

#[test]
fn latin1_and_utf8_bytes() {
    let mut bytes = br"{\rtf1\ansi caf".to_vec();
    bytes.push(0xE9);
    bytes.extend_from_slice(b"\\par}");
    assert_eq!(rtf_bytes_to_fragment(&bytes).plain_text(), "caf\u{e9}");
    // Valid UTF-8 passes through as it is.
    assert_eq!(rtf_bytes_to_fragment("{\\rtf1 caf\u{e9} \u{4e2d}}".as_bytes()).plain_text(), "caf\u{e9} \u{4e2d}");
    assert_eq!(rtf_to_fragment("{\\rtf1 \u{201c}hi\u{201d}}").plain_text(), "\u{201c}hi\u{201d}");
}

#[test]
fn looks_like_rtf_checks_the_start() {
    assert!(looks_like_rtf(r"{\rtf1\ansi hi}"));
    assert!(looks_like_rtf("\u{feff}  \n{\\rtf1 hi}"));
    assert!(!looks_like_rtf("<html>{\\rtf1}</html>"));
    assert!(!looks_like_rtf("plain text"));
    assert!(!looks_like_rtf(""));
}

#[test]
fn character_effects() {
    let f = read(
        r#"{\rtf1\ansi\deff0{\fonttbl{\f0 Arial;}}{\colortbl;\red255\green0\blue0;\red10\green200\blue30;}
\pard {\strike s}{\striked1 d}{\v hidden}{\scaps sc}{\caps c}{\super up}{\sub dn}{\up10 raised}{\expnd8 wide}{\charscalex150 scaled}{\cb2 bg}{\lang3082 es}{\lang1024 none}{\ulw words}{\uldb dbl}{\ulth thick}{\ulc1\ul colored}\par}"#,
    );
    for want in [
        "<w:strike/>",
        "<w:dstrike/>",
        "<w:vanish/>",
        "<w:smallCaps/>",
        "<w:caps/>",
        r#"<w:vertAlign w:val="superscript"/>"#,
        r#"<w:vertAlign w:val="subscript"/>"#,
        r#"<w:position w:val="10"/>"#,
        r#"<w:spacing w:val="40"/>"#,
        r#"<w:w w:val="150"/>"#,
        r#"<w:shd w:val="clear" w:color="auto" w:fill="0AC81E"/>"#,
        r#"<w:lang w:val="es-ES"/>"#,
        r#"<w:u w:val="words"/>"#,
        r#"<w:u w:val="double"/>"#,
        r#"<w:u w:val="thick"/>"#,
        r#"<w:u w:val="single" w:color="FF0000"/>"#,
    ] {
        assert!(f.body.contains(want), "missing {want}: {}", f.body);
    }
    assert!(!f.body.contains("1024"));
    // Adjacent runs with the same formatting are one run.
    let g = read(r#"{\rtf1\ansi {\b a}{\b b}{\b c} {\i d}\par}"#);
    assert_eq!(g.body.matches("<w:r>").count(), 3, "{}", g.body);
}

#[test]
fn tabs_breaks_and_pages() {
    let f = read(r#"{\rtf1\ansi A\tab B\line C\page D\column E\par}"#);
    assert_eq!(f.plain_text(), "A\tB\nCDE");
    assert!(f.body.contains(r#"<w:br w:type="page"/>"#));
    assert!(f.body.contains("<w:tab/>"));
}

#[test]
fn highlight_without_a_word_colour_falls_back_to_shading() {
    let f = read(r#"{\rtf1\ansi{\colortbl;\red255\green255\blue0;\red200\green100\blue50;\red0\green0\blue128;}\pard {\highlight1 a}{\highlight2 b}{\highlight3 c}\par}"#);
    assert!(f.body.contains(r#"<w:highlight w:val="yellow"/>"#));
    assert!(f.body.contains(r#"<w:shd w:val="clear" w:color="auto" w:fill="C86432"/>"#));
    assert!(f.body.contains(r#"<w:highlight w:val="darkBlue"/>"#));
}

// ---------------------------------------------------------------------------
// Paragraph formatting
// ---------------------------------------------------------------------------

#[test]
fn paragraph_properties_are_in_schema_order() {
    let f = read(
        r#"{\rtf1\ansi{\colortbl;\red255\green0\blue0;\red0\green255\blue0;\red255\green255\blue0;}
\pard\keepn\keep\pagebb\nowidctlpar\box\brdrs\brdrw10\brdrcf1\tx720\sb100\sa200\li100\ri50\fi-50\qj\outlinelevel1\cbpat3\rtlpar\hyphpar0 text\par}"#,
    );
    let p = &f.parse().body[0];
    let names: Vec<String> = p.child("w:pPr").unwrap().elements().map(|e| e.name.to_string()).collect();
    assert_eq!(
        names,
        ["w:keepNext", "w:keepLines", "w:pageBreakBefore", "w:widowControl", "w:pBdr", "w:shd", "w:tabs", "w:suppressAutoHyphens", "w:bidi", "w:spacing", "w:ind", "w:jc", "w:outlineLvl"]
    );
    assert!(f.body.contains(r#"<w:widowControl w:val="0"/>"#));
    assert!(f.body.contains(r#"<w:jc w:val="both"/>"#));
    assert!(f.body.contains(r#"w:left="100" w:right="50" w:hanging="50""#));
}

#[test]
fn tabs_borders_and_shading() {
    let f = read(
        r#"{\rtf1\ansi{\colortbl;\red255\green0\blue0;\red0\green255\blue0;\red255\green255\blue0;}
\pard\tx720\tqr\tldot\tx9000\tqdec\tlhyph\tx5000\brdrt\brdrs\brdrw15\brdrcf1\brdrsp20\brdrb\brdrdb\brdrw10\brdrl\brdrdash\brdrr\brdrnil\cbpat3 Para\par}"#,
    );
    assert!(f.body.contains(r#"<w:tab w:val="left" w:pos="720"/>"#), "{}", f.body);
    assert!(f.body.contains(r#"<w:tab w:val="right" w:leader="dot" w:pos="9000"/>"#));
    assert!(f.body.contains(r#"<w:tab w:val="decimal" w:leader="hyphen" w:pos="5000"/>"#));
    assert!(f.body.contains(r#"<w:top w:val="single" w:sz="6" w:space="1" w:color="FF0000"/>"#));
    assert!(f.body.contains(r#"<w:bottom w:val="double" w:sz="4" w:space="0" w:color="auto"/>"#));
    assert!(f.body.contains(r#"<w:left w:val="dashed""#));
    assert!(f.body.contains(r#"<w:right w:val="nil"/>"#));
    assert!(f.body.contains(r#"<w:shd w:val="clear" w:color="auto" w:fill="FFFF00"/>"#));
    // Tab stops come out in position order.
    let a = f.body.find("5000").unwrap();
    let b = f.body.find("9000").unwrap();
    assert!(a < b);
}

#[test]
fn line_spacing_rules() {
    let f = read(r#"{\rtf1\ansi\pard\sl-300\slmult0 a\par\pard\sl400\slmult0 b\par\pard\sl480\slmult1 c\par\pard\sbauto1\saauto1 d\par}"#);
    assert!(f.body.contains(r#"w:line="300" w:lineRule="exact""#));
    assert!(f.body.contains(r#"w:line="400" w:lineRule="atLeast""#));
    assert!(f.body.contains(r#"w:line="480" w:lineRule="auto""#));
    assert!(f.body.contains(r#"w:beforeAutospacing="1" w:afterAutospacing="1""#));
}

#[test]
fn trailing_paragraph_marks() {
    assert_eq!(paras(&read(r"{\rtf1\ansi a\par}")), 1);
    assert_eq!(paras(&read(r"{\rtf1\ansi a\par b}")), 2);
    assert_eq!(paras(&read(r"{\rtf1\ansi a\par b\par\par}")), 3);
    assert_eq!(paras(&read(r"{\rtf1\ansi \par a\par}")), 2);
    assert_eq!(paras(&read(r"{\rtf1\ansi }")), 0);
    assert_eq!(read(r"{\rtf1\ansi a\par\sect b\par}").plain_text(), "a\nb");
}

#[test]
fn groups_save_and_restore_formatting() {
    let f = read(r#"{\rtf1\ansi\pard\qc A {\b B {\i C} D} E\par\pard F\par}"#);
    assert_eq!(f.plain_text(), "A B C D E\nF");
    // Only the paragraph that said \qc is centred.
    assert_eq!(f.body.matches(r#"w:val="center""#).count(), 1);
    assert_eq!(f.body.matches("<w:i/>").count(), 1);
    assert_eq!(f.body.matches("<w:b/>").count(), 3);
}

#[test]
fn skipped_destinations_and_headers() {
    let f = read(
        r#"{\rtf1\ansi\deff0{\fonttbl{\f0 Arial;}}{\*\unknown stuff {\b nested}}{\info{\title T}{\author A}}{\header Header text\par}{\footerl Footer\par}
{\*\bkmkstart mark}Body{\*\bkmkend mark}{\xe{\v index}} text{\*\generator X}{\themedata ABCD}{\datastore 00}\par}"#,
    );
    assert_eq!(f.plain_text(), "Body text");
}

// ---------------------------------------------------------------------------
// Lists
// ---------------------------------------------------------------------------

const LIST_DOC: &str = r#"{\rtf1\ansi\deff0{\fonttbl{\f0\fswiss Arial;}{\f1\fnil\fcharset2 Symbol;}{\f2\fnil Wingdings;}{\f3\fmodern Courier New;}}
{\*\listtable
{\list\listtemplateid101\listhybrid
{\listlevel\levelnfc23\levelnfcn23\leveljc0\leveljcn0\levelfollow0\levelstartat1\levelspace0\levelindent0{\leveltext\leveltemplateid1\'01\uc1\u-3913 ?;}{\levelnumbers;}\f1\fi-360\li720\jclisttab\tx720}
{\listlevel\levelnfc23\levelnfcn23\leveljc0\leveljcn0\levelfollow0\levelstartat1\levelspace0\levelindent0{\leveltext\leveltemplateid2\'01o;}{\levelnumbers;}\f3\fi-360\li1440\jclisttab\tx1440}
{\listlevel\levelnfc23\levelnfcn23\leveljc0\leveljcn0\levelfollow0\levelstartat1\levelspace0\levelindent0{\leveltext\leveltemplateid3\'01\'a7;}{\levelnumbers;}\f2\fi-360\li2160\jclisttab\tx2160}
\listid1001}
{\list\listtemplateid102\listhybrid
{\listlevel\levelnfc0\levelnfcn0\leveljc0\leveljcn0\levelfollow0\levelstartat1\levelspace0\levelindent0{\leveltext\leveltemplateid4\'02\'00.;}{\levelnumbers\'01;}\fi-360\li720\jclisttab\tx720}
{\listlevel\levelnfc4\levelnfcn4\leveljc0\leveljcn0\levelfollow0\levelstartat1\levelspace0\levelindent0{\leveltext\leveltemplateid5\'02\'01.;}{\levelnumbers\'01;}\fi-360\li1440\jclisttab\tx1440}
{\listlevel\levelnfc2\levelnfcn2\leveljc2\leveljcn2\levelfollow0\levelstartat3\levelspace0\levelindent0{\leveltext\leveltemplateid6\'04(\'02);}{\levelnumbers\'01;}\fi-180\li2160}
\listid1002}}
{\*\listoverridetable{\listoverride\listid1001\listoverridecount0\ls1}{\listoverride\listid1002\listoverridecount0\ls2}}
\pard\plain\ls1\ilvl0\fi-360\li720{\listtext\f1\'b7\tab}Bullet one\par
\pard\plain\ls1\ilvl1\fi-360\li1440{\listtext\f3 o\tab}Sub bullet\par
\pard\plain\ls1\ilvl2\fi-360\li2160{\listtext\f2 \'a7\tab}Third level\par
\pard\plain\ls2\ilvl0\fi-360\li720{\listtext 1.\tab}First\par
\pard\plain\ls2\ilvl0\fi-360\li720{\listtext 2.\tab}Second\par
\pard\plain\ls2\ilvl1\fi-360\li1440{\listtext a.\tab}Nested\par
\pard\plain\ls2\ilvl2\fi-180\li2160{\listtext (iii)\tab}Deep\par
\pard\plain Plain\par
}"#;

#[test]
fn bullet_and_numbered_lists() {
    let f = read(LIST_DOC);
    assert_eq!(f.plain_text(), "Bullet one\nSub bullet\nThird level\nFirst\nSecond\nNested\nDeep\nPlain");
    // One list per \ls, in order of first use: a bullet list and a decimal list.
    assert_eq!(f.numbering.matches("<w:abstractNum ").count(), 2, "{}", f.numbering);
    assert_eq!(f.numbering.matches("<w:num ").count(), 2);
    assert!(f.numbering.contains(r#"<w:numFmt w:val="bullet"/>"#));
    assert!(f.numbering.contains(r#"<w:numFmt w:val="decimal"/>"#));
    assert!(f.numbering.contains(r#"<w:numFmt w:val="lowerLetter"/>"#));
    assert!(f.numbering.contains(r#"<w:numFmt w:val="lowerRoman"/>"#));
    // Symbol private-use bullets become ordinary bullets; Courier's "o" stays; Wingdings 0xA7 becomes a square.
    assert!(f.numbering.contains(r#"<w:lvlText w:val="&#8226;"/>"#) || f.numbering.contains("<w:lvlText w:val=\"\u{2022}\"/>"), "{}", f.numbering);
    assert!(f.numbering.contains(r#"<w:lvlText w:val="o"/>"#));
    assert!(f.numbering.contains("<w:lvlText w:val=\"\u{25AA}\"/>"));
    assert!(f.numbering.contains(r#"w:rFonts w:ascii="Courier New""#));
    assert!(!f.numbering.contains("Symbol"));
    // Number patterns.
    assert!(f.numbering.contains(r#"<w:lvlText w:val="%1."/>"#));
    assert!(f.numbering.contains(r#"<w:lvlText w:val="%2."/>"#));
    assert!(f.numbering.contains(r#"<w:lvlText w:val="(%3)"/>"#));
    assert!(f.numbering.contains(r#"<w:start w:val="3"/>"#));
    // Indents come from the levels.
    assert!(f.numbering.contains(r#"<w:ind w:left="720" w:hanging="360"/>"#));
    assert!(f.numbering.contains(r#"<w:ind w:left="1440" w:hanging="360"/>"#));
    assert!(f.numbering.contains(r#"<w:ind w:left="2160" w:hanging="180"/>"#));
}

#[test]
fn list_paragraphs_use_numpr_and_list_paragraph() {
    let f = read(LIST_DOC);
    assert!(f.body.contains(r#"<w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr>"#), "{}", f.body);
    assert!(f.body.contains(r#"<w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr>"#));
    assert!(f.body.contains(r#"<w:numPr><w:ilvl w:val="0"/><w:numId w:val="2"/></w:numPr>"#));
    assert!(f.body.contains(r#"<w:numPr><w:ilvl w:val="2"/><w:numId w:val="2"/></w:numPr>"#));
    assert!(f.styles.contains(r#"w:styleId="ListParagraph""#));
    assert!(f.body.contains(r#"<w:pStyle w:val="ListParagraph"/>"#));
    // The indents repeat the levels', so the paragraphs do not carry their own.
    assert!(!f.body.contains("<w:ind"), "{}", f.body);
    // The \listtext numbers are not text.
    assert!(!f.plain_text().contains("1."));
    // The plain paragraph is not in a list.
    let last = &f.body[f.body.rfind("<w:p>").unwrap()..];
    assert!(!last.contains("numPr"));
}

#[test]
fn list_override_start_values() {
    let f = read(
        r#"{\rtf1\ansi{\fonttbl{\f0 Arial;}}
{\*\listtable{\list\listtemplateid1{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}\fi-360\li720}\listid5}}
{\*\listoverridetable{\listoverride\listid5\listoverridecount1{\lfolevel\listoverridestartat\levelstartat7}\ls3}}
\pard\ls3\ilvl0 seven\par}"#,
    );
    assert!(f.numbering.contains(r#"<w:start w:val="7"/>"#), "{}", f.numbering);
    assert!(f.body.contains(r#"<w:numId w:val="1"/>"#));
}

#[test]
fn word_97_list_paragraphs() {
    let f = read(
        r#"{\rtf1\ansi{\fonttbl{\f0\fnil Arial;}{\f1\fnil\fcharset2 Symbol;}}
\pard{\pntext\f1\'B7\tab}{\*\pn\pnlvlblt\pnf1\pnindent0{\pntxtb\'B7}}\fi-720\li720 item one\par
\pard{\pntext\f1\'B7\tab}\fi-720\li720 item two\par
\pard normal\par
\pard{\pntext\f0 1.\tab}{\*\pn\pnlvlbody\pnf0\pnindent0\pnstart1\pndec{\pntxta.}}\fi-720\li720 first\par
\pard{\pntext\f0 2.\tab}\fi-720\li720 second\par}"#,
    );
    assert_eq!(f.plain_text(), "item one\nitem two\nnormal\nfirst\nsecond");
    assert_eq!(f.numbering.matches("<w:abstractNum ").count(), 2, "{}", f.numbering);
    assert!(f.numbering.contains(r#"<w:numFmt w:val="bullet"/>"#));
    assert!(f.numbering.contains(r#"<w:numFmt w:val="decimal"/>"#));
    assert_eq!(f.body.matches(r#"<w:numId w:val="1"/>"#).count(), 2);
    assert_eq!(f.body.matches(r#"<w:numId w:val="2"/>"#).count(), 2);
    assert_eq!(paras(&f), 5);
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

#[test]
fn table_with_merges_shading_and_borders() {
    let f = read(
        r#"{\rtf1\ansi{\fonttbl{\f0 Arial;}}{\colortbl;\red255\green0\blue0;}
Before\par
\trowd\trgaph108\trleft-108\trrh300
\clvmgf\clbrdrt\brdrs\brdrw10\clbrdrl\brdrs\brdrw10\clbrdrb\brdrs\brdrw10\clbrdrr\brdrs\brdrw10\clcbpat1\cellx3000
\clvertalc\cellx6000\cellx9000
\pard\intbl A1\cell B1\cell C1\cell\row
\trowd\trgaph108\trleft-108
\clvmrg\cellx3000\clmgf\cellx6000\clmrg\cellx9000
\pard\intbl\cell B2\cell \cell\row
\pard After\par
}"#,
    );
    assert_eq!(f.plain_text(), "Before\nA1\tB1\tC1\n\tB2\nAfter");
    let body = f.parse().body;
    assert_eq!(body.len(), 3);
    assert_eq!(body[1].name, "w:tbl");
    let tbl = &body[1];
    let grid: Vec<&str> = tbl.child("w:tblGrid").unwrap().children_named("w:gridCol").map(|c| c.attr("w:w").unwrap()).collect();
    assert_eq!(grid, ["3108", "3000", "3000"]);
    assert_eq!(tbl.child("w:tblPr").unwrap().child("w:tblW").unwrap().attr("w:w"), Some("9108"));
    let rows: Vec<&Element> = tbl.children_named("w:tr").collect();
    assert_eq!(rows.len(), 2);
    let cells0: Vec<&Element> = rows[0].children_named("w:tc").collect();
    let cells1: Vec<&Element> = rows[1].children_named("w:tc").collect();
    assert_eq!(cells0.len(), 3);
    // Row 2: the first cell continues the vertical merge, the second spans two columns.
    assert_eq!(cells1.len(), 2);
    assert_eq!(cells1[1].child("w:tcPr").unwrap().child("w:gridSpan").unwrap().attr("w:val"), Some("2"));
    assert_eq!(cells0[0].child("w:tcPr").unwrap().child("w:vMerge").unwrap().attr("w:val"), Some("restart"));
    let cont = cells1[0].child("w:tcPr").unwrap().child("w:vMerge").unwrap();
    assert_eq!(cont.attr("w:val"), None);
    // Shading, borders, vertical alignment and row height.
    assert!(tbl.to_xml().contains(r#"<w:shd w:val="clear" w:color="auto" w:fill="FF0000"/>"#));
    assert_eq!(cells0[0].child("w:tcPr").unwrap().child("w:tcBorders").unwrap().elements().count(), 4);
    assert!(cells0[1].child("w:tcPr").unwrap().child("w:vAlign").is_some());
    assert_eq!(rows[0].child("w:trPr").unwrap().child("w:trHeight").unwrap().attr("w:val"), Some("300"));
    // Every cell ends with a paragraph.
    for tc in cells0.iter().chain(cells1.iter()) {
        assert_eq!(tc.elements().last().unwrap().name, "w:p");
    }
    assert!(f.body.ends_with("After</w:t></w:r></w:p>"));
}

#[test]
fn table_rows_with_different_layouts_share_a_grid() {
    let f = read(
        r#"{\rtf1\ansi
\trowd\trgaph108\cellx2000\cellx4000\pard\intbl a\cell b\cell\row
\trowd\trgaph108\cellx1000\cellx4000\pard\intbl c\cell d\cell\row
}"#,
    );
    let tbl = &f.parse().body[0];
    let grid: Vec<&str> = tbl.child("w:tblGrid").unwrap().children_named("w:gridCol").map(|c| c.attr("w:w").unwrap()).collect();
    assert_eq!(grid, ["1000", "1000", "2000"]);
    let rows: Vec<&Element> = tbl.children_named("w:tr").collect();
    let spans = |r: &Element| -> Vec<Option<String>> {
        r.children_named("w:tc").map(|c| c.child("w:tcPr").unwrap().child("w:gridSpan").and_then(|g| g.attr("w:val")).map(String::from)).collect()
    };
    assert_eq!(spans(rows[0]), [Some("2".to_string()), None]);
    assert_eq!(spans(rows[1]), [None, Some("2".to_string())]);
}

#[test]
fn table_paragraphs_keep_their_formatting_and_multiple_paragraphs() {
    let f = read(
        r#"{\rtf1\ansi\trowd\trgaph108\cellx3000\cellx6000
\pard\intbl\qc{\b one}\par\pard\intbl two\cell\pard\intbl\qr three\cell\row
\pard after\par}"#,
    );
    let tbl = &f.parse().body[0];
    let cells: Vec<&Element> = tbl.child("w:tr").unwrap().children_named("w:tc").collect();
    assert_eq!(cells[0].children_named("w:p").count(), 2);
    assert!(cells[0].to_xml().contains(r#"<w:jc w:val="center"/>"#));
    assert!(cells[1].to_xml().contains(r#"<w:jc w:val="right"/>"#));
    assert_eq!(f.plain_text(), "one two\tthree\nafter");
}

#[test]
fn nested_tables_are_flattened() {
    let f = read(
        r#"{\rtf1\ansi\trowd\cellx6000\pard\intbl\itap1 outer\par
\pard\intbl\itap2 inner a\nestcell\pard\intbl\itap2 inner b\nestcell{\*\nesttableprops\trowd\cellx3000\nestrow}{\nonesttables\par}
\pard\intbl\itap1\cell\row\pard done\par}"#,
    );
    let body = f.parse().body;
    assert_eq!(body.len(), 2);
    let tc = body[0].child("w:tr").unwrap().child("w:tc").unwrap();
    assert_eq!(tc.children_named("w:p").count(), 3);
    assert_eq!(f.plain_text(), "outer inner a inner b\ndone");
}

#[test]
fn table_without_row_end_still_closes() {
    let f = read(r#"{\rtf1\ansi\trowd\cellx2000\cellx4000\pard\intbl a\cell b"#);
    assert_eq!(f.plain_text(), "a\tb");
    assert_eq!(f.parse().body[0].name, "w:tbl");
}

// ---------------------------------------------------------------------------
// Fields, pictures, notes
// ---------------------------------------------------------------------------

#[test]
fn hyperlinks_and_other_fields() {
    let f = read(
        r##"{\rtf1\ansi\deff0{\fonttbl{\f0 Arial;}}{\colortbl;\red0\green0\blue255;}
\pard See {\field{\*\fldlock}{\*\fldinst {HYPERLINK "http://example.com/a?b=1&c=2"}}{\fldrslt {\cs15\ul\cf1 the site}}} and {\field{\*\fldinst HYPERLINK \\l "chapter1"}{\fldrslt chapter}}, page {\field{\*\fldinst PAGE}{\fldrslt 5}}, {\field{\*\fldinst HYPERLINK "javascript:alert(1)"}{\fldrslt bad}}.\par}"##,
    );
    assert_eq!(f.plain_text(), "See the site and chapter, page 5, bad.");
    assert_eq!(f.links.get("l1").map(String::as_str), Some("http://example.com/a?b=1&c=2"));
    assert_eq!(f.links.len(), 1);
    assert!(f.body.contains(r#"<w:hyperlink r:id="l1">"#), "{}", f.body);
    assert!(f.body.contains(r#"<w:hyperlink w:anchor="chapter1">"#));
    assert_eq!(f.body.matches("<w:hyperlink").count(), 2);
    assert!(f.body.contains(r#"<w:rStyle w:val="Hyperlink"/>"#));
    assert!(f.styles.contains(r#"w:styleId="Hyperlink""#));
    // The same target gets the same key.
    let g = read(r#"{\rtf1 {\field{\*\fldinst HYPERLINK "http://a.org"}{\fldrslt a}} {\field{\*\fldinst HYPERLINK "http://a.org"}{\fldrslt b}}\par}"#);
    assert_eq!(g.links.len(), 1);
}

#[test]
fn run_properties_keep_schema_order_in_a_hyperlink() {
    let f = read(
        r#"{\rtf1\ansi\deff0{\fonttbl{\f0 Calibri;}{\f1 Arial;}}{\colortbl;\red255\green0\blue0;\red0\green0\blue255;\red255\green255\blue0;}
\pard {\field{\*\fldinst HYPERLINK "http://x.org"}{\fldrslt {\b\i\f1\cf1\fs28\highlight3\ul Link}}}\par}"#,
    );
    let rpr = f.parse().body[0].find("w:rPr").cloned().unwrap();
    let names: Vec<&str> = rpr.elements().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["w:rStyle", "w:rFonts", "w:b", "w:bCs", "w:i", "w:iCs", "w:color", "w:sz", "w:szCs", "w:highlight", "w:u"]);
}

#[test]
fn png_picture() {
    let f = read(&format!(r#"{{\rtf1\ansi\pard Before {{\pict\pngblip\picw1\pich1\picwgoal1500\pichgoal750 {}}} after\par}}"#, png_hex()));
    assert_eq!(f.media.len(), 1);
    assert_eq!(f.media.get("m1").unwrap().ext, "png");
    assert!(f.body.contains(r#"<a:blip r:embed="m1"/>"#), "{}", f.body);
    // 1500 twips is 75 pt, 750 twips is 37.5 pt.
    assert!(f.body.contains(r#"<wp:extent cx="952500" cy="476250"/>"#));
    assert_eq!(f.plain_text(), "Before  after");
    assert!(f.warnings.is_empty());
    // With a scale, and without a goal (a 1x1 picture is 0.75 pt, shown at least 1 pt).
    let g = read(&format!(r#"{{\rtf1\pard {{\pict\pngblip\picwgoal1000\pichgoal1000\picscalex50\picscaley200 {}}}\par}}"#, png_hex()));
    assert!(g.body.contains(r#"<wp:extent cx="317500" cy="1270000"/>"#), "{}", g.body);
    let h = read(&format!(r#"{{\rtf1\pard {{\pict\pngblip {}}}\par}}"#, png_hex()));
    assert!(h.body.contains(r#"<wp:extent cx="12700" cy="12700"/>"#), "{}", h.body);
}

#[test]
fn shape_picture_prefers_shppict_and_skips_nonshppict() {
    let hex = png_hex();
    let f = read(&format!(
        r#"{{\rtf1\ansi\pard {{\*\shppict{{\pict{{\*\picprop}}\picscalex100\picscaley100\picwgoal600\pichgoal600\pngblip\bliptag123 {hex}}}}}{{\nonshppict{{\pict\picscalex100\picwgoal600\emfblip 0100000058000000}}}}\par}}"#
    ));
    assert_eq!(f.media.len(), 1);
    assert!(f.warnings.is_empty(), "{:?}", f.warnings);
    assert!(f.body.contains("a:blip"));
}

#[test]
fn emf_and_wmf_pictures_warn() {
    let f = read(r#"{\rtf1\ansi\pard a{\pict\emfblip\picw100\pich100 0100000058000000}b{\pict\wmetafile8\picw10\pich10 010009000003}c\par}"#);
    assert!(f.media.is_empty());
    assert_eq!(f.plain_text(), "abc");
    assert_eq!(f.warnings.len(), 1);
    assert_eq!(f.warnings[0].kind, "picture-unsupported");
    assert_eq!(f.warnings[0].count, 2);
    // Pixels that are not a picture at all.
    let g = read(r#"{\rtf1\ansi\pard {\pict\pngblip 00ff00ff}\par}"#);
    assert_eq!(g.warnings[0].kind, "picture-unreadable");
}

#[test]
fn embedded_objects_are_dropped_with_a_warning() {
    let f = read(r#"{\rtf1\ansi\pard x{\object\objemb{\*\objclass Excel}{\*\objdata 0102}{\result {\pict\wmetafile8 0102}}}y\par}"#);
    assert_eq!(f.plain_text(), "xy");
    assert_eq!(f.warnings.len(), 1);
    assert_eq!(f.warnings[0].kind, "object-dropped");
}

#[test]
fn binary_picture_data() {
    let png = b64_decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==").unwrap();
    let bin: String = png.iter().map(|&b| b as char).collect();
    let f = read(&format!("{{\\rtf1\\pard {{\\pict\\pngblip\\bin{} {}}}x\\par}}", png.len(), bin));
    assert_eq!(f.media.len(), 1);
    assert_eq!(f.plain_text(), "x");
}

#[test]
fn footnotes_and_endnotes() {
    let f = read(
        r#"{\rtf1\ansi\deff0{\fonttbl{\f0 Arial;}}{\stylesheet{\s246 footnote text;}}
\pard Body text{\super\chftn{\footnote \pard\plain\s246 {\super\chftn} Note text here.\par}} continues{\super\chftn{\footnote\ftnalt \pard\plain {\super\chftn} The end.\par}}.\par}"#,
    );
    assert_eq!(f.plain_text(), "Body text continues.");
    assert_eq!(f.notes.len(), 2);
    let n1 = &f.notes["n1"];
    assert_eq!(n1.kind, "footnote");
    assert!(n1.xml.starts_with(r#"<w:footnote w:id="n1">"#), "{}", n1.xml);
    assert!(n1.xml.contains("<w:footnoteRef/>"));
    assert!(n1.xml.contains(r#"<w:pStyle w:val="FootnoteText"/>"#));
    assert!(n1.xml.contains("Note text here."));
    let n2 = &f.notes["n2"];
    assert_eq!(n2.kind, "endnote");
    assert!(n2.xml.contains("<w:endnoteRef/>"));
    assert!(n2.xml.starts_with(r#"<w:endnote w:id="n2">"#));
    assert!(f.body.contains(r#"<w:footnoteReference w:id="n1"/>"#));
    assert!(f.body.contains(r#"<w:endnoteReference w:id="n2"/>"#));
    assert!(!f.body.contains(r#"w:footnoteReference w:id="n2""#));
    assert!(f.body.contains(r#"<w:rStyle w:val="FootnoteReference"/>"#));
    assert!(f.body.contains(r#"<w:rStyle w:val="EndnoteReference"/>"#));
    assert!(f.styles.contains(r#"w:styleId="FootnoteReference""#));
    assert!(f.styles.contains(r#"w:styleId="FootnoteText""#));
    assert!(f.styles.contains(r#"w:styleId="EndnoteText""#));
    assert!(f.ns.iter().any(|(p, _)| p == "w"));
}

// ---------------------------------------------------------------------------
// Garbage in
// ---------------------------------------------------------------------------

#[test]
fn unbalanced_and_truncated_input() {
    let cases = [
        "",
        "{",
        "}",
        "}}}}",
        "\\",
        "\\'",
        "\\'4",
        "{\\rtf1\\u",
        "{\\rtf1\\u-",
        "{\\rtf1 \\bin5 ab",
        "{\\rtf1\\bin99999999999 x}",
        "{\\rtf1\\b99999999999999999999999 x\\par}",
        "{\\rtf1\\ansi{\\fonttbl{\\f0 Arial;}}\\pard Hello {\\b wor",
        "{\\rtf1 a}}}}} b {{{ c",
        "{\\rtf1{\\pict\\pngblip 89504e47",
        "{\\rtf1\\trowd\\cellx",
        "{\\rtf1\\trowd\\cellx1000\\intbl x\\cell",
        "{\\rtf1{\\field{\\*\\fldinst HYPERLINK \"http://x\"}{\\fldrslt link",
        "{\\rtf1{\\footnote text",
        "{\\rtf1{\\stylesheet{\\s1 heading 1",
        "{\\rtf1{\\*\\listtable{\\list{\\listlevel{\\leveltext",
        "{\\rtf1\\row\\cell\\row\\cell",
        "{\\rtf1 {\\*\\",
        "{\\rtf1 {\\*",
        "just some text with no rtf at all",
    ];
    for c in cases {
        let f = rtf_to_fragment(c);
        check(&f);
    }
    assert_eq!(read("{\\rtf1\\ansi{\\fonttbl{\\f0 Arial;}}\\pard Hello {\\b wor").plain_text(), "Hello wor");
    assert_eq!(read("{\\rtf1 a}}}}} b {{{ c").plain_text(), "a");
    assert_eq!(rtf_to_fragment("just some text").plain_text(), "");
}

#[test]
fn deeply_nested_braces() {
    let deep = format!("{{\\rtf1\\ansi {}x{}", "{\\b ".repeat(100_000), "}".repeat(100_000));
    let f = read(&deep);
    assert_eq!(f.plain_text(), "");
    // Content after the deep part is read again once the groups close.
    let after = format!("{{\\rtf1\\ansi before {}x{} after\\par}}", "{".repeat(5_000), "}".repeat(5_000));
    assert_eq!(read(&after).plain_text(), "before  after");
    // Unclosed bomb.
    let open = format!("{{\\rtf1 {}", "{".repeat(1_000_000));
    check(&rtf_to_fragment(&open));
    // Nested destinations.
    let dest = format!("{{\\rtf1 {}{}", "{\\footnote ".repeat(10_000), "x");
    check(&rtf_to_fragment(&dest));
    let tbl = format!("{{\\rtf1 {}", "\\trowd\\cellx100\\intbl x\\cell\\row ".repeat(20_000));
    check(&rtf_to_fragment(&tbl));
}

#[test]
fn random_token_soup_never_panics() {
    let toks = [
        "{", "}", "\\par ", "\\b ", "\\i0 ", "\\pard ", "\\trowd ", "\\cellx1000", "\\cell ", "\\row ", "\\intbl ", "\\pict ", "\\pngblip ", "\\emfblip ", "\\field ",
        "\\fldinst ", "\\fldrslt ", "\\footnote ", "\\ftnalt ", "\\chftn ", "\\*", "\\'e9", "\\'", "\\u8364?", "\\u-3913 ", "\\uc0 ", "\\ls1", "\\ilvl3", "\\list ",
        "\\listlevel ", "\\leveltext ", "\\listoverride ", "\\lfolevel ", "\\stylesheet ", "\\s1", "\\cs2", "\\sbasedon1", "\\fonttbl ", "\\f1", "\\colortbl ", "\\red1",
        ";", "text", "HYPERLINK \"x\"", "\\pntext ", "\\pn ", "\\nestcell ", "\\clmrg ", "\\clvmrg ", "\\brdrt ", "\\box ", "\\tx100 ", "\\bin3 abc", "00ff", "\\object ",
        "\\shp ", "\\sv ", "\\\\", "\\{", "\\}", "\\~", "\\-",
    ];
    let mut seed: u64 = 0x9E3779B97F4A7C15;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..400 {
        let mut s = String::from("{\\rtf1\\ansi ");
        for _ in 0..(next() % 120) {
            s.push_str(toks[(next() % toks.len() as u64) as usize]);
        }
        let f = rtf_to_fragment(&s);
        check(&f);
    }
}

#[test]
fn a_big_document_reads_quickly() {
    let mut s = String::from(r#"{\rtf1\ansi\deff0{\fonttbl{\f0 Arial;}}{\colortbl;\red255\green0\blue0;}"#);
    let para = r#"\pard\plain\ql\sa200\sl276\slmult1\f0\fs22 Some {\b bold} text, {\i italic} here and {\cf1\ul more} words to make it longer, plus a tab\tab and \'e9\u8364? marks.\par
"#;
    while s.len() < 2_000_000 {
        s.push_str(para);
    }
    s.push('}');
    let start = std::time::Instant::now();
    let f = rtf_to_fragment(&s);
    let took = start.elapsed();
    eprintln!("2 MB RTF read in {took:?}");
    assert!(f.plain_text().starts_with("Some bold text"));
    assert!(paras(&f) > 10_000);
    // Debug builds are slow; the release build is far below a second.
    assert!(took.as_secs_f64() < 20.0, "took {took:?}");
}

#[test]
fn defaults_are_reported() {
    let f = read(r#"{\rtf1\ansi\deff1{\fonttbl{\f0 Arial;}{\f1 Georgia;}}\pard x\par}"#);
    assert_eq!(f.defaults.as_ref().and_then(|d| d.font.as_deref()), Some("Georgia"));
    // Georgia is the default font, so only a run that picks Arial writes a font.
    assert!(!f.body.contains("Georgia"));
    let g = read(r#"{\rtf1\ansi\deff1{\fonttbl{\f0 Arial;}{\f1 Georgia;}}\pard x{\f0 y}\par}"#);
    assert!(g.body.contains(r#"w:ascii="Arial""#));
}

#[test]
fn the_last_paragraph_keeps_its_formatting_without_a_paragraph_mark() {
    let f = read(r#"{\rtf1\ansi\pard\qc\b hello}"#);
    assert_eq!(f.plain_text(), "hello");
    assert!(f.body.contains(r#"<w:jc w:val="center"/>"#), "{}", f.body);
    assert!(f.body.contains("<w:b/>"));
    // Also when the text is cut off inside groups.
    let g = read(r#"{\rtf1\ansi\pard\qr {\i hello"#);
    assert!(g.body.contains(r#"<w:jc w:val="right"/>"#), "{}", g.body);
}

#[test]
fn style_aliases_use_the_first_name() {
    let f = read(r#"{\rtf1\ansi{\stylesheet{\s1 heading 1,h1;}{\s2 My Style,ms,alias;}}\pard\s1 a\par\pard\s2 b\par}"#);
    assert!(f.styles.contains(r#"w:styleId="Heading1""#), "{}", f.styles);
    assert!(f.styles.contains(r#"w:styleId="MyStyle""#));
    assert!(f.styles.contains(r#"<w:name w:val="My Style"/>"#));
}

#[test]
fn namespaces_cover_links_pictures_and_notes() {
    let f = read(&format!(
        r#"{{\rtf1\ansi\pard {{\field{{\*\fldinst HYPERLINK "http://x.org"}}{{\fldrslt a}}}}{{\pict\pngblip {}}}{{\super\chftn{{\footnote \pard {{\chftn}} n\par}}}}\par}}"#,
        png_hex()
    ));
    for want in ["w", "r", "wp", "a", "pic"] {
        assert!(f.ns.iter().any(|(p, _)| p == want), "{want} missing: {:?}", f.ns);
    }
}
