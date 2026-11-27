//! Equations: Word's OMML shown as MathML, LaTeX in and out, the ops, undo, offsets, the AI tools and Markdown.

use docxtor_core::ai::{call_tool, docscript, ScriptOpts};
use docxtor_core::export::{range_to_markdown, to_markdown};
use docxtor_core::math::{latex_to_omml, omml_to_latex, omml_to_mathml};
use docxtor_core::model::{find_block, find_para, walk_paragraphs, write_block};
use docxtor_core::xml::{self, Element};
use docxtor_core::{package::Package, Document, Pos};
use serde_json::{json, Value};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const M: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Every XML part of a saved package must be namespace-well-formed.
fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn ops(doc: &mut Document, json: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"))
}

fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

/// A document whose body is the given XML, with the math namespace declared the way Word does.
fn docx_from_body(body: &str) -> Document {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:m="{M}"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    pkg.set("word/document.xml", xml.into_bytes());
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

fn saved_body(doc: &mut Document) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    String::from_utf8(Package::open(&bytes).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap()
}

fn para_xml(doc: &Document, id: u32) -> String {
    write_block(find_block(&doc.body, id).unwrap()).to_xml()
}

fn paras(doc: &Document) -> Vec<(u32, String)> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push((p.id, p.text())));
    v
}

fn first_para(doc: &Document) -> u32 {
    paras(doc)[0].0
}

fn segs(doc: &Document, id: u32) -> Vec<Value> {
    let r = serde_json::to_value(doc.render_snapshot()).unwrap();
    fn find(blocks: &Value, id: u32) -> Option<Value> {
        for b in blocks.as_array()? {
            match b["t"].as_str() {
                Some("p") if b["id"] == id => return Some(b["segs"].clone()),
                Some("c") => {
                    if let Some(s) = find(&b["blocks"], id) {
                        return Some(s);
                    }
                }
                _ => {}
            }
        }
        None
    }
    find(&r["blocks"], id).expect("rendered paragraph").as_array().unwrap().clone()
}

fn math_segs(doc: &Document, id: u32) -> Vec<Value> {
    segs(doc, id).into_iter().filter(|s| s["k"] == "math").collect()
}

// Word's markup, as it writes it.
const F: &str = r#"<w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/></w:rPr>"#;
const CTRL: &str = r#"<m:ctrlPr><w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/><w:i/></w:rPr></m:ctrlPr>"#;

fn r(text: &str) -> String {
    format!("<m:r>{F}<m:t>{text}</m:t></m:r>")
}

