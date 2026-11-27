//! A colour in a .docx is a string the file chooses. Whatever the engine hands the page (the render, the shape, the chart, the picture,
//! the page set-up, the theme, the styles, the lists, the exports) must hold only colours: six hex digits, `auto`, or a name from a
//! closed list, never `FF0000;background:url(...)` or `FF0000" onload="...`.
//!
//! One document is built twice, with a colour in every place the engine reads one: once with a different valid colour in each place
//! (every place must show up in what the engine hands out, so a place the engine stopped reading cannot hide a hole), and once with a
//! hostile value in each place (nothing of it may show). The file's own text is untouched: saving the hostile one keeps every value.

use docxtor_core::export::{epub::to_epub, html::to_html};
use docxtor_core::{package::Package, Document, Pos};
use serde::Serialize;
use std::io::Read;

/// The two shapes of value a hostile file uses: one that adds a CSS declaration, one that leaves an attribute and adds a handler.
const BAD_CSS: &str = "FF0000;background:url(http://evil.test/x)";
const BAD_ATTR: &str = "FF0000\" onload=\"window.__pwned=1";
/// Six characters that are not hex digits and carry no marker: a check that counts characters lets it through.
const BAD_SHORT: &str = "a;b:c)";
/// Six bytes, four characters, none of them hex digits: slicing it every two bytes (as a colour reader that checks only the length does) splits a character.
const BAD_UNICODE: &str = "a\u{e9}\u{e9}a";
/// What must not appear in anything handed out.
const MARKERS: [&str; 4] = ["evil.test", "onload", "url(", "__pwned"];

const NS: &str = r##"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word" mc:Ignorable="w14 w15""##;
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// A 1 x 1 PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00,
    0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42,
    0x60, 0x82,
];

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Hands out the value for each place a colour is written: the hostile one, or a valid one nothing else uses.
struct Places {
    hostile: bool,
    n: usize,
    /// (place, the valid colour written there) for the valid build.
    seen: Vec<(String, String)>,
}

impl Places {
    fn new(hostile: bool) -> Places {
        Places { hostile, n: 0, seen: Vec::new() }
    }

    fn bad(&self) -> &'static str {
        [BAD_CSS, BAD_ATTR, BAD_SHORT, BAD_UNICODE][self.n % 4]
    }

    /// A colour for an attribute value (already escaped for XML).
    fn c(&mut self, place: &str) -> String {
        self.n += 1;
        let valid = format!("{:02X}{:02X}{:02X}", 0x10 + 2 * self.n % 0xE0, 0x20 + self.n % 0xD0, 0xF0 - self.n % 0xC0);
        self.seen.push((place.to_string(), valid.clone()));
        if self.hostile {
            esc(self.bad())
        } else {
            valid
        }
    }

    /// A place whose valid value cannot be told from another by its text (a name from a closed list, a two-digit tint): it is not listed as a place to find.
    fn quiet(&mut self, valid: &str) -> String {
        self.n += 1;
        if self.hostile {
            esc(self.bad())
        } else {
            valid.to_string()
        }
    }
}

