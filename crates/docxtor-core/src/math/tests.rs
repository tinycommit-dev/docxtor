use super::*;

fn rt(src: &str) -> String {
    omml_to_latex(&latex_to_omml(src).unwrap_or_else(|e| panic!("{src}: {e}")))
}

#[test]
fn round_trips_are_stable() {
    for src in [
        "x^2",
        "x_i^2",
        "\\frac{a+b}{c}",
        "\\sqrt{x}",
        "\\sqrt[3]{x+1}",
        "\\sum_{i=1}^{n} i^2",
        "\\int_0^1 x^2 \\, dx",
        "\\lim_{x \\to 0} \\frac{\\sin x}{x}",
        "\\left( \\frac{a}{b} \\right)",
        "\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}",
        "f(x) = \\begin{cases} 1 & x > 0 \\\\ 0 & x \\leq 0 \\end{cases}",
        "\\alpha + \\beta \\cdot \\gamma",
        "\\hat{x} + \\vec{v} + \\bar{y}",
        "\\text{if } x > 0",
        "\\mathbf{v} \\times \\mathbf{w}",
        "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}",
        "\\binom{n}{k}",
        "\\sin^2 x + \\cos^2 x = 1",
        "e^{i\\pi} + 1 = 0",
        "\\prod_{k=1}^{n} k",
        "\\oint f \\, dz",
        "\\overline{AB}",
        "\\underbrace{a+b}_{n}",
    ] {
        let once = rt(src);
        let twice = rt(&once);
        assert_eq!(once, twice, "not stable: {src} -> {once} -> {twice}");
    }
}

#[test]
fn shows_latex_back() {
    assert_eq!(rt("\\frac{a}{b}"), "\\frac{a}{b}");
    assert_eq!(rt("x^2"), "x^2");
    assert_eq!(rt("\\sum_{i=1}^{n} i^2"), "\\sum_{i=1}^{n} i^2");
    assert_eq!(rt("\\sin x"), "\\sin x");
}

#[test]
fn mathml_of_a_fraction() {
    let m = omml_to_mathml(&latex_to_omml("\\frac{a}{b}").unwrap());
    assert!(m.contains("<mfrac>"), "{m}");
    assert!(m.starts_with("<math"), "{m}");
}

fn omml_with_fraction(kind: &str) -> Element {
    let src = format!(
        r#"<m:oMath xmlns:m="{NS_M}"><m:f><m:fPr><m:type m:val="{kind}"/></m:fPr><m:num><m:r><m:t>a</m:t></m:r></m:num><m:den><m:r><m:t>b</m:t></m:r></m:den></m:f></m:oMath>"#
    );
    crate::xml::parse(src.as_bytes()).unwrap().root
}

fn fraction_type(omath: &Element) -> Option<String> {
    omath.child("m:f").and_then(|f| f.child("m:fPr")).and_then(|p| p.child("m:type")).and_then(|t| t.attr("m:val")).map(String::from)
}

#[test]
fn fraction_kinds_survive_a_trip_through_latex() {
    for (kind, latex) in [("lin", "\\nicefrac{a}{b}"), ("skw", "\\sfrac{a}{b}"), ("noBar", "\\genfrac{}{}{0pt}{}{a}{b}")] {
        let omml = omml_with_fraction(kind);
        assert_eq!(omml_to_latex(&omml), latex, "{kind}");
        let back = latex_to_omml(latex).unwrap();
        assert_eq!(fraction_type(&back).as_deref(), Some(kind), "{latex}");
        assert_eq!(omml_to_latex(&back), latex);
    }
    // A fraction with a bar has no type, as before.
    assert_eq!(fraction_type(&latex_to_omml("\\frac{a}{b}").unwrap()), None);
    // In a script, the fraction is one atom.
    for src in ["\\nicefrac{1}{2}^2", "\\sfrac{x}{y} + \\genfrac{}{}{0pt}{}{n}{k}"] {
        let once = rt(src);
        assert_eq!(once, rt(&once), "{src}");
    }
}

#[test]
fn genfrac_reads_thickness_and_delimiters() {
    assert_eq!(fraction_type(&latex_to_omml("\\genfrac{}{}{0pt}{}{n}{k}").unwrap()).as_deref(), Some("noBar"));
    assert_eq!(fraction_type(&latex_to_omml("\\genfrac{}{}{0.4pt}{}{n}{k}").unwrap()), None);
    // Delimiters make it a binomial.
    assert_eq!(rt("\\genfrac(){0pt}{}{n}{k}"), "\\binom{n}{k}");
    assert_eq!(rt("\\binom{n}{k}"), "\\binom{n}{k}");
    for src in ["\\genfrac{}{}{0pt}{}{a}", "\\genfrac{}{}{0pt}{a}", "\\nicefrac{a}", "\\sfrac"] {
        assert!(latex_to_omml(src).is_err(), "{src}");
    }
}

#[test]
fn every_fraction_kind_draws() {
    let lin = omml_to_mathml(&latex_to_omml("\\nicefrac{a}{b}").unwrap());
    assert!(lin.contains("<mo>/</mo>") && !lin.contains("<mfrac"), "{lin}");
    let skw = omml_to_mathml(&latex_to_omml("\\sfrac{a}{b}").unwrap());
    assert!(skw.contains("<mo>/</mo>"), "{skw}");
    let no_bar = omml_to_mathml(&latex_to_omml("\\genfrac{}{}{0pt}{}{a}{b}").unwrap());
    assert!(no_bar.contains("linethickness=\"0\""), "{no_bar}");
}

#[test]
fn errors_point_at_the_problem() {
    for (src, needle) in [("\\frac{a}", "second group"), ("\\foo", "Unknown command \\foo at character 1"), ("{x", "never closed"), ("x}", "}"), ("\\left( x", "\\right"), ("", "empty"), ("x^", "needs something")] {
        let e = latex_to_omml(src).unwrap_err();
        assert!(e.contains(needle), "{src}: {e}");
    }
}