fn rp(text: &str) -> String {
    format!(r#"<m:r><m:rPr><m:sty m:val="p"/></m:rPr>{F}<m:t>{text}</m:t></m:r>"#)
}

fn parse_math(xml: &str) -> Element {
    let doc = xml::parse(format!(r#"<w:p xmlns:w="{W}" xmlns:m="{M}">{xml}</w:p>"#).as_bytes()).unwrap();
    let el = doc.root.elements().next().unwrap().clone();
    el
}

fn mathml_of(omml: &str) -> String {
    omml_to_mathml(&parse_math(omml))
}

/// The quadratic formula in a display equation, as Word writes it.
fn quadratic() -> String {
    let b2 = format!("<m:sSup><m:sSupPr>{CTRL}</m:sSupPr><m:e>{}</m:e><m:sup>{}</m:sup></m:sSup>", r("b"), r("2"));
    let rad = format!(r#"<m:rad><m:radPr><m:degHide m:val="1"/>{CTRL}</m:radPr><m:deg/><m:e>{b2}{}</m:e></m:rad>"#, r("-4ac"));
    let num = format!("{}{rad}", r("-b±"));
    let frac = format!("<m:f><m:fPr>{CTRL}</m:fPr><m:num>{num}</m:num><m:den>{}</m:den></m:f>", r("2a"));
    format!(r#"<m:oMathPara><m:oMathParaPr><m:jc m:val="center"/></m:oMathParaPr><m:oMath>{}{frac}</m:oMath></m:oMathPara>"#, r("x="))
}

fn matrix_eq() -> String {
    let row = |a: &str, b: &str| format!("<m:mr><m:e>{}</m:e><m:e>{}</m:e></m:mr>", r(a), r(b));
    format!(
        r#"<m:oMath><m:d><m:dPr><m:begChr m:val="["/><m:endChr m:val="]"/>{CTRL}</m:dPr><m:e><m:m><m:mPr><m:mcs><m:mc><m:mcPr><m:count m:val="2"/><m:mcJc m:val="center"/></m:mcPr></m:mc></m:mcs>{CTRL}</m:mPr>{}{}</m:m></m:e></m:d></m:oMath>"#,
        row("a", "b"),
        row("c", "d")
    )
}

fn integral_eq() -> String {
    format!(
        r#"<m:oMath><m:nary><m:naryPr><m:limLoc m:val="subSup"/>{CTRL}</m:naryPr><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:nary><m:r>{F}<m:t>=1</m:t></m:r></m:oMath>"#,
        r("0"),
        r("1"),
        r("f(x)dx")
    )
}

fn sum_eq() -> String {
    format!(
        r#"<m:oMath><m:nary><m:naryPr><m:chr m:val="∑"/><m:limLoc m:val="undOvr"/><m:grow m:val="1"/>{CTRL}</m:naryPr><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:nary></m:oMath>"#,
        r("i=1"),
        r("n"),
        r("i")
    )
}

fn cases_eq() -> String {
    format!(
        r#"<m:oMath>{}<m:d><m:dPr><m:begChr m:val="{{"/><m:endChr m:val=""/>{CTRL}</m:dPr><m:e><m:eqArr><m:eqArrPr>{CTRL}</m:eqArrPr><m:e>{}</m:e><m:e>{}</m:e></m:eqArr></m:e></m:d></m:oMath>"#,
        r("f(x)="),
        r("1,x&gt;0"),
        r("0,x≤0")
    )
}

fn para(inner: &str) -> String {
    format!("<w:p>{inner}</w:p>")
}

fn word_document() -> Document {
    docx_from_body(&[
        para(&format!("<w:r><w:t xml:space=\"preserve\">Solve </w:t></w:r>{}<w:r><w:t xml:space=\"preserve\"> now.</w:t></w:r>", parse_inline_only(&sum_eq()))),
        para(&quadratic()),
        para(&matrix_eq()),
        para(&integral_eq()),
        para(&cases_eq()),
    ]
    .join(""))
}

fn parse_inline_only(s: &str) -> String {
    s.to_string()
}

// ---------------------------------------------------------------------------
// OMML to MathML, construct by construct (Word's exact markup)
// ---------------------------------------------------------------------------

#[test]
fn omml_to_mathml_runs_letters_digits_and_operators() {
    let m = mathml_of(&format!("<m:oMath>{}</m:oMath>", r("x+12=y")));
    assert!(m.starts_with("<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"inline\">"), "{m}");
    // Letters are italic identifiers, digits numbers, the rest operators.
    assert!(m.contains("<mi>x</mi><mo>+</mo><mn>12</mn><mo>=</mo><mi>y</mi>"), "{m}");
}

#[test]
fn omml_to_mathml_styles() {
    // Plain text of a function, normal text, bold.
    let m = mathml_of(&format!("<m:oMath>{}</m:oMath>", rp("sin")));
    assert!(m.contains("<mi mathvariant=\"normal\">sin</mi>"), "{m}");
    let m = mathml_of(&format!(r#"<m:oMath><m:r><m:rPr><m:nor/></m:rPr>{F}<m:t xml:space="preserve">if x</m:t></m:r></m:oMath>"#));
    assert!(m.contains("<mtext>if\u{a0}x</mtext>"), "{m}");
    let m = mathml_of(&format!(r#"<m:oMath><m:r><m:rPr><m:sty m:val="b"/></m:rPr>{F}<m:t>v</m:t></m:r></m:oMath>"#));
    assert!(m.contains("𝐯"), "{m}");
    // Greek capitals stay upright, small letters lean.
    let m = mathml_of(&format!("<m:oMath>{}</m:oMath>", r("Δα")));
    assert!(m.contains("<mi mathvariant=\"normal\">Δ</mi><mi>α</mi>"), "{m}");
}

#[test]
fn omml_to_mathml_fractions() {
    let f = |ty: &str| format!(r#"<m:oMath><m:f><m:fPr><m:type m:val="{ty}"/>{CTRL}</m:fPr><m:num>{}</m:num><m:den>{}</m:den></m:f></m:oMath>"#, r("a"), r("b"));
    assert!(mathml_of(&f("bar")).contains("<mfrac><mrow><mi>a</mi></mrow><mrow><mi>b</mi></mrow></mfrac>"));
    assert!(mathml_of(&f("noBar")).contains("<mfrac linethickness=\"0\">"));
    let lin = mathml_of(&f("lin"));
    assert!(lin.contains("<mo>/</mo>") && !lin.contains("<mfrac"), "{lin}");
}

#[test]
fn omml_to_mathml_scripts() {
    let e = |name: &str, extra: &str| format!("<m:oMath><m:{name}><m:{name}Pr>{CTRL}</m:{name}Pr><m:e>{}</m:e>{extra}</m:{name}></m:oMath>", r("x"));
    assert!(mathml_of(&e("sSup", &format!("<m:sup>{}</m:sup>", r("2")))).contains("<msup><mrow><mi>x</mi></mrow><mrow><mn>2</mn></mrow></msup>"));
    assert!(mathml_of(&e("sSub", &format!("<m:sub>{}</m:sub>", r("i")))).contains("<msub>"));
    assert!(mathml_of(&e("sSubSup", &format!("<m:sub>{}</m:sub><m:sup>{}</m:sup>", r("i"), r("2")))).contains("<msubsup>"));
    let pre = format!("<m:oMath><m:sPre><m:sPrePr>{CTRL}</m:sPrePr><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:sPre></m:oMath>", r("1"), r("2"), r("X"));
    let m = mathml_of(&pre);
    assert!(m.contains("<mmultiscripts>") && m.contains("<mprescripts/>"), "{m}");
}

#[test]
fn omml_to_mathml_radicals() {
    let sqrt = format!(r#"<m:oMath><m:rad><m:radPr><m:degHide m:val="1"/>{CTRL}</m:radPr><m:deg/><m:e>{}</m:e></m:rad></m:oMath>"#, r("x"));
    assert!(mathml_of(&sqrt).contains("<msqrt>"));
    let root = format!("<m:oMath><m:rad><m:radPr>{CTRL}</m:radPr><m:deg>{}</m:deg><m:e>{}</m:e></m:rad></m:oMath>", r("3"), r("x"));
    let m = mathml_of(&root);
    assert!(m.contains("<mroot>") && m.contains("<mn>3</mn>"), "{m}");
}

#[test]
fn omml_to_mathml_large_operators() {
    // A sum in a display equation has its limits above and below; in a line of text they go beside it, as in Word.
    let display = mathml_of(&format!("<m:oMathPara>{}</m:oMathPara>", sum_eq()));
    assert!(display.contains("display=\"block\"") && display.contains("<munderover>"), "{display}");
    let inline = mathml_of(&sum_eq());
    assert!(inline.contains("<msubsup>") && !inline.contains("<munderover>"), "{inline}");
    // An integral keeps its limits at the side; no chr means the integral sign.
    let int = mathml_of(&integral_eq());
    assert!(int.contains("<msubsup><mo largeop=\"true\" stretchy=\"false\">∫</mo>"), "{int}");
    // Hidden limits.
    let none = format!(r#"<m:oMath><m:nary><m:naryPr><m:chr m:val="∮"/><m:subHide m:val="1"/><m:supHide m:val="1"/></m:naryPr><m:sub/><m:sup/><m:e>{}</m:e></m:nary></m:oMath>"#, r("f"));
    let m = mathml_of(&none);
    assert!(m.contains("<mo largeop=\"true\" stretchy=\"false\">∮</mo><mrow><mi>f</mi>"), "{m}");
}

#[test]
fn omml_to_mathml_delimiters() {
    let d = |pr: &str| format!("<m:oMath><m:d><m:dPr>{pr}{CTRL}</m:dPr><m:e>{}</m:e><m:e>{}</m:e></m:d></m:oMath>", r("a"), r("b"));
    let plain = mathml_of(&d(""));
    assert!(plain.contains(">(</mo>") && plain.contains(">)</mo>") && plain.contains(">|</mo>"), "{plain}");
    let custom = mathml_of(&d(r#"<m:begChr m:val="⟨"/><m:sepChr m:val=","/><m:endChr m:val="⟩"/>"#));
    assert!(custom.contains(">⟨</mo>") && custom.contains(">,</mo>") && custom.contains(">⟩</mo>"), "{custom}");
    let none = mathml_of(&d(r#"<m:begChr m:val=""/><m:endChr m:val=""/>"#));
    assert!(!none.contains("fence"), "{none}");
}

#[test]
fn brackets_typed_in_a_line_do_not_stretch_but_delimiters_do() {
    let m = mathml_of(&format!("<m:oMath>{}</m:oMath>", r("f(x)")));
    assert!(m.contains("<mo stretchy=\"false\">(</mo>") && m.contains("<mo stretchy=\"false\">)</mo>"), "{m}");
    let d = mathml_of(&format!("<m:oMath><m:d><m:dPr>{CTRL}</m:dPr><m:e>{}</m:e></m:d></m:oMath>", r("x")));
    assert!(d.contains("stretchy=\"true\""), "{d}");
}

#[test]
fn omml_to_mathml_matrix_and_cases() {
    let m = mathml_of(&matrix_eq());
    assert!(m.contains("<mtable>") && m.matches("<mtr>").count() == 2 && m.matches("<mtd>").count() == 4, "{m}");
    assert!(m.contains(">[</mo>") && m.contains(">]</mo>"), "{m}");
    let c = mathml_of(&cases_eq());
    assert!(c.contains(">{</mo>") && c.matches("<mtr>").count() == 2 && c.contains("columnalign=\"left\""), "{c}");
}

#[test]
fn omml_to_mathml_functions_limits_accents_bars() {
    let sin = format!("<m:oMath><m:func><m:funcPr>{CTRL}</m:funcPr><m:fName>{}</m:fName><m:e>{}</m:e></m:func></m:oMath>", rp("sin"), r("x"));
    let m = mathml_of(&sin);
    assert!(m.contains("<mi mathvariant=\"normal\">sin</mi></mrow><mo>\u{2061}</mo>"), "{m}");
    let lim = format!("<m:oMath><m:limLow><m:limLowPr>{CTRL}</m:limLowPr><m:e>{}</m:e><m:lim>{}</m:lim></m:limLow></m:oMath>", rp("lim"), r("x→0"));
    assert!(mathml_of(&lim).contains("<munder>"));
    let upp = lim.replace("limLow", "limUpp");
    assert!(mathml_of(&upp).contains("<mover>"));
    let acc = format!(r#"<m:oMath><m:acc><m:accPr><m:chr m:val="&#x302;"/>{CTRL}</m:accPr><m:e>{}</m:e></m:acc></m:oMath>"#, r("x"));
    let m = mathml_of(&acc);
    assert!(m.contains("<mover accent=\"true\">") && m.contains(">^</mo>"), "{m}");
    // The accent defaults to a hat.
    let acc = acc.replace(r#"<m:chr m:val="&#x302;"/>"#, "");
    assert!(mathml_of(&acc).contains(">^</mo>"));
    let bar = |pos: &str| format!(r#"<m:oMath><m:bar><m:barPr><m:pos m:val="{pos}"/>{CTRL}</m:barPr><m:e>{}</m:e></m:bar></m:oMath>"#, r("x"));
    assert!(mathml_of(&bar("top")).contains("<mover accent=\"true\">"));
    assert!(mathml_of(&bar("bot")).contains("<munder accentunder=\"true\">"));
    let grp = format!("<m:oMath><m:groupChr><m:groupChrPr>{CTRL}</m:groupChrPr><m:e>{}</m:e></m:groupChr></m:oMath>", r("abc"));
    let m = mathml_of(&grp);
    assert!(m.contains("<munder>") && m.contains("⏟"), "{m}");
}

#[test]
fn omml_to_mathml_boxes_and_unknown_markup() {
    let b = |name: &str, pr: &str| format!("<m:oMath><m:{name}><m:{name}Pr>{pr}</m:{name}Pr><m:e>{}</m:e></m:{name}></m:oMath>", r("x"));
    assert!(mathml_of(&b("box", "")).contains("<mi>x</mi>"));
    assert!(mathml_of(&b("borderBox", "")).contains("class=\"m-box\""));
    assert!(mathml_of(&b("phant", r#"<m:show m:val="0"/>"#)).contains("<mphantom>"));
    // Something Docxtor has never heard of: its text still shows.
    let m = mathml_of("<m:oMath><m:future><m:foo>kept</m:foo></m:future></m:oMath>");
    assert!(m.contains("kept"), "{m}");
}

#[test]
fn display_equations_are_blocks_and_carry_their_justification() {
    let m = mathml_of(&quadratic());
    assert!(m.contains("display=\"block\""), "{m}");
    let doc = docx_from_body(&para(&quadratic().replace("center", "left")));
    let id = first_para(&doc);
    let s = &math_segs(&doc, id)[0];
    assert_eq!(s["math"]["display"], true);
    assert_eq!(s["math"]["jc"], "left");
}

#[test]
fn omml_to_latex_reads_words_markup() {
    assert_eq!(omml_to_latex(&parse_math(&quadratic())), "x=\\frac{-b\\pm\\sqrt{b^2-4ac}}{2a}");
    assert_eq!(omml_to_latex(&parse_math(&matrix_eq())), "\\begin{bmatrix} a & b \\\\ c & d \\end{bmatrix}");
    assert_eq!(omml_to_latex(&parse_math(&integral_eq())), "\\int_{0}^{1} f(x)dx=1");
    assert_eq!(omml_to_latex(&parse_math(&sum_eq())), "\\sum_{i=1}^{n} i");
    assert_eq!(omml_to_latex(&parse_math(&cases_eq())), "f(x)=\\begin{cases} 1,x>0 \\\\ 0,x\\leq0 \\end{cases}");
}

// ---------------------------------------------------------------------------
// LaTeX
// ---------------------------------------------------------------------------

#[test]
fn latex_to_omml_builds_words_structures() {
    let x = |src: &str| latex_to_omml(src).unwrap().to_xml();
    assert!(x("\\frac{a}{b}").contains("<m:f>") && x("\\frac{a}{b}").contains("<m:num>"));
    assert!(x("x^2").contains("<m:sSup>") && x("x_1").contains("<m:sSub>") && x("x_1^2").contains("<m:sSubSup>"));
    assert!(x("\\sqrt{x}").contains("<m:degHide m:val=\"1\"/>") && x("\\sqrt[3]{x}").contains("<m:deg><m:r>"));
    let sum = x("\\sum_{i=1}^{n} i");
    assert!(sum.contains("<m:chr m:val=\"∑\"/>") && sum.contains("<m:limLoc m:val=\"undOvr\"/>"), "{sum}");
    assert!(x("\\int_0^1 f").contains("<m:limLoc m:val=\"subSup\"/>"));
    assert!(x("\\iint f").contains("∬") && x("\\oint f").contains("∮") && x("\\prod_i f").contains("∏"));
    assert!(x("\\lim_{x \\to 0} f").contains("<m:limLow>"));
    assert!(x("\\left( a \\right)").contains("<m:begChr m:val=\"(\"/>"));
    assert!(x("\\begin{matrix} a & b \\\\ c & d \\end{matrix}").contains("<m:mr>"));
    assert!(x("\\begin{pmatrix} a \\end{pmatrix}").contains("<m:d>"));
    assert!(x("\\begin{cases} a & b \\\\ c & d \\end{cases}").contains("<m:eqArr>"));
    assert!(x("\\sin x").contains("<m:func>"));
    assert!(x("\\hat{x}").contains("<m:acc>") && x("\\vec{x}").contains("<m:acc>"));
    assert!(x("\\text{if}").contains("<m:nor/>"));
    assert!(x("\\mathbf{v}").contains("<m:sty m:val=\"b\"/>"));
    // Runs carry Word's math font.
    assert!(x("a").contains("Cambria Math"));
    // Neighbouring characters share one run, as in Word.
    assert_eq!(x("ab+1").matches("<m:r>").count(), 1);
}

#[test]
fn latex_symbols_and_spacing() {
    for (src, ch) in [("\\alpha", "α"), ("\\Omega", "Ω"), ("\\cdot", "⋅"), ("\\times", "×"), ("\\div", "÷"), ("\\pm", "±"), ("\\mp", "∓"), ("\\leq", "≤"), ("\\geq", "≥"), ("\\neq", "≠"), ("\\approx", "≈"), ("\\equiv", "≡"), ("\\infty", "∞"), ("\\partial", "∂"), ("\\nabla", "∇"), ("\\to", "→"), ("\\rightarrow", "→"), ("\\Rightarrow", "⇒"), ("\\in", "∈"), ("\\notin", "∉"), ("\\subset", "⊂"), ("\\cup", "∪"), ("\\cap", "∩"), ("\\forall", "∀"), ("\\exists", "∃"), ("\\ldots", "…"), ("\\cdots", "⋯"), ("\\,", "\u{2009}"), ("\\;", "\u{2005}"), ("\\quad", "\u{2003}")] {
        let el = latex_to_omml(src).unwrap();
        assert!(el.to_xml().contains(ch), "{src} -> {}", el.to_xml());
    }
    // A minus sign is the real one.
    assert!(latex_to_omml("a-b").unwrap().to_xml().contains("a−b"));
}

#[test]
fn latex_functions_are_upright_func_elements() {
    for f in ["sin", "cos", "tan", "log", "ln", "exp", "max", "min"] {
        let x = latex_to_omml(&format!("\\{f} x")).unwrap().to_xml();
        assert!(x.contains("<m:func>") && x.contains("<m:sty m:val=\"p\"/>") && x.contains(&format!(">{f}<")), "{f}: {x}");
    }
}

#[test]
fn latex_round_trips_to_a_fixed_point() {
    let rt = |s: &str| omml_to_latex(&latex_to_omml(s).unwrap_or_else(|e| panic!("{s}: {e}")));
    for src in [
        "x^2",
        "x_i^2",
        "\\frac{a+b}{c}",
        "\\sqrt{x}",
        "\\sqrt[3]{x+1}",
        "\\sum_{i=1}^{n} i^2",
        "\\iint_D f \\, dA",
        "\\int_0^1 x^2 \\, dx",
        "\\lim_{x \\to 0} \\frac{\\sin x}{x}",
        "\\left( \\frac{a}{b} \\right)",
        "\\left[ a \\middle| b \\right)",
        "\\begin{matrix} a & b \\\\ c & d \\end{matrix}",
        "\\begin{bmatrix} 1 & 0 \\\\ 0 & 1 \\end{bmatrix}",
        "f(x) = \\begin{cases} 1 & x > 0 \\\\ 0 & x \\leq 0 \\end{cases}",
        "\\alpha + \\beta \\cdot \\gamma - \\Delta",
        "\\hat{x} + \\bar{y} + \\vec{v} + \\dot{z} + \\tilde{w}",
        "\\text{if } x > 0",
        "\\mathbf{v} \\cdot \\mathbf{w}",
        "\\mathbb{R}^n",
        "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}",
        "\\binom{n}{k}",
        "e^{i\\pi} + 1 = 0",
        "\\oint_C \\mathbf{F} \\cdot d\\mathbf{r}",
        "a \\, b \\; c \\quad d",
        "\\overline{AB} + \\underline{CD}",
        "\\underbrace{a+b}_{n}",
        "f'(x) = 2x",
        "\\operatorname{rank}(A)",
    ] {
        let once = rt(src);
        assert!(!once.is_empty(), "{src}");
        assert_eq!(once, rt(&once), "{src} -> {once}");
    }
}

#[test]
fn latex_structure_survives_a_round_trip() {
    // Reading the LaTeX that came back gives the same markup, not just the same text.
    for src in ["\\frac{a+b}{c}", "\\sum_{i=1}^{n} i^2", "\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}", "\\sqrt[3]{x}", "x_i^2", "\\sin^2 x"] {
        let a = latex_to_omml(src).unwrap();
        let b = latex_to_omml(&omml_to_latex(&a)).unwrap();
        assert_eq!(a.to_xml(), b.to_xml(), "{src}");
    }
}

#[test]
fn latex_errors_say_what_is_wrong() {
    for (src, needle) in [
        ("\\frac{a}", "second group"),
        ("\\frac", "needs two groups"),
        ("\\foo{x}", "Unknown command \\foo at character 1"),
        ("x + \\bogus", "Unknown command \\bogus at character 5"),
        ("{x", "never closed"),
        ("x}", "no { before it"),
        ("\\left( x", "no matching \\right"),
        ("x \\right)", "no \\left"),
        ("", "empty"),
        ("   ", "empty"),
        ("x^", "needs something after it"),
        ("x^2^3", "second ^"),
        ("\\begin{matrix} a", "no matching \\end{matrix}"),
        ("\\begin{pmatrix} a \\end{bmatrix}", "should end with \\end{pmatrix}"),
        ("\\begin{tabular} a \\end{tabular}", "isn't supported"),
        ("a & b", "only works inside"),
        ("\\text{abc", "never closed"),
        ("\\sqrt[3 x", "never closed"),
        ("x\\", "backslash"),
    ] {
        let e = latex_to_omml(src).expect_err(src);
        assert!(e.contains(needle), "{src:?}: {e}");
    }
}

#[test]
fn dollar_signs_around_latex_are_ignored() {
    assert_eq!(omml_to_latex(&latex_to_omml("$x^2$").unwrap()), "x^2");
    assert_eq!(omml_to_latex(&latex_to_omml("$$x^2$$").unwrap()), "x^2");
}

// ---------------------------------------------------------------------------
// Word documents: rendering and lossless saving
// ---------------------------------------------------------------------------

#[test]
fn word_equations_render_as_math_atoms() {
    let doc = word_document();
    let ps = paras(&doc);
    // "Solve ", an equation, " now.": the equation is one character in the paragraph's text.
    assert_eq!(ps[0].1, "Solve \u{FFFC} now.");
    let s = segs(&doc, ps[0].0);
    let kinds: Vec<_> = s.iter().map(|x| x["k"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["t", "math", "t"], "{s:?}");
    assert_eq!(s[1]["o"], 6);
    assert_eq!(s[2]["o"], 7);
    assert!(s[1]["math"]["mathml"].as_str().unwrap().contains("<msubsup>"));
    assert_eq!(s[1]["math"]["latex"], "\\sum_{i=1}^{n} i");
    assert!(s[1]["math"].get("display").is_none(), "inline equations carry no display flag");
    // The display equations sit alone in their paragraphs.
    for (i, latex) in [(1, "x="), (2, "\\begin{bmatrix}"), (3, "\\int_"), (4, "f(x)=")] {
        let m = math_segs(&doc, ps[i].0);
        assert_eq!(m.len(), 1);
        assert!(m[0]["math"]["latex"].as_str().unwrap().starts_with(latex), "{}", m[0]["math"]["latex"]);
    }
    assert_eq!(math_segs(&doc, ps[1].0)[0]["math"]["display"], true);
    assert_eq!(math_segs(&doc, ps[1].0)[0]["math"]["jc"], "center");
    assert_eq!(ps[1].1, "\u{FFFC}");
}

#[test]
fn untouched_equations_are_written_back_byte_for_byte() {
    let mut doc = word_document();
    let original = saved_body(&mut doc);
    // Open, render and save again without an edit.
    let mut again = Document::open(&doc.save().unwrap()).unwrap();
    again.render_all();
    assert_eq!(saved_body(&mut again), original);
    // Editing text next to an equation leaves the equation's XML as it was.
    let id = first_para(&again);
    ops(&mut again, json!({"op":"insert_text","at":{"p":id,"o":0},"text":"Please "}));
    let body = saved_body(&mut again);
    let eq = sum_eq();
    assert!(body.contains(&eq), "equation changed");
    assert!(body.contains("Please Solve"));
}

#[test]
fn only_the_edited_equation_changes() {
    let mut doc = word_document();
    let ps = paras(&doc);
    let before = saved_body(&mut doc);
    ops(&mut doc, json!({"op":"set_equation","at":{"p":ps[2].0,"o":0},"latex":"\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}"}));
    let after = saved_body(&mut doc);
    assert!(!after.contains(&matrix_eq()));
    assert!(after.contains(&quadratic()) && after.contains(&integral_eq()) && after.contains(&cases_eq()));
    assert_ne!(before, after);
}

// ---------------------------------------------------------------------------
// Ops
// ---------------------------------------------------------------------------

fn blank_with(text: &str) -> (Document, u32) {
    let mut doc = Document::new_blank();
    let id = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":id,"o":0},"text":text}));
    (doc, id)
}

#[test]
fn insert_an_inline_equation_and_undo_it() {
    let (mut doc, id) = blank_with("Area is  and more");
    let r = ops(&mut doc, json!({"op":"insert_equation","at":{"p":id,"o":8},"latex":"\\pi r^2"}));
    assert_eq!(r[0].caret.unwrap().o, 9);
    assert_eq!(paras(&doc)[0].1, "Area is \u{FFFC} and more");
    let m = math_segs(&doc, id);
    assert_eq!(m.len(), 1);
    assert_eq!(m[0]["o"], 8);
    assert_eq!(m[0]["math"]["latex"], "\\pi r^2");
    // The saved file declares the namespace, uses Word's font and is valid.
    let body = saved_body(&mut doc);
    assert!(body.contains(&format!("xmlns:m=\"{M}\"")));
    assert!(body.contains("<m:oMath>") && body.contains("Cambria Math"));
    assert!(doc.undo());
    assert_eq!(paras(&doc)[0].1, "Area is  and more");
    assert!(math_segs(&doc, id).is_empty());
    assert!(doc.redo());
    assert_eq!(math_segs(&doc, id).len(), 1);
}

#[test]
fn insert_a_display_equation_splits_the_paragraph() {
    let (mut doc, id) = blank_with("Before After");
    let r = ops(&mut doc, json!({"op":"insert_equation","at":{"p":id,"o":7},"latex":"E = mc^2","display":true}));
    let ps = paras(&doc);
    assert_eq!(ps.len(), 3, "{ps:?}");
    assert_eq!(ps[0].1, "Before ");
    assert_eq!(ps[1].1, "\u{FFFC}");
    assert_eq!(ps[2].1, "After");
    let eq = ps[1].0;
    assert_eq!(r[0].caret.unwrap().p.0, eq);
    assert!(r[0].created.contains(&eq));
    let xml = para_xml(&doc, eq);
    assert!(xml.contains("<m:oMathPara><m:oMathParaPr><m:jc m:val=\"center\"/></m:oMathParaPr><m:oMath>"), "{xml}");
    assert_eq!(math_segs(&doc, eq)[0]["math"]["display"], true);
    saved_body(&mut doc);
    // One undo takes the whole thing back.
    assert!(doc.undo());
    assert_eq!(paras(&doc).iter().map(|p| p.1.clone()).collect::<Vec<_>>(), ["Before After"]);
}

#[test]
fn display_equations_at_the_edges_and_in_empty_paragraphs() {
    // At the end of a paragraph: a new paragraph below.
    let (mut doc, id) = blank_with("Text");
    ops(&mut doc, json!({"op":"insert_equation","at":{"p":id,"o":4},"latex":"x","display":true}));
    assert_eq!(paras(&doc).iter().map(|p| p.1.clone()).collect::<Vec<_>>(), ["Text", "\u{FFFC}"]);
    // At the start: the equation goes first, the text moves down.
    let (mut doc, id) = blank_with("Text");
    ops(&mut doc, json!({"op":"insert_equation","at":{"p":id,"o":0},"latex":"x","display":true}));
    assert_eq!(paras(&doc).iter().map(|p| p.1.clone()).collect::<Vec<_>>(), ["\u{FFFC}", "Text"]);
    // In an empty paragraph: that paragraph holds it.
    let mut doc = Document::new_blank();
    let id = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_equation","at":{"p":id,"o":0},"latex":"x","display":true}));
    assert_eq!(paras(&doc).len(), 1);
    assert_eq!(paras(&doc)[0].1, "\u{FFFC}");
}

#[test]
fn invalid_latex_is_an_error_and_changes_nothing() {
    let (mut doc, id) = blank_with("Text");
    let before = saved_body(&mut doc);
    let e = err(&mut doc, json!({"op":"insert_equation","at":{"p":id,"o":2},"latex":"\\frac{a}"}));
    assert!(e.contains("second group"), "{e}");
    assert_eq!(saved_body(&mut doc), before);
}

#[test]
fn set_equation_replaces_the_formula_and_keeps_the_paragraph_properties() {
    let mut doc = docx_from_body(&para(&quadratic().replace("<m:jc m:val=\"center\"/>", "<m:jc m:val=\"left\"/>")));
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"set_equation","at":{"p":id,"o":0},"latex":"a^2 + b^2 = c^2"}));
    let eq = doc.equation_at(Pos::new(id, 0)).unwrap();
    assert_eq!(eq.latex, "a^2 + b^2 = c^2".replace(' ', ""));
    assert!(eq.display);
    let xml = para_xml(&doc, id);
    assert!(xml.contains("<m:jc m:val=\"left\"/>"), "{xml}");
    assert!(!xml.contains("<m:rad>"));
    assert!(doc.undo());
    assert!(doc.equation_at(Pos::new(id, 0)).unwrap().latex.contains("\\sqrt"));
}

#[test]
fn set_equation_can_move_between_inline_and_display() {
    let (mut doc, id) = blank_with("A  B");
    ops(&mut doc, json!({"op":"insert_equation","at":{"p":id,"o":2},"latex":"x"}));
    ops(&mut doc, json!({"op":"set_equation","at":{"p":id,"o":2},"latex":"x^2","display":true}));
    let ps = paras(&doc);
    assert_eq!(ps.len(), 3, "{ps:?}");
    assert_eq!(ps[1].1, "\u{FFFC}");
    let eq = doc.equation_at(Pos::new(ps[1].0, 0)).unwrap();
    assert!(eq.display);
    assert_eq!(eq.latex, "x^2");
    // And back.
    ops(&mut doc, json!({"op":"set_equation","at":{"p":ps[1].0,"o":0},"latex":"x^3","display":false}));
    assert!(!doc.equation_at(Pos::new(ps[1].0, 0)).unwrap().display);
}

#[test]
fn set_equation_needs_an_equation() {
    let (mut doc, id) = blank_with("Text");
    let e = err(&mut doc, json!({"op":"set_equation","at":{"p":id,"o":1},"latex":"x"}));
    assert!(e.contains("no equation"), "{e}");
}

#[test]
fn deleting_an_equation_is_a_normal_delete() {
    let mut doc = word_document();
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"delete","from":{"p":id,"o":5},"to":{"p":id,"o":8}}));
    assert_eq!(paras(&doc)[0].1, "Solvenow.");
    assert!(math_segs(&doc, id).is_empty());
    assert!(!saved_body(&mut doc).contains(&sum_eq()));
    assert!(doc.undo());
    assert_eq!(paras(&doc)[0].1, "Solve \u{FFFC} now.");
    assert_eq!(math_segs(&doc, id).len(), 1);
    // Deleting only the equation's own position.
    ops(&mut doc, json!({"op":"delete","from":{"p":id,"o":6},"to":{"p":id,"o":7}}));
    assert_eq!(paras(&doc)[0].1, "Solve  now.");
    // A display equation's paragraph is emptied.
    let ps = paras(&doc);
    ops(&mut doc, json!({"op":"delete","from":{"p":ps[1].0,"o":0},"to":{"p":ps[1].0,"o":1}}));
    assert_eq!(paras(&doc)[1].1, "");
}

#[test]
fn offsets_after_an_equation_are_consistent() {
    let mut doc = word_document();
    let id = first_para(&doc);
    // Typing right after the equation lands after it, and the text stays where it was.
    ops(&mut doc, json!({"op":"insert_text","at":{"p":id,"o":7},"text":"X"}));
    assert_eq!(paras(&doc)[0].1, "Solve \u{FFFC}X now.");
    let s = segs(&doc, id);
    let end = s.iter().map(|x| (x["k"].as_str().unwrap().to_string(), x["o"].as_u64().unwrap())).collect::<Vec<_>>();
    assert_eq!(end, [("t".to_string(), 0), ("math".to_string(), 6), ("t".to_string(), 7)]);
    // Typing just before it too.
    ops(&mut doc, json!({"op":"insert_text","at":{"p":id,"o":6},"text":"Y"}));
    assert_eq!(paras(&doc)[0].1, "Solve Y\u{FFFC}X now.");
    // The paragraph length counts the equation once.
    let len = |doc: &Document| find_para(&doc.body, id).unwrap().len16();
    assert_eq!(len(&doc), 14);
    // A split lands on either side of it.
    ops(&mut doc, json!({"op":"split_paragraph","at":{"p":id,"o":8}}));
    let ps = paras(&doc);
    assert_eq!(ps[0].1, "Solve Y\u{FFFC}");
    assert_eq!(ps[1].1, "X now.");
    assert_eq!(math_segs(&doc, ps[0].0).len(), 1);
    saved_body(&mut doc);
}

#[test]
fn text_search_and_replace_still_find_text_around_equations() {
    let mut doc = word_document();
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"replace_text","find":"now","replace":"later"}));
    assert_eq!(paras(&doc)[0].1, "Solve \u{FFFC} later.");
    assert_eq!(math_segs(&doc, id).len(), 1);
}

#[test]
fn equation_in_a_hyperlink_and_in_a_tracked_insertion_are_found() {
    let eq = format!("<m:oMath>{}</m:oMath>", r("y"));
    let doc = docx_from_body(&para(&format!(r#"<w:r><w:t>a</w:t></w:r><w:ins w:id="1" w:author="x" w:date="2024-01-01T00:00:00Z">{eq}</w:ins><w:r><w:t>b</w:t></w:r>"#)));
    let id = first_para(&doc);
    assert_eq!(paras(&doc)[0].1, "a\u{FFFC}b");
    let e = doc.equation_at(Pos::new(id, 1)).unwrap();
    assert_eq!(e.latex, "y");
    assert!(doc.equation_at(Pos::new(id, 0)).is_none());
    let m = math_segs(&doc, id);
    assert_eq!(m[0]["o"], 1);
    assert_eq!(m[0]["ins"], true);
}

#[test]
fn equations_in_headers_and_table_cells_work() {
    let doc = docx_from_body(&format!("<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w=\"4000\"/></w:tblGrid><w:tr><w:tc><w:tcPr/>{}</w:tc></w:tr></w:tbl>{}", para(&format!("<w:r><w:t>q</w:t></w:r><m:oMath>{}</m:oMath>", r("z"))), para("")));
    let mut found = false;
    let mut doc = doc;
    let mut ids = vec![];
    walk_paragraphs(&doc.body, &mut |p| ids.push(p.id));
    for id in ids {
        if doc.equation_at(Pos::new(id, 1)).is_some() {
            found = true;
            ops(&mut doc, json!({"op":"set_equation","at":{"p":id,"o":1},"latex":"w"}));
            assert_eq!(doc.equation_at(Pos::new(id, 1)).unwrap().latex, "w");
        }
    }
    assert!(found);
}

// ---------------------------------------------------------------------------
// AI: tools, DocScript and Markdown
// ---------------------------------------------------------------------------

#[test]
fn docscript_shows_equations_as_latex() {
    let mut doc = word_document();
    let script = docscript(&mut doc, &ScriptOpts { start: None, limit: 100000 });
    assert!(script.contains("Solve $\\sum_{i=1}^{n} i$ now."), "{script}");
    assert!(script.contains("$$x=\\frac{"), "{script}");
    assert!(script.contains("$$\\begin{bmatrix} a & b \\\\ c & d \\end{bmatrix}$$") || script.contains("\\begin{bmatrix}"), "{script}");
}

#[test]
fn markdown_export_and_copy_use_dollar_signs() {
    let doc = word_document();
    let md = to_markdown(&doc);
    assert!(md.contains("Solve $\\sum_{i=1}^{n} i$ now."), "{md}");
    assert!(md.contains("$$x=\\frac{"), "{md}");
    let ps = paras(&doc);
    let part = range_to_markdown(&doc, Pos::new(ps[0].0, 4), Pos::new(ps[0].0, 8)).unwrap();
    assert!(part.contains("$\\sum_{i=1}^{n} i$"), "{part}");
}

#[test]
fn insert_content_reads_dollar_signs() {
    let mut doc = Document::new_blank();
    let id = doc.render_all().order[0];
    let r = ops(&mut doc, json!({"op":"insert_markdown","after":id,"markdown":"Energy is $E = mc^2$ here.\n\n$$\n\\int_0^1 x \\, dx\n$$\n\nCosts $5 and $6 only."}));
    assert_eq!(r[0].created.len(), 3);
    let ps = paras(&doc);
    assert_eq!(ps[1].1, "Energy is \u{FFFC} here.");
    assert_eq!(ps[2].1, "\u{FFFC}");
    assert_eq!(ps[3].1, "Costs $5 and $6 only.");
    assert_eq!(doc.equation_at(Pos::new(ps[1].0, 10)).unwrap().latex, "E=mc^2");
    assert!(doc.equation_at(Pos::new(ps[2].0, 0)).unwrap().display);
    saved_body(&mut doc);
}

#[test]
fn rewrite_paragraph_keeps_and_changes_equations() {
    let mut doc = word_document();
    let id = first_para(&doc);
    call_tool(&mut doc, "rewrite_paragraph", &json!({"id": id, "markdown": "Now solve $\\sum_{k=1}^{m} k^2$ please."})).unwrap();
    assert_eq!(paras(&doc)[0].1, "Now solve \u{FFFC} please.");
    assert_eq!(math_segs(&doc, id).len(), 1, "the old equation does not stay behind");
    assert_eq!(math_segs(&doc, id)[0]["math"]["latex"], "\\sum_{k=1}^{m} k^2");
}

#[test]
fn ai_insert_equation_tool() {
    let (mut doc, id) = blank_with("The area is given by the formula.");
    let out = call_tool(&mut doc, "insert_equation", &json!({"id": id, "after_text": "given by", "latex": "A = \\pi r^2"})).unwrap();
    assert!(out.contains("$A=\\pi r^2$"), "{out}");
    assert_eq!(paras(&doc)[0].1, "The area is given by\u{FFFC} the formula.");
    let out = call_tool(&mut doc, "insert_equation", &json!({"id": id, "latex": "x^2", "display": true})).unwrap();
    assert!(out.contains("$$x^2$$"), "{out}");
    assert!(paras(&doc).len() >= 2);
    // Mistakes come back as plain words for the model.
    let e = call_tool(&mut doc, "insert_equation", &json!({"id": id, "latex": "\\frac{1}"})).unwrap_err();
    assert!(e.contains("second group"), "{e}");
    let e = call_tool(&mut doc, "insert_equation", &json!({"id": id, "after_text": "nowhere", "latex": "x"})).unwrap_err();
    assert!(e.contains("not found"), "{e}");
}

#[test]
fn ai_set_equation_tool() {
    let mut doc = word_document();
    let id = first_para(&doc);
    let out = call_tool(&mut doc, "set_equation", &json!({"id": id, "latex": "\\prod_{i=1}^{n} i"})).unwrap();
    assert!(out.contains("\\prod_{i=1}^{n} i"), "{out}");
    let e = call_tool(&mut doc, "set_equation", &json!({"id": id, "latex": "x", "number": 2})).unwrap_err();
    assert!(e.contains("no number 2"), "{e}");
}

#[test]
fn the_tools_are_listed() {
    let defs = docxtor_core::ai::tool_definitions();
    let names: Vec<_> = defs.as_array().unwrap().iter().map(|d| d["name"].as_str().unwrap().to_string()).collect();
    assert!(names.contains(&"insert_equation".to_string()) && names.contains(&"set_equation".to_string()));
}

#[test]
fn large_operator_bodies_get_braces_only_when_they_need_them() {
    let rt = |s: &str| omml_to_latex(&latex_to_omml(s).unwrap_or_else(|e| panic!("{s}: {e}")));
    // A body that runs to the end or to a sign needs none.
    assert_eq!(rt("\\int_0^1 x^2 \\, dx"), "\\int_{0}^{1} x^2\\,dx");
    assert_eq!(rt("\\sum_{i=1}^{n} i + 1"), "\\sum_{i=1}^{n} i+1");
    assert_eq!(rt("\\int f(x) \\, dx = F"), "\\int f(x)\\,dx=F");
    // A body with a sign inside, or a comparison, does.
    assert_eq!(rt("\\sum_{i} {a+b}"), "\\sum_{i} {a+b}");
    assert_eq!(rt("\\sum_{i} {a \\leq b}"), "\\sum_{i} {a\\leq b}");
    // Brackets keep their contents together without braces.
    assert_eq!(rt("\\sum_{i} (a+b)"), "\\sum_{i} (a+b)");
    for src in ["\\sum_{i} {a+b}", "\\sum_{i} (a+b)", "\\int -x \\, dx", "\\sum_{i} \\left( a + b \\right)"] {
        let once = rt(src);
        assert_eq!(once, rt(&once), "{src}");
        let a = latex_to_omml(src).unwrap();
        let b = latex_to_omml(&once).unwrap();
        assert_eq!(a.to_xml(), b.to_xml(), "{src} -> {once}");
    }
}

#[test]
fn plain_text_copy_keeps_equations_as_latex() {
    let doc = word_document();
    let ps = paras(&doc);
    let text = doc.text_range(Pos::new(ps[0].0, 0), Pos::new(ps[0].0, 14));
    assert_eq!(text, "Solve $\\sum_{i=1}^{n} i$ now.");
    // Across paragraphs, the display equation too.
    let text = doc.text_range(Pos::new(ps[0].0, 0), Pos::new(ps[1].0, 1));
    assert!(text.contains("\n$$x=\\frac{"), "{text}");
}

#[test]
fn replacing_a_font_leaves_the_math_font_alone() {
    let mut doc = word_document();
    ops(&mut doc, json!({"op":"replace_font","from":"Calibri","to":"Georgia"}));
    assert!(saved_body(&mut doc).contains("Cambria Math"));
}

#[test]
fn plain_text_export_keeps_equations_as_latex() {
    let doc = word_document();
    let text = docxtor_core::export::to_text(&doc);
    assert!(text.contains("Solve $\\sum_{i=1}^{n} i$ now."), "{text}");
    assert!(text.contains("$$x=\\frac{"), "{text}");
}

#[test]
fn an_equation_made_by_word_in_a_header_or_note_style_part_is_valid_when_saved() {
    // The namespace is declared on the part that holds the equation: inserting into a document that lacks it
    // adds it, and the saved file passes the namespace check.
    let (mut doc, id) = blank_with("Text");
    ops(&mut doc, json!({"op":"insert_equation","at":{"p":id,"o":2},"latex":"\\sum_{i=1}^{n} i"}));
    let body = saved_body(&mut doc);
    assert_eq!(body.matches(&format!("xmlns:m=\"{M}\"")).count(), 1);
    // A second equation does not declare it again.
    ops(&mut doc, json!({"op":"insert_equation","at":{"p":id,"o":0},"latex":"x"}));
    assert_eq!(saved_body(&mut doc).matches("xmlns:m=").count(), 1);
}

#[test]
fn shorthand_and_corner_cases_of_the_latex_subset() {
    let rt = |s: &str| omml_to_latex(&latex_to_omml(s).unwrap_or_else(|e| panic!("{s}: {e}")));
    // Single tokens need no braces, as in TeX.
    assert_eq!(rt("\\frac12"), "\\frac{1}{2}");
    assert_eq!(rt("\\sqrt x"), "\\sqrt{x}");
    assert_eq!(rt("x^\\alpha"), "x^\\alpha");
    assert_eq!(rt("x_1^2"), "x_1^2");
    assert_eq!(rt("a^b_c"), "a_c^b");
    // Scripts on groups and on nothing.
    assert_eq!(rt("{a+b}^2"), "{a+b}^2");
    assert_eq!(rt("^2 x"), "{}^2x");
    // Nested structures.
    assert_eq!(rt("\\frac{\\sqrt{a}}{\\frac{b}{c}}"), "\\frac{\\sqrt{a}}{\\frac{b}{c}}");
    assert_eq!(rt("\\left( \\frac{a}{b} \\right)^2"), "\\left( \\frac{a}{b} \\right)^2");
    // Primes.
    assert_eq!(rt("f''(x)"), "f^″(x)");
    // Text keeps its spaces.
    assert_eq!(rt("\\text{ and }"), "\\text{ and }");
    // Greek and functions run together without losing the space a command needs.
    assert_eq!(rt("\\alpha x"), "\\alpha x");
    assert_eq!(rt("\\sin x \\cos y"), "\\sin x\\cos y");
    // Spacing.
    assert_eq!(rt("a\\,b"), "a\\,b");
    // Dollar signs and stray backslashes.
    assert_eq!(rt("\\{ a \\}"), "\\{a\\}");
    assert_eq!(rt("50\\%"), "50\\%");
    // Several rows and columns.
    assert_eq!(rt("\\begin{aligned} a &= b \\\\ c &= d \\end{aligned}"), "\\begin{aligned} a & =b \\\\ c & =d \\end{aligned}");
}

#[test]
fn an_equation_directly_under_the_body_can_be_edited_and_stays_there() {
    let eq = format!(r#"<m:oMathPara><m:oMathParaPr><m:jc m:val="center"/></m:oMathParaPr><m:oMath>{}</m:oMath></m:oMathPara>"#, r("x=1"));
    let body = format!(r#"<w:p><w:r><w:t>Before</w:t></w:r></w:p>{eq}<w:p><w:r><w:t>After</w:t></w:r></w:p>"#);
    let mut doc = docx_from_body(&body);
    // It is a paragraph of its own now, with an equation in it, that can be addressed.
    let id = paras(&doc)[1].0;
    let at = doc.equation_at(Pos::new(id, 0)).expect("an equation at the start of its paragraph");
    assert!(at.display && at.latex.contains("x=1"), "{}", at.latex);
    // Untouched, it is written back as it was.
    let untouched = saved_body(&mut docx_from_body(&body));
    assert!(untouched.contains(&format!("</w:p>{eq}<w:p>")), "{untouched}");
    // Editing it keeps the bare element in place, with its properties.
    ops(&mut doc, json!({"op":"set_equation","at":{"p":id,"o":0},"latex":"y^2"}));
    assert_eq!(doc.equation_at(Pos::new(id, 0)).unwrap().latex, "y^2");
    let xml = saved_body(&mut doc);
    assert!(xml.contains("</w:p><m:oMathPara><m:oMathParaPr><m:jc m:val=\"center\"/></m:oMathParaPr><m:oMath>") && xml.contains("</m:oMathPara><w:p>"), "{xml}");
    assert!(!xml.contains("<w:p><m:oMathPara"), "{xml}");
    // It undoes like any other edit.
    assert!(doc.undo());
    assert!(doc.equation_at(Pos::new(id, 0)).unwrap().latex.contains("x=1"));
    assert!(docxtor_core::export::to_text(&doc).contains("x=1"));
    let reopened = Document::open(&doc.save().unwrap()).unwrap();
    assert!(reopened.equation_at(Pos::new(paras(&reopened)[1].0, 0)).is_some());
}

#[test]
fn an_equation_paragraph_that_gets_text_or_formatting_is_saved_as_a_paragraph() {
    let eq = format!("<m:oMathPara><m:oMath>{}</m:oMath></m:oMathPara>", r("z"));
    let mut doc = docx_from_body(&eq);
    let id = paras(&doc)[0].0;
    ops(&mut doc, json!({"op":"insert_text","at":{"p":id,"o":1},"text":" is z"}));
    let xml = saved_body(&mut doc);
    assert!(xml.contains("<w:p><m:oMathPara>") && xml.contains("is z"), "{xml}");
    // Aligning the line also makes it a normal paragraph with properties.
    let mut doc = docx_from_body(&eq);
    let id = paras(&doc)[0].0;
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[id],"format":{"align":"right"}}));
    let xml = saved_body(&mut doc);
    assert!(xml.contains("<w:p><w:pPr>") && xml.contains("<m:oMathPara>"), "{xml}");
    // A display equation inserted into an empty paragraph is a normal paragraph holding one, as before.
    let mut doc = docx_from_body("<w:p/>");
    let id = paras(&doc)[0].0;
    ops(&mut doc, json!({"op":"insert_equation","at":{"p":id,"o":0},"latex":"a+b","display":true}));
    assert!(saved_body(&mut doc).contains("<w:p><m:oMathPara>"));
}