fn solid(p: &mut Places, place: &str) -> String {
    format!(r##"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"##, p.c(place))
}

fn run(rpr: &str, text: &str) -> String {
    format!(r##"<w:r><w:rPr>{rpr}</w:rPr><w:t xml:space="preserve">{text}</w:t></w:r>"##)
}

fn inline_shape(id: u32, name: &str, sppr: &str, style: &str, body: &str, body_pr: &str) -> String {
    format!(
        r##"<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="2400000" cy="1200000"/><wp:docPr id="{id}" name="{name}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2400000" cy="1200000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{sppr}</wps:spPr>{style}{body}<wps:bodyPr>{body_pr}</wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"##
    )
}

fn txbx(rpr: &str, text: &str) -> String {
    format!(r##"<wps:txbx><w:txbxContent><w:p>{}</w:p></w:txbxContent></wps:txbx>"##, run(rpr, text))
}

/// The body of the document, with a colour in every place the engine reads one.
fn body(p: &mut Places, chart_rid: &str, pic_rid: &str) -> String {
    let mut b = String::new();
    // 1. a paragraph with shading, borders and a mark colour; a run with colour, highlight, shading, underline colour, a box and a text effect
    let (shd_c, shd_f) = (p.c("para shd colour"), p.c("para shd fill"));
    let borders = ["top", "left", "bottom", "right", "between", "bar"].map(|s| format!(r##"<w:{s} w:val="single" w:sz="4" w:space="1" w:color="{}"/>"##, p.c(&format!("para border {s}")))).concat();
    let mark = p.c("paragraph mark colour");
    let ppr = format!(r##"<w:pPr><w:pBdr>{borders}</w:pBdr><w:shd w:val="pct25" w:color="{shd_c}" w:fill="{shd_f}"/><w:rPr><w:color w:val="{mark}"/></w:rPr></w:pPr>"##);
    let rpr = format!(
        r##"<w:color w:val="{}"/><w:highlight w:val="{}"/><w:u w:val="single" w:color="{}"/><w:bdr w:val="single" w:sz="4" w:space="0" w:color="{}"/><w:shd w:val="pct25" w:color="{}" w:fill="{}"/>"##,
        p.c("run colour"),
        p.quiet("darkMagenta"),
        p.c("underline colour"),
        p.c("text border"),
        p.c("run shd colour"),
        p.c("run shd fill")
    );
    b += &format!("<w:p>{ppr}{}</w:p>", run(&rpr, "Plain colours"));
    // 2. theme colour references: names the theme does not have (the fallback colour shows), then names it has with tints and shades that are not hex
    let theme_rpr = format!(
        r##"<w:color w:val="{}" w:themeColor="{}" w:themeTint="{}" w:themeShade="{}"/><w:shd w:val="clear" w:color="auto" w:fill="{}" w:themeFill="{}" w:themeFillTint="{}" w:themeFillShade="{}"/><w:u w:val="single" w:color="{}" w:themeColor="{}" w:themeTint="{}"/>"##,
        p.c("theme run colour fallback"),
        p.quiet("nonesuch"),
        p.quiet("80"),
        p.quiet("80"),
        p.c("theme shading fallback"),
        p.quiet("nonesuch"),
        p.quiet("80"),
        p.quiet("80"),
        p.c("theme underline fallback"),
        p.quiet("nonesuch"),
        p.quiet("80")
    );
    b += &format!("<w:p>{}</w:p>", run(&theme_rpr, "Theme colours"));
    let theme_rpr = format!(
        r##"<w:color w:val="{}" w:themeColor="accent1" w:themeTint="{}" w:themeShade="{}"/><w:shd w:val="clear" w:color="auto" w:fill="{}" w:themeFill="accent2" w:themeFillTint="{}" w:themeFillShade="{}"/><w:u w:val="single" w:color="{}" w:themeColor="accent3" w:themeTint="{}" w:themeShade="{}"/>"##,
        p.c("theme tinted run colour fallback"),
        p.quiet("80"),
        p.quiet("80"),
        p.c("theme tinted shading fallback"),
        p.quiet("80"),
        p.quiet("80"),
        p.c("theme tinted underline fallback"),
        p.quiet("80"),
        p.quiet("80")
    );
    b += &format!("<w:p>{}</w:p>", run(&theme_rpr, "Tinted theme colours"));
    // 3. Word 2010 text effects: glow, shadow, outline, a gradient fill, an outline
    let fx = format!(
        r##"<w14:glow w14:rad="63500"><w14:srgbClr w14:val="{}"/></w14:glow><w14:shadow w14:blurRad="50800" w14:dist="38100" w14:dir="2700000" w14:sx="100000" w14:sy="100000" w14:kx="0" w14:ky="0" w14:algn="tl"><w14:srgbClr w14:val="{}"/></w14:shadow><w14:textOutline w14:w="9525"><w14:solidFill><w14:srgbClr w14:val="{}"/></w14:solidFill></w14:textOutline><w14:textFill><w14:gradFill><w14:gsLst><w14:gs w14:pos="0"><w14:srgbClr w14:val="{}"/></w14:gs><w14:gs w14:pos="100000"><w14:srgbClr w14:val="{}"/></w14:gs></w14:gsLst><w14:lin w14:ang="5400000" w14:scaled="0"/></w14:gradFill></w14:textFill>"##,
        p.c("text glow"),
        p.c("text shadow"),
        p.c("text outline"),
        p.c("text gradient stop 1"),
        p.c("text gradient stop 2")
    );
    b += &format!("<w:p>{}</w:p>", run(&fx, "Text effects"));
    let fx2 = format!(r##"<w14:textFill><w14:solidFill><w14:srgbClr w14:val="{}"/></w14:solidFill></w14:textFill>"##, p.c("text solid fill"));
    b += &format!("<w:p>{}</w:p>", run(&fx2, "Solid text fill"));
    // 4. a table: borders, cell borders, diagonals, table shading, cell shading with a pattern
    let tbl_borders = ["top", "left", "bottom", "right", "insideH", "insideV"].map(|s| format!(r##"<w:{s} w:val="single" w:sz="4" w:space="0" w:color="{}"/>"##, p.c(&format!("table border {s}")))).concat();
    let cell_borders = ["top", "left", "bottom", "right", "tl2br", "tr2bl"].map(|s| format!(r##"<w:{s} w:val="single" w:sz="4" w:space="0" w:color="{}"/>"##, p.c(&format!("cell border {s}")))).concat();
    b += &format!(
        r##"<w:tbl><w:tblPr><w:tblBorders>{tbl_borders}</w:tblBorders><w:shd w:val="clear" w:color="auto" w:fill="{}"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/><w:tcBorders>{cell_borders}</w:tcBorders><w:shd w:val="pct25" w:color="{}" w:fill="{}"/></w:tcPr><w:p><w:r><w:t>Cell one</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="{}" w:themeFill="{}"/></w:tcPr><w:p><w:r><w:t>Cell two</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"##,
        p.c("table shading"),
        p.c("cell shd colour"),
        p.c("cell shd fill"),
        p.c("cell themed fill fallback"),
        p.quiet("accent4")
    );
    // 5. a horizontal rule (VML) and a content control with a colour
    b += &format!(r##"<w:p><w:r><w:pict><v:rect style="width:0;height:1.5pt" o:hr="t" o:hrstd="t" o:hralign="center" fillcolor="#{}" stroked="f"/></w:pict></w:r></w:p>"##, p.c("hrule fillcolor"));
    b += &format!(r##"<w:sdt><w:sdtPr><w:alias w:val="coloured"/><w:id w:val="4244"/><w15:color w:val="{}"/><w:text/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Coloured control</w:t></w:r></w:p></w:sdtContent></w:sdt>"##, p.c("control colour"));
    b += &format!(r##"<w:p><w:sdt><w:sdtPr><w:alias w:val="inline"/><w:id w:val="4245"/><w15:color w:val="{}"/><w:text/></w:sdtPr><w:sdtContent><w:r><w:t>inline control</w:t></w:r></w:sdtContent></w:sdt></w:p>"##, p.c("inline control colour"));
    // 6. shapes: solid fill, line, shadow, inner shadow, glow, a preset colour, a system colour, a scheme colour, a gradient, a pattern
    let eff = format!(
        r##"<a:effectLst><a:outerShdw blurRad="50800" dist="38100" dir="2700000"><a:srgbClr val="{}"/></a:outerShdw><a:innerShdw blurRad="50800" dist="38100" dir="2700000"><a:srgbClr val="{}"/></a:innerShdw><a:glow rad="63500"><a:srgbClr val="{}"/></a:glow></a:effectLst>"##,
        p.c("shape shadow"),
        p.c("shape inner shadow"),
        p.c("shape glow")
    );
    let sppr = format!(r##"{}<a:ln w="12700">{}</a:ln>{eff}"##, solid(p, "shape fill"), solid(p, "shape line"));
    b += &inline_shape(11, "Shape one", &sppr, "", &txbx(&format!(r##"<w:color w:val="{}"/>"##, p.c("shape text colour")), "in a shape"), "");
    let grad = format!(
        r##"<a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="{}"/></a:gs><a:gs pos="50000"><a:schemeClr val="{}"/></a:gs><a:gs pos="100000"><a:sysClr val="windowText" lastClr="{}"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill><a:ln w="12700"><a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="{}"/></a:gs><a:gs pos="100000"><a:srgbClr val="{}"/></a:gs></a:gsLst></a:gradFill></a:ln>"##,
        p.c("gradient stop 1"),
        p.quiet("accent1"),
        p.c("gradient system colour"),
        p.c("line gradient stop 1"),
        p.c("line gradient stop 2")
    );
    b += &inline_shape(12, "Shape two", &grad, "", &txbx("", "gradient"), "");
    let patt = format!(
        r##"<a:pattFill prst="pct50"><a:fgClr><a:srgbClr val="{}"/></a:fgClr><a:bgClr><a:srgbClr val="{}"/></a:bgClr></a:pattFill><a:ln w="12700"><a:solidFill><a:prstClr val="{}"/></a:solidFill></a:ln>"##,
        p.c("pattern foreground"),
        p.c("pattern background"),
        p.quiet("red")
    );
    b += &inline_shape(13, "Shape three", &patt, "", &txbx("", "pattern"), "");
    let style = format!(
        r##"<wps:style><a:lnRef idx="2"><a:schemeClr val="{}"/></a:lnRef><a:fillRef idx="1"><a:schemeClr val="{}"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="{}"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="{}"/></a:fontRef></wps:style>"##,
        p.quiet("accent1"),
        p.quiet("accent2"),
        p.quiet("accent3"),
        p.quiet("lt1")
    );
    b += &inline_shape(14, "Shape four", "", &style, &txbx("", "themed"), "");
    // 7. WordArt: a warped text with a gradient fill, an outline, a glow and a shadow
    let wa = format!(
        r##"<w14:glow w14:rad="63500"><w14:srgbClr w14:val="{}"/></w14:glow><w14:shadow w14:blurRad="0" w14:dist="38100" w14:dir="2700000" w14:sx="100000" w14:sy="100000" w14:kx="0" w14:ky="0" w14:algn="tl"><w14:srgbClr w14:val="{}"/></w14:shadow><w14:textOutline w14:w="9525"><w14:solidFill><w14:srgbClr w14:val="{}"/></w14:solidFill></w14:textOutline><w14:textFill><w14:gradFill><w14:gsLst><w14:gs w14:pos="0"><w14:srgbClr w14:val="{}"/></w14:gs><w14:gs w14:pos="100000"><w14:srgbClr w14:val="{}"/></w14:gs></w14:gsLst><w14:lin w14:ang="5400000" w14:scaled="0"/></w14:gradFill></w14:textFill>"##,
        p.c("wordart glow"),
        p.c("wordart shadow"),
        p.c("wordart outline"),
        p.c("wordart gradient 1"),
        p.c("wordart gradient 2")
    );
    b += &inline_shape(15, "WordArt", "", "", &txbx(&format!(r##"<w:color w:val="{}"/>{wa}"##, p.c("wordart colour")), "WordArt"), r##"<a:prstTxWarp prst="textArchUp"><a:avLst/></a:prstTxWarp>"##);
    // 8. a freehand stroke (what the pen writes): a curve with a coloured line
    let ink = format!(
        r##"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="0" t="0" r="r" b="b"/><a:pathLst><a:path w="100" h="100" fill="none"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="100" y="100"/></a:lnTo></a:path></a:pathLst></a:custGeom><a:noFill/><a:ln w="19050" cap="rnd"><a:solidFill><a:srgbClr val="{}"><a:alpha val="50000"/></a:srgbClr></a:solidFill></a:ln>"##,
        p.c("ink colour")
    );
    b += &inline_shape(16, "Ink", &ink, "", "", "");
    // 9. VML: fill, stroke, shadow, gradient colours
    b += &format!(
        r##"<w:p><w:r><w:pict><v:rect id="vml1" style="width:100pt;height:50pt" fillcolor="#{}" strokecolor="#{}"><v:fill type="gradient" color2="#{}" colors="0 #{};1 #{}"/><v:stroke color="#{}"/><v:shadow on="t" color="#{}"/></v:rect></w:pict></w:r></w:p>"##,
        p.c("vml fillcolor"),
        p.c("vml strokecolor"),
        p.c("vml fill color2"),
        p.c("vml fill colors 1"),
        p.c("vml fill colors 2"),
        p.c("vml stroke colour"),
        p.c("vml shadow colour")
    );
    b += &format!(
        r##"<w:p><w:r><w:pict><v:rect id="vml2" style="width:100pt;height:50pt" fillcolor="#{}" strokecolor="#{}"/></w:pict></w:r></w:p><w:p><w:r><w:pict><v:rect id="vml3" style="width:100pt;height:50pt"><v:fill type="gradient" color="#{}" color2="#{}"/></v:rect></w:pict></w:r></w:p>"##,
        p.c("vml plain fillcolor"),
        p.c("vml plain strokecolor"),
        p.c("vml gradient color"),
        p.c("vml gradient color2")
    );
    // 10. a chart
    b += &format!(
        r##"<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="4572000" cy="2743200"/><wp:docPr id="21" name="Chart"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:r="{REL_NS}" r:id="{chart_rid}"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"##
    );
    // 11. a picture with an outline, effects, a recolour and a colour change
    b += &format!(
        r##"<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="914400" cy="914400"/><wp:docPr id="22" name="Picture"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="0" name="p.png"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip xmlns:r="{REL_NS}" r:embed="{pic_rid}"><a:duotone><a:srgbClr val="{}"/><a:srgbClr val="{}"/></a:duotone><a:clrChange><a:clrFrom><a:srgbClr val="{}"/></a:clrFrom><a:clrTo><a:srgbClr val="{}"><a:alpha val="0"/></a:srgbClr></a:clrTo></a:clrChange></a:blip><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="914400"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:ln w="12700"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:ln><a:effectLst><a:outerShdw blurRad="50800" dist="38100" dir="2700000"><a:srgbClr val="{}"/></a:outerShdw><a:glow rad="63500"><a:srgbClr val="{}"/></a:glow></a:effectLst></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"##,
        p.c("picture duotone 1"),
        p.c("picture duotone 2"),
        p.c("picture colour change from"),
        p.c("picture colour change to"),
        p.c("picture outline"),
        p.c("picture shadow"),
        p.c("picture glow")
    );
    // 12. a list paragraph (its numbering is in numbering.xml) and a paragraph in a hostile style
    b += r##"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="7"/></w:numPr></w:pPr><w:r><w:t>List item</w:t></w:r></w:p>"##;
    b += r##"<w:p><w:pPr><w:pStyle w:val="Hostile"/></w:pPr><w:r><w:t>Styled</w:t></w:r></w:p>"##;
    b += r##"<w:p><w:r><w:rPr><w:rStyle w:val="HostileChar"/></w:rPr><w:t>Character styled</w:t></w:r></w:p>"##;
    b += r##"<w:tbl><w:tblPr><w:tblStyle w:val="HostileTable"/><w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Styled table</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>head</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>c</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"##;
    b
}

fn styles_xml(p: &mut Places) -> String {
    let side = |p: &mut Places, s: &str, place: &str| format!(r##"<w:{s} w:val="single" w:sz="4" w:space="0" w:color="{}"/>"##, p.c(&format!("{place} {s}")));
    let para = format!(
        r##"<w:style w:type="paragraph" w:styleId="Hostile"><w:name w:val="Hostile"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:pBdr>{}{}</w:pBdr><w:shd w:val="clear" w:color="auto" w:fill="{}"/></w:pPr><w:rPr><w:color w:val="{}"/><w:highlight w:val="{}"/><w:u w:val="single" w:color="{}"/></w:rPr></w:style>"##,
        side(p, "top", "style para border"),
        side(p, "bottom", "style para border"),
        p.c("style para shading"),
        p.c("style run colour"),
        p.quiet("yellow"),
        p.c("style underline colour")
    );
    let chr = format!(r##"<w:style w:type="character" w:styleId="HostileChar"><w:name w:val="Hostile char"/><w:rPr><w:color w:val="{}"/><w:shd w:val="clear" w:color="auto" w:fill="{}"/></w:rPr></w:style>"##, p.c("character style colour"), p.c("character style shading"));
    let tbl = format!(
        r##"<w:style w:type="table" w:styleId="HostileTable"><w:name w:val="Hostile table"/><w:tblPr><w:tblBorders>{}{}</w:tblBorders></w:tblPr><w:tblStylePr w:type="firstRow"><w:rPr><w:color w:val="{}"/></w:rPr><w:tcPr><w:tcBorders>{}</w:tcBorders><w:shd w:val="clear" w:color="auto" w:fill="{}"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="band1Horz"><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="{}"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="firstCol"><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="{}"/></w:tcPr></w:tblStylePr></w:style>"##,
        side(p, "top", "table style border"),
        side(p, "insideH", "table style border"),
        p.c("table style first row text"),
        side(p, "bottom", "table style cell border"),
        p.c("table style first row shading"),
        p.c("table style band shading"),
        p.c("table style first column shading")
    );
    format!("{para}{chr}{tbl}")
}

fn numbering_xml(p: &mut Places) -> String {
    format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="7"><w:multiLevelType w:val="hybridMultilevel"/><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="o"/><w:lvlJc w:val="left"/><w:rPr><w:color w:val="{}"/><w:highlight w:val="{}"/><w:shd w:val="clear" w:color="auto" w:fill="{}"/></w:rPr></w:lvl></w:abstractNum><w:num w:numId="7"><w:abstractNumId w:val="7"/></w:num></w:numbering>"##,
        p.c("list label colour"),
        p.quiet("cyan"),
        p.c("list label shading")
    )
}

fn theme_xml(p: &mut Places) -> String {
    let mut slots = String::new();
    for (name, place) in [("dk2", "theme dk2"), ("lt2", "theme lt2"), ("accent1", "theme accent1"), ("accent2", "theme accent2"), ("accent3", "theme accent3"), ("accent4", "theme accent4"), ("accent5", "theme accent5"), ("accent6", "theme accent6"), ("hlink", "theme hlink"), ("folHlink", "theme folHlink")] {
        slots += &format!(r##"<a:{name}><a:srgbClr val="{}"/></a:{name}>"##, p.c(place));
    }
    format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="{A_NS}" name="Hostile"><a:themeElements><a:clrScheme name="Hostile"><a:dk1><a:sysClr val="windowText" lastClr="{}"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="{}"/></a:lt1>{slots}</a:clrScheme><a:fontScheme name="Office"><a:majorFont><a:latin typeface="Calibri Light"/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/></a:minorFont></a:fontScheme><a:fmtScheme name="Office"><a:fillStyleLst><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:ln><a:ln w="12700"/><a:ln w="19050"/></a:lnStyleLst></a:fmtScheme></a:themeElements></a:theme>"##,
        p.c("theme dk1"),
        p.c("theme lt1"),
        p.c("theme fill style"),
        p.c("theme line style")
    )
}

fn chart_xml(p: &mut Places) -> String {
    let sp = |p: &mut Places, place: &str| format!("<c:spPr>{}<a:ln>{}</a:ln></c:spPr>", solid(p, &format!("{place} fill")), solid(p, &format!("{place} line")));
    let txpr = |p: &mut Places, place: &str| format!(r##"<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr>{}</a:defRPr></a:pPr><a:endParaRPr lang="en-US"/></a:p></c:txPr>"##, solid(p, place));
    let ser = |p: &mut Places, i: usize| {
        format!(
            r##"<c:ser><c:idx val="{i}"/><c:order val="{i}"/><c:tx><c:strRef><c:f>Sheet1!$B$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>Series {i}</c:v></c:pt></c:strCache></c:strRef></c:tx>{}<c:invertIfNegative val="0"/><c:dPt><c:idx val="1"/><c:invertIfNegative val="0"/><c:bubble3D val="0"/>{}</c:dPt><c:dLbls>{}<c:showLegendKey val="0"/><c:showVal val="1"/><c:showCatName val="0"/><c:showSerName val="0"/><c:showPercent val="0"/><c:showBubbleSize val="0"/></c:dLbls><c:cat><c:strRef><c:f>Sheet1!$A$2:$A$3</c:f><c:strCache><c:ptCount val="2"/><c:pt idx="0"><c:v>A</c:v></c:pt><c:pt idx="1"><c:v>B</c:v></c:pt></c:strCache></c:strRef></c:cat><c:val><c:numRef><c:f>Sheet1!$B$2:$B$3</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="2"/><c:pt idx="0"><c:v>1</c:v></c:pt><c:pt idx="1"><c:v>2</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser>"##,
            sp(p, &format!("chart series {i}")),
            sp(p, &format!("chart point {i}")),
            txpr(p, &format!("chart data label {i}"))
        )
    };
    let (s0, s1) = (ser(p, 0), ser(p, 1));
    let title_run = format!(r##"<a:r><a:rPr lang="en-US">{}</a:rPr><a:t>Title</a:t></a:r>"##, solid(p, "chart title run"));
    let line = format!(
        r##"<c:lineChart><c:grouping val="standard"/><c:varyColors val="0"/><c:ser><c:idx val="2"/><c:order val="2"/><c:tx><c:strRef><c:f>Sheet1!$D$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>Line</c:v></c:pt></c:strCache></c:strRef></c:tx><c:spPr><a:ln w="28575">{}</a:ln></c:spPr><c:marker><c:symbol val="circle"/><c:size val="7"/><c:spPr>{}<a:ln>{}</a:ln></c:spPr></c:marker><c:cat><c:strRef><c:f>Sheet1!$A$2:$A$3</c:f><c:strCache><c:ptCount val="2"/><c:pt idx="0"><c:v>A</c:v></c:pt><c:pt idx="1"><c:v>B</c:v></c:pt></c:strCache></c:strRef></c:cat><c:val><c:numRef><c:f>Sheet1!$D$2:$D$3</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="2"/><c:pt idx="0"><c:v>3</c:v></c:pt><c:pt idx="1"><c:v>4</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser><c:marker val="1"/><c:axId val="1"/><c:axId val="2"/></c:lineChart>"##,
        solid(p, "chart line series line"),
        solid(p, "chart marker fill"),
        solid(p, "chart marker line")
    );
    let cat_ax = format!(r##"<c:catAx><c:axId val="1"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/>{}<c:crossAx val="2"/></c:catAx>"##, txpr(p, "chart axis text"));
    let val_ax = format!(
        r##"<c:valAx><c:axId val="2"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:majorGridlines><c:spPr><a:ln>{}</a:ln></c:spPr></c:majorGridlines><c:crossAx val="1"/></c:valAx>"##,
        solid(p, "chart gridline")
    );
    let plot_sp = sp(p, "chart plot area");
    let space_sp = sp(p, "chart space");
    let legend = format!(r##"<c:legend><c:legendPos val="b"/>{}</c:legend>"##, txpr(p, "chart legend text"));
    format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="{A_NS}" xmlns:r="{REL_NS}"><c:chart><c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p>{title_run}</a:p></c:rich></c:tx><c:overlay val="0"/></c:title><c:autoTitleDeleted val="0"/><c:plotArea><c:layout/><c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>{s0}{s1}<c:gapWidth val="150"/><c:axId val="1"/><c:axId val="2"/></c:barChart>{line}{cat_ax}{val_ax}{plot_sp}</c:plotArea>{legend}<c:plotVisOnly val="1"/></c:chart>{space_sp}</c:chartSpace>"##
    )
}

fn header_xml(p: &mut Places) -> String {
    format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word"><w:p><w:pPr><w:shd w:val="clear" w:color="auto" w:fill="{}"/></w:pPr><w:r><w:rPr><w:color w:val="{}"/></w:rPr><w:t>Header text</w:t></w:r><w:r><w:pict><v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" path="m@7,l@8,m@5,21600l@6,21600e"><v:path textpathok="t" o:connecttype="custom"/><v:textpath on="t" fitshape="t"/></v:shapetype><v:shape id="PowerPlusWaterMarkObject1" o:spid="_x0000_s2049" type="#_x0000_t136" style="position:absolute;margin-left:0;margin-top:0;width:412pt;height:247pt;rotation:315;z-index:-251658752;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" o:allowincell="f" fillcolor="#{}" stroked="f"><v:fill opacity=".5"/><v:textpath style="font-family:&quot;Calibri&quot;;font-size:1pt" string="DRAFT"/><w10:wrap anchorx="margin" anchory="margin"/></v:shape></w:pict></w:r></w:p></w:hdr>"##,
        p.c("header shading"),
        p.c("header run colour"),
        p.c("watermark colour")
    )
}

/// The document, as bytes.
fn build(hostile: bool) -> (Vec<u8>, Places) {
    let mut p = Places::new(hostile);
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let text = |pkg: &Package, name: &str| String::from_utf8(pkg.get(name).unwrap().to_vec()).unwrap();

    // parts first, so the document can name them
    pkg.set("word/theme/theme1.xml", theme_xml(&mut p).into_bytes());
    pkg.ensure_override("word/theme/theme1.xml", "application/vnd.openxmlformats-officedocument.theme+xml");
    pkg.set("word/charts/chart1.xml", chart_xml(&mut p).into_bytes());
    pkg.ensure_override("word/charts/chart1.xml", docxtor_core::chart::CT_CHART);
    let chart_rid = pkg.add_rel("word/document.xml", docxtor_core::chart::REL_CHART, "charts/chart1.xml", false);
    pkg.set("word/media/p.png", PNG.to_vec());
    pkg.ensure_default("png", "image/png");
    let pic_rid = pkg.add_rel("word/document.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image", "media/p.png", false);
    pkg.set("word/numbering.xml", numbering_xml(&mut p).into_bytes());
    pkg.ensure_override("word/numbering.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml");
    pkg.add_rel("word/document.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering", "numbering.xml", false);
    pkg.set("word/header1.xml", header_xml(&mut p).into_bytes());
    pkg.ensure_override("word/header1.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml");
    let header_rid = pkg.add_rel("word/document.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header", "header1.xml", false);

    // styles: before the closing tag
    let styles = text(&pkg, "word/styles.xml").replacen("</w:styles>", &format!("{}</w:styles>", styles_xml(&mut p)), 1);
    pkg.set("word/styles.xml", styles.into_bytes());

    // the page: its colour (and a gradient), its borders, a header
    let page_borders = format!(
        r##"<w:pgBorders w:offsetFrom="page"><w:top w:val="single" w:sz="8" w:space="24" w:color="{}"/><w:left w:val="single" w:sz="8" w:space="24" w:color="{}"/><w:bottom w:val="single" w:sz="8" w:space="24" w:color="{}"/><w:right w:val="single" w:sz="8" w:space="24" w:color="{}"/></w:pgBorders>"##,
        p.c("page border top"),
        p.c("page border left"),
        p.c("page border bottom"),
        p.c("page border right")
    );
    let background = format!(
        r##"<w:background w:color="{}"><v:background id="_x0000_s1025" o:bwmode="white" fillcolor="#{}"><v:fill type="gradient" color2="#{}" angle="-90" focus="100%" method="none"/></v:background></w:background>"##,
        p.c("page colour"),
        p.c("page fill colour"),
        p.c("page fill colour 2")
    );
    let xml = text(&pkg, "word/document.xml");
    let xml = xml.replacen("<w:document ", &format!("<w:document {NS} "), 1);
    let body_xml = body(&mut p, &chart_rid, &pic_rid);
    let xml = xml.replacen("<w:body><w:p/>", &format!("{background}<w:body>{body_xml}"), 1);
    assert!(xml.contains("Plain colours"), "the body was not replaced");
    // the section properties: put the header and borders in the last one
    let xml = if let Some(at) = xml.rfind("<w:sectPr") {
        let end = xml[at..].find('>').unwrap() + at + 1;
        let open_selfclosed = xml[at..end].ends_with("/>");
        assert!(!open_selfclosed, "the blank document's section properties are not empty");
        let (head, tail) = xml.split_at(end);
        let pg = tail.find("<w:pgSz").unwrap_or(0);
        let (a, b) = tail.split_at(pg);
        // the header reference goes first; the page borders after the page margins, which the blank document has
        let b = match b.find("<w:pgMar") {
            Some(m) => {
                let close = b[m..].find("/>").unwrap() + m + 2;
                format!("{}{}{}", &b[..close], page_borders, &b[close..])
            }
            None => format!("{page_borders}{b}"),
        };
        format!(r##"{head}<w:headerReference w:type="default" r:id="{header_rid}"/>{a}{b}"##)
    } else {
        xml
    };
    pkg.set("word/document.xml", xml.into_bytes());
    // the background only shows when the settings ask for it
    let settings = text(&pkg, "word/settings.xml");
    let settings = settings.replacen("</w:settings>", "<w:displayBackgroundShape/></w:settings>", 1);
    pkg.set("word/settings.xml", settings.into_bytes());
    (pkg.to_bytes().unwrap(), p)
}

/// Everything the engine hands to the page, as text.
fn outputs(doc: &mut Document) -> Vec<(String, String)> {
    fn json<T: Serialize>(v: &T) -> String {
        serde_json::to_string(v).unwrap()
    }
    let mut out: Vec<(String, String)> = Vec::new();
    let mut add = |name: &str, s: String| out.push((name.to_string(), s));
    let render = doc.render_all();
    add("render_all", json(&render));
    add("render_json", docxtor_core::document::render_json(|each| doc.render_all_each(each)));
    add("render_snapshot", json(&doc.render_snapshot()));
    add("page_setup", json(&doc.page_setup()));
    add("theme_colors", json(&doc.theme_colors()));
    add("theme_info", json(&doc.theme_info()));
    add("design_snapshot", json(&doc.design_snapshot()));
    add("style_list", json(&doc.style_list()));
    add("style_gallery", json(&doc.style_gallery()));
    add("style_set_previews", json(&doc.style_set_previews()));
    add("list_definitions", json(&doc.list_definitions()));
    add("objects", json(&doc.objects()));
    add("doc_part_controls", json(&doc.doc_part_controls()));
    add("tabs_info", json(&doc.tabs_info()));
    for s in doc.style_list() {
        add(&format!("style_details {}", s.id), json(&doc.style_details(&s.id)));
    }
    // each position the document has: format, find format, reveal, picture, shape, chart
    let mut ids = Vec::new();
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |p| ids.push((p.id, p.plain_text().chars().count())));
    for (id, len) in ids {
        add("table_details", json(&doc.table_details(id)));
        add("list_definition", json(&doc.list_definition(7)));
        for o in 0..=len.min(40) {
            let at = Pos::new(id, o);
            add("format_at", json(&doc.format_at(at)));
            add("find_format_at", json(&doc.find_format_at(id, o)));
            add("reveal_formatting", json(&doc.reveal_formatting(at)));
            add("picture_at", json(&doc.picture_at(at)));
            add("shape_at", json(&doc.shape_at(at)));
            add("chart_at", json(&doc.chart_at(at)));
        }
    }
    add("html", to_html(doc));
    add("rtf", docxtor_core::export::rtf::to_rtf(doc));
    add("markdown", docxtor_core::export::to_markdown(doc));
    add("inspect", json(&doc.inspect()));
    add("accessibility", json(&doc.accessibility()));
    let odt = docxtor_core::export::odt::to_odt(doc).unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(odt)).unwrap();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let name = f.name().unwrap().to_string();
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            add(&format!("odt {name}"), s);
        }
    }
    let epub = to_epub(doc, Some("2026-01-01T00:00:00Z")).unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(epub)).unwrap();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let name = f.name().unwrap().to_string();
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            add(&format!("epub {name}"), s);
        }
    }
    out
}

fn leaks(outputs: &[(String, String)]) -> Vec<String> {
    let mut found = Vec::new();
    let mut named = std::collections::BTreeSet::new();
    for (name, text) in outputs {
        // one line for each kind of output
        if named.contains(name.split(' ').next().unwrap_or(name)) {
            continue;
        }
        for m in MARKERS {
            if let Some(at) = text.find(m) {
                let from = text[..at].char_indices().rev().nth(80).map_or(0, |(i, _)| i);
                let to = text[at..].char_indices().nth(60).map_or(text.len(), |(i, _)| at + i);
                found.push(format!("{name}: {m:?} in ...{}...", &text[from..to]));
                named.insert(name.split(' ').next().unwrap_or(name).to_string());
                break;
            }
        }
    }
    found
}

/// Every string held by a field that names a colour (by its key) in the JSON: must be a colour, `auto`, `none` or one of Word's highlight names.
const COLOUR_KEYS: [&str; 9] = ["color", "colour", "fill", "shd", "shading", "highlight", "hex", "stroke", "background"];
const HIGHLIGHTS: [&str; 17] = ["yellow", "green", "cyan", "magenta", "blue", "red", "darkBlue", "darkCyan", "darkGreen", "darkMagenta", "darkRed", "darkYellow", "darkGray", "lightGray", "black", "white", "none"];

const THEME_NAMES: [&str; 20] = ["dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink", "tx1", "tx2", "bg1", "bg2", "text1", "text2", "background1", "background2"];

fn is_colour(key: &str, v: &str) -> bool {
    let hex = |n: usize| v.len() == n && v.bytes().all(|c| c.is_ascii_hexdigit());
    // a reference to the theme's colour: a name from its list, with a tint or a shade of two hex digits
    if key.to_ascii_lowercase().contains("theme") {
        return THEME_NAMES.contains(&v) || hex(2);
    }
    hex(6) || v.eq_ignore_ascii_case("auto") || v == "none" || HIGHLIGHTS.contains(&v)
}

fn colour_fields(v: &serde_json::Value, key: &str, out: &mut Vec<String>) {
    use serde_json::Value;
    match v {
        Value::String(s) => {
            if COLOUR_KEYS.iter().any(|k| key.to_ascii_lowercase().contains(k)) && !is_colour(key, s) {
                out.push(format!("{key}: {s:?}"));
            }
        }
        Value::Array(a) => a.iter().for_each(|x| colour_fields(x, key, out)),
        Value::Object(m) => m.iter().for_each(|(k, x)| colour_fields(x, k, out)),
        _ => {}
    }
}

#[test]
fn a_hostile_colour_in_any_place_never_reaches_the_page() {
    // the valid build: every place a colour was written shows in what the engine hands out
    let (bytes, places) = build(false);
    let mut valid = Document::open(&bytes).unwrap();
    let valid_out = outputs(&mut valid);
    let mut odd = std::collections::BTreeSet::new();
    for (name, text) in &valid_out {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
            let mut fields = Vec::new();
            colour_fields(&v, "", &mut fields);
            odd.extend(fields.into_iter().map(|f| format!("{} -> {f}", name.split(' ').next().unwrap_or(name))));
        }
    }
    assert!(odd.is_empty(), "fields named for a colour that hold something else, in the valid document: {odd:#?}");
    let everything = valid_out.iter().map(|(_, s)| s.to_ascii_uppercase()).collect::<Vec<_>>().join("\n");
    let missing: Vec<&str> = places.seen.iter().filter(|(_, c)| !everything.contains(c.as_str())).map(|(p, _)| p.as_str()).collect();
    // a place the engine does not hand out has no way to leak, but a place that is missing because the file was written wrongly is a hole in this
    // test: each one is named here with the reason
    // The model of a chart has the colours of its series and its points (their fill and their line); the colours of its title, its labels, its
    // axes, its gridlines, its markers and its plot area are drawn from the theme and the chart's style, and the theme's format styles are not read.
    let not_handed_out: &[&str] = &[
        "theme line style",
        "chart point 0 line",
        "chart point 1 line",
        "chart data label 0",
        "chart data label 1",
        "chart title run",
        "chart marker fill",
        "chart marker line",
        "chart axis text",
        "chart gridline",
        "chart plot area fill",
        "chart plot area line",
        "chart space fill",
        "chart space line",
        "chart legend text",
        // another value of the same place is drawn, or the model keeps one colour of two: a theme colour replaces the fallback that sits beside it (the
        // underline's, the cell's), a shape's `v:stroke` and `v:fill colors` win over its `strokecolor`, `fillcolor` and `color2`, a pattern's
        // colour is drawn only as the flat fill the cell shows, a line gradient has its first stop, a colour change its `from`
        "theme tinted underline fallback",
        "cell themed fill fallback",
        "vml fillcolor",
        "vml strokecolor",
        "vml fill color2",
        "cell shd colour",
        "line gradient stop 2",
        "picture colour change to",
        // not in the model: the shading of a whole table (Word draws it in the cells), the shading of a list's label
        "table shading",
        "list label shading",
    ];
    let unexpected: Vec<&&str> = missing.iter().filter(|m| !not_handed_out.contains(m)).collect();
    assert!(unexpected.is_empty(), "places whose colour the engine does not hand out (is the markup wrong?): {unexpected:#?}");
    let listed_but_found: Vec<&&str> = not_handed_out.iter().filter(|n| !missing.contains(n)).collect();
    assert!(listed_but_found.is_empty(), "places listed as not handed out that are: {listed_but_found:#?}");

    // the hostile build
    let (bytes, _) = build(true);
    let mut doc = Document::open(&bytes).unwrap();
    let out = outputs(&mut doc);
    assert!(out.iter().any(|(n, s)| n == "render_all" && s.contains("Plain colours")), "the hostile document opened");
    let mut found = leaks(&out);
    let mut odd = std::collections::BTreeSet::new();
    for (name, text) in &out {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
            let mut fields = Vec::new();
            colour_fields(&v, "", &mut fields);
            odd.extend(fields.into_iter().map(|f| format!("{} -> {f}", name.split(' ').next().unwrap_or(name))));
        }
    }
    found.extend(odd.into_iter().map(|o| format!("not a colour: {o}")));
    assert!(found.is_empty(), "{} outputs carry a hostile colour:\n{}", found.len(), found.join("\n"));

    // what is copied is the document's own XML (a fragment for a paste into another document, never drawn): the values stay in it, as they are in the file
    let (first, last) = (doc.render_snapshot().order[0], *doc.render_snapshot().order.last().unwrap());
    let fragment = serde_json::to_string(&doc.copy_fragment(Pos::new(first, 0), Pos::new(last, 1)).expect("a fragment")).unwrap();
    assert!(fragment.contains("evil.test"), "a copy keeps the file's own values");

    // and the file keeps what it had: the hostile values are still in the saved parts, byte for byte
    let saved = doc.save().unwrap();
    let pkg = Package::open(&saved).unwrap();
    let document = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(document.contains(&esc(BAD_CSS)) && document.contains(&esc(BAD_ATTR)), "the saved document keeps the colours it was given");
    for part in ["word/styles.xml", "word/numbering.xml", "word/theme/theme1.xml", "word/charts/chart1.xml", "word/header1.xml"] {
        let text = String::from_utf8(pkg.get(part).unwrap_or_else(|| panic!("{part} is saved")).to_vec()).unwrap();
        assert!(text.contains(&esc(BAD_CSS)) || text.contains(&esc(BAD_ATTR)), "{part} keeps the colours it was given");
    }
}
