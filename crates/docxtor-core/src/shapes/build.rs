//! Writing new shapes as Word writes them: an `mc:AlternateContent` whose `mc:Choice Requires="wps"` holds the
//! DrawingML shape (`wp:anchor` or `wp:inline`) and whose `mc:Fallback` holds the VML equivalent.

use super::{LineProps, LineSpec, EMU_PER_PT};
use crate::model::{write_paragraph, Inline, Paragraph, Run};
use crate::xml::{self, Element, Node};

pub const NS_MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
pub const NS_WPS: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingShape";
pub const NS_WP14: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing";
pub const NS_V: &str = "urn:schemas-microsoft-com:vml";
pub const NS_O: &str = "urn:schemas-microsoft-com:office:office";
pub const NS_W10: &str = "urn:schemas-microsoft-com:office:word";
pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const NS_WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
pub const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// The namespaces a root element needs for shapes.
pub const ROOT_NS: &[(&str, &str)] = &[("mc", NS_MC), ("wps", NS_WPS), ("wp14", NS_WP14), ("v", NS_V), ("o", NS_O), ("w10", NS_W10), ("a", NS_A), ("wp", NS_WP)];

/// Every preset DrawingML knows (`ST_ShapeType`). An unknown name makes Word reject the file.
pub const PRESETS: &[&str] = &[
    "line", "lineInv", "triangle", "rtTriangle", "rect", "diamond", "parallelogram", "trapezoid", "nonIsoscelesTrapezoid", "pentagon", "hexagon",
    "heptagon", "octagon", "decagon", "dodecagon", "star4", "star5", "star6", "star7", "star8", "star10", "star12", "star16", "star24", "star32",
    "roundRect", "round1Rect", "round2SameRect", "round2DiagRect", "snipRoundRect", "snip1Rect", "snip2SameRect", "snip2DiagRect", "plaque",
    "ellipse", "teardrop", "homePlate", "chevron", "pieWedge", "pie", "blockArc", "donut", "noSmoking", "rightArrow", "leftArrow", "upArrow",
    "downArrow", "stripedRightArrow", "notchedRightArrow", "bentUpArrow", "leftRightArrow", "upDownArrow", "leftUpArrow", "leftRightUpArrow",
    "quadArrow", "leftArrowCallout", "rightArrowCallout", "upArrowCallout", "downArrowCallout", "leftRightArrowCallout", "upDownArrowCallout",
    "quadArrowCallout", "bentArrow", "uturnArrow", "circularArrow", "leftCircularArrow", "leftRightCircularArrow", "curvedRightArrow",
    "curvedLeftArrow", "curvedUpArrow", "curvedDownArrow", "swooshArrow", "cube", "can", "lightningBolt", "heart", "sun", "moon", "smileyFace",
    "irregularSeal1", "irregularSeal2", "foldedCorner", "bevel", "frame", "halfFrame", "corner", "diagStripe", "chord", "arc", "leftBracket",
    "rightBracket", "leftBrace", "rightBrace", "bracketPair", "bracePair", "straightConnector1", "bentConnector2", "bentConnector3",
    "bentConnector4", "bentConnector5", "curvedConnector2", "curvedConnector3", "curvedConnector4", "curvedConnector5", "callout1", "callout2",
    "callout3", "accentCallout1", "accentCallout2", "accentCallout3", "borderCallout1", "borderCallout2", "borderCallout3",
    "accentBorderCallout1", "accentBorderCallout2", "accentBorderCallout3", "wedgeRectCallout", "wedgeRoundRectCallout", "wedgeEllipseCallout",
    "cloudCallout", "cloud", "ribbon", "ribbon2", "ellipseRibbon", "ellipseRibbon2", "leftRightRibbon", "verticalScroll", "horizontalScroll", "wave",
    "doubleWave", "plus", "flowChartProcess", "flowChartAlternateProcess", "flowChartDecision", "flowChartInputOutput",
    "flowChartPredefinedProcess", "flowChartInternalStorage", "flowChartDocument", "flowChartMultidocument", "flowChartTerminator",
    "flowChartPreparation", "flowChartManualInput", "flowChartManualOperation", "flowChartConnector", "flowChartOffpageConnector",
    "flowChartPunchedCard", "flowChartPunchedTape", "flowChartSummingJunction", "flowChartOr", "flowChartCollate", "flowChartSort",
    "flowChartExtract", "flowChartMerge", "flowChartOnlineStorage", "flowChartDelay", "flowChartMagneticTape", "flowChartMagneticDisk",
    "flowChartMagneticDrum", "flowChartDisplay", "actionButtonBlank", "actionButtonHome", "actionButtonHelp", "actionButtonInformation",
    "actionButtonForwardNext", "actionButtonBackPrevious", "actionButtonEnd", "actionButtonBeginning", "actionButtonReturn",
    "actionButtonDocument", "actionButtonSound", "actionButtonMovie", "gear6", "gear9", "funnel", "mathPlus", "mathMinus", "mathMultiply",
    "mathDivide", "mathEqual", "mathNotEqual", "cornerTabs", "squareTabs", "plaqueTabs", "chartX", "chartStar", "chartPlus",
];

/// A preset by its DrawingML name (any capitalisation) or a plain-language alias (`rectangle`, `oval`, `arrow`).
pub fn preset(name: &str) -> Result<&'static str, String> {
    let key: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
    if let Some(p) = PRESETS.iter().find(|p| p.to_ascii_lowercase() == key) {
        return Ok(p);
    }
    let alias = match key.as_str() {
        "rectangle" | "box" | "square" | "textbox" => "rect",
        "roundedrectangle" | "roundedrect" | "roundrectangle" => "roundRect",
        "oval" | "circle" => "ellipse",
        "arrow" | "rightarrow" => "rightArrow",
        "star" => "star5",
        "righttriangle" => "rtTriangle",
        "straightline" | "straightarrow" => "line",
        "connector" | "straightconnector" => "straightConnector1",
        "speechbubble" | "callout" => "wedgeRoundRectCallout",
        "pentagonarrow" => "homePlate",
        _ => return Err(format!("unknown shape '{name}'; use a DrawingML preset such as rect, roundRect, ellipse, triangle, rtTriangle, diamond, pentagon, hexagon, star5, rightArrow, leftArrow, upArrow, downArrow, chevron, cloud, heart or line")),
    };
    Ok(alias)
}

/// Lines and connectors: open shapes drawn as a stroke between two corners.
pub fn is_line(geometry: &str) -> bool {
    geometry == "line" || geometry == "lineInv" || geometry.contains("Connector")
}

/// A new shape.
#[derive(Debug, Clone)]
pub struct Spec {
    pub geometry: String,
    pub w: f64,
    pub h: f64,
    /// Hex fill; None for no fill.
    pub fill: Option<String>,
    /// None for no outline.
    pub line: Option<LineLook>,
    /// Text inside (lines are paragraphs); None for a shape without a text frame.
    pub text: Option<String>,
    pub text_box: bool,
    pub floating: bool,
    pub x: f64,
    pub y: f64,
    pub id: u32,
}

#[derive(Debug, Clone, Default)]
pub struct LineLook {
    pub color: String,
    pub width_pt: f64,
    pub dash: Option<String>,
    pub head: Option<String>,
    pub tail: Option<String>,
    /// Opacity below 1; None for opaque.
    pub alpha: Option<f64>,
    /// `dbl`, `thickThin`, `thinThick` or `tri`; None for a single line.
    pub compound: Option<String>,
    /// `round` or `square`; None for flat.
    pub cap: Option<String>,
    /// `round`, `bevel` or `miter`; None when the file says nothing.
    pub join: Option<String>,
    /// Miter limit in percent.
    pub miter: Option<f64>,
    pub head_w: Option<String>,
    pub head_len: Option<String>,
    pub tail_w: Option<String>,
    pub tail_len: Option<String>,
    /// The op sets the colour or the opacity: the line's fill is written again.
    pub recolor: bool,
    /// What the op itself said (the fields left out stay as the file has them when a line is changed in place).
    pub set: LineProps,
}

fn pick(v: &Option<String>, old: &Option<String>, ok: &[&str], name: &str) -> Result<Option<String>, String> {
    match v.as_deref() {
        None => Ok(old.clone()),
        Some(x) if ok.contains(&x) => Ok(Some(x.to_string())),
        Some(x) => Err(format!("unknown {name} '{x}'; use {}", ok.join(", "))),
    }
}

impl LineLook {
    pub fn from_props(p: &LineProps, base: &LineLook) -> Result<LineLook, String> {
        let color = match &p.color {
            Some(c) => super::hex6(c).ok_or_else(|| format!("'{c}' is not a colour; use six hex digits like 000000"))?,
            None => base.color.clone(),
        };
        let width_pt = p.width_pt.unwrap_or(base.width_pt);
        if !(0.0..=1584.0).contains(&width_pt) {
            return Err("the line width must be between 0 and 1584 points".into());
        }
        let arrow = |a: &Option<String>, old: &Option<String>| -> Result<Option<String>, String> {
            match a.as_deref() {
                None => Ok(old.clone()),
                Some("none") => Ok(None),
                Some(t @ ("triangle" | "stealth" | "diamond" | "oval" | "arrow")) => Ok(Some(t.into())),
                Some(other) => Err(format!("unknown arrow '{other}'; use none, triangle, stealth, diamond, oval or arrow")),
            }
        };
        let dash = match p.dash.as_deref() {
            None => base.dash.clone(),
            Some("solid") => None,
            Some(d @ ("dot" | "dash" | "lgDash" | "dashDot" | "lgDashDot" | "lgDashDotDot" | "sysDash" | "sysDot" | "sysDashDot" | "sysDashDotDot")) => Some(d.into()),
            Some(other) => return Err(format!("unknown dash '{other}'; use solid, dot, dash, lgDash, dashDot, lgDashDot, lgDashDotDot, sysDash, sysDot, sysDashDot or sysDashDotDot")),
        };
        let alpha = match p.alpha {
            None => base.alpha,
            Some(a) if (0.0..=1.0).contains(&a) => (a < 1.0).then_some(a),
            Some(_) => return Err("alpha must be between 0 (clear) and 1 (opaque)".into()),
        };
        let compound = match p.compound.as_deref() {
            None => base.compound.clone(),
            Some("single") => None,
            _ => pick(&p.compound, &base.compound, &["dbl", "thickThin", "thinThick", "tri"], "compound")?,
        };
        let cap = match p.cap.as_deref() {
            None => base.cap.clone(),
            Some("flat") => None,
            _ => pick(&p.cap, &base.cap, &["round", "square"], "cap")?,
        };
        let join = pick(&p.join, &base.join, &["miter", "round", "bevel"], "join")?;
        if let Some(m) = p.miter {
            if !(0.0..=10000.0).contains(&m) {
                return Err("miter must be between 0 and 10000 percent".into());
            }
        }
        let size = |v: &Option<String>, old: &Option<String>, name: &str| pick(v, old, &["sm", "med", "lg"], name);
        Ok(LineLook {
            color,
            width_pt,
            dash,
            head: arrow(&p.head_arrow, &base.head)?,
            tail: arrow(&p.tail_arrow, &base.tail)?,
            alpha,
            compound,
            cap,
            join,
            miter: p.miter.or(base.miter),
            head_w: size(&p.head_w, &base.head_w, "head_w")?,
            head_len: size(&p.head_len, &base.head_len, "head_len")?,
            tail_w: size(&p.tail_w, &base.tail_w, "tail_w")?,
            tail_len: size(&p.tail_len, &base.tail_len, "tail_len")?,
            recolor: p.color.is_some() || p.alpha.is_some(),
            set: p.clone(),
        })
    }

    pub fn default_for(text_box: bool) -> LineLook {
        LineLook { color: "000000".into(), width_pt: if text_box { 0.75 } else { 1.0 }, ..Default::default() }
    }
}

/// Resolve an op's `line` against a default: `None` leaves the default.
pub fn line_look(spec: Option<&LineSpec>, default: Option<LineLook>) -> Result<Option<LineLook>, String> {
    match spec {
        None => Ok(default),
        Some(LineSpec::None) => Ok(None),
        Some(LineSpec::Set(p)) => LineLook::from_props(p, &default.unwrap_or_else(|| LineLook::default_for(false))).map(Some),
    }
}

/// A fill from an op: `none`, or hex.
pub fn fill_hex(v: &str) -> Result<Option<String>, String> {
    if v.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    super::hex6(v).map(Some).ok_or_else(|| format!("'{v}' is not a colour; use six hex digits like FFF2CC, or none"))
}

fn emu(pt: f64) -> i64 {
    (pt * EMU_PER_PT).round() as i64
}

fn srgb(hex: &str) -> String {
    format!(r#"<a:solidFill><a:srgbClr val="{hex}"/></a:solidFill>"#)
}

pub fn fill_xml(fill: &Option<String>) -> String {
    match fill {
        Some(h) => srgb(h),
        None => "<a:noFill/>".into(),
    }
}

pub fn ln_xml(line: &Option<LineLook>) -> String {
    let Some(l) = line else { return "<a:ln><a:noFill/></a:ln>".into() };
    let dash = l.dash.as_ref().map(|d| format!(r#"<a:prstDash val="{d}"/>"#)).unwrap_or_default();
    let end = |tag: &str, a: &Option<String>, w: &Option<String>, len: &Option<String>| match a {
        Some(t) => format!(r#"<a:{tag} type="{t}"{}{}/>"#, w.as_ref().map(|w| format!(r#" w="{w}""#)).unwrap_or_default(), len.as_ref().map(|l| format!(r#" len="{l}""#)).unwrap_or_default()),
        None => String::new(),
    };
    let attrs = format!(
        "{}{}",
        l.cap.as_deref().map(|c| format!(r#" cap="{}""#, if c == "round" { "rnd" } else { "sq" })).unwrap_or_default(),
        l.compound.as_ref().map(|c| format!(r#" cmpd="{c}""#)).unwrap_or_default()
    );
    let join = match l.join.as_deref() {
        Some("round") => "<a:round/>".to_string(),
        Some("bevel") => "<a:bevel/>".to_string(),
        _ => format!(r#"<a:miter lim="{}"/>"#, (l.miter.unwrap_or(800.0) * 1000.0).round() as i64),
    };
    let fill = super::fill::clr(&l.color, l.alpha.unwrap_or(1.0));
    format!(
        r#"<a:ln w="{}"{attrs}><a:solidFill>{fill}</a:solidFill>{}{join}{}{}</a:ln>"#,
        emu(l.width_pt),
        dash,
        end("headEnd", &l.head, &l.head_w, &l.head_len),
        end("tailEnd", &l.tail, &l.tail_w, &l.tail_len)
    )
}

/// The paragraphs of a shape's text: one per line.
pub fn text_paragraphs(text: &str, centered: bool) -> Vec<Element> {
    text.split('\n')
        .map(|line| {
            let mut p = Paragraph::new(0);
            if centered {
                p.ppr = Some(Element::new("w:pPr").with_child(Element::new("w:jc").with_attr("w:val", "center")));
            }
            if !line.is_empty() {
                p.inlines.push(Inline::Run(Run::of_text(None, line)));
            }
            write_paragraph(&p)
        })
        .collect()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}

/// The display name Word gives a new shape.
pub fn default_name(spec: &Spec) -> String {
    let base = if spec.text_box {
        "Text Box"
    } else {
        match spec.geometry.as_str() {
            "rect" => "Rectangle",
            "roundRect" => "Rectangle: Rounded Corners",
            "ellipse" => "Oval",
            "triangle" => "Isosceles Triangle",
            "line" | "straightConnector1" => "Straight Connector",
            "rightArrow" => "Arrow: Right",
            "leftArrow" => "Arrow: Left",
            "upArrow" => "Arrow: Up",
            "downArrow" => "Arrow: Down",
            _ => "Shape",
        }
    };
    format!("{base} {}", spec.id)
}

/// The shape's markup: `mc:AlternateContent` with the DrawingML choice and the VML fallback. Namespace
/// declarations are left to the document root.
pub fn build(spec: &Spec) -> Element {
    let name = esc(&default_name(spec));
    let (cx, cy) = (emu(spec.w), emu(spec.h));
    let id = spec.id;
    let line_shape = is_line(&spec.geometry);
    let has_text = spec.text.is_some() && !line_shape;
    let (px, py) = (emu(spec.x), emu(spec.y));
    let frame = if spec.floating {
        format!(
            r#"<wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="{}" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>{px}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>{py}</wp:posOffset></wp:positionV><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapSquare wrapText="bothSides"/>"#,
            251658240 + id as u64
        )
    } else {
        format!(r#"<wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>"#)
    };
    let close = if spec.floating {
        r#"<wp14:sizeRelH relativeFrom="margin"><wp14:pctWidth>0</wp14:pctWidth></wp14:sizeRelH><wp14:sizeRelV relativeFrom="margin"><wp14:pctHeight>0</wp14:pctHeight></wp14:sizeRelV></wp:anchor>"#
    } else {
        "</wp:inline>"
    };
    let nv = if line_shape { "<wps:cNvCnPr/>".to_string() } else if spec.text_box { r#"<wps:cNvSpPr txBox="1"/>"#.to_string() } else { "<wps:cNvSpPr/>".to_string() };
    let (anchor, autofit) = if spec.text_box { ("t", "<a:noAutofit/>") } else { ("ctr", "<a:noAutofit/>") };
    let txbx = if has_text { "<wps:txbx><w:txbxContent/></wps:txbx>" } else { "" };
    let src = format!(
        r#"<mc:AlternateContent><mc:Choice Requires="wps"><w:drawing>{frame}<wp:docPr id="{id}" name="{name}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="{NS_WPS}"><wps:wsp>{nv}<wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="{}"><a:avLst/></a:prstGeom>{}{}</wps:spPr>{txbx}<wps:bodyPr rot="0" vert="horz" wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" anchor="{anchor}" anchorCtr="0">{autofit}</wps:bodyPr></wps:wsp></a:graphicData></a:graphic>{close}</w:drawing></mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent>"#,
        spec.geometry,
        fill_xml(&spec.fill),
        ln_xml(&spec.line),
        vml_xml(spec, &name),
    );
    let mut el = parse_fragment(&src);
    if let Some(text) = spec.text.as_deref().filter(|_| has_text) {
        let paras = text_paragraphs(text, !spec.text_box);
        fill_contents(&mut el, &paras);
    }
    el
}

/// Parse markup that uses the shape namespaces, then drop the declarations (they live on the document root).
fn parse_fragment(body: &str) -> Element {
    let decls: String = ROOT_NS.iter().map(|(p, u)| format!(r#" xmlns:{p}="{u}""#)).collect::<String>() + &format!(r#" xmlns:w="{NS_W}""#);
    let src = body.replacen('>', &format!("{decls}>"), 1);
    let mut el = xml::parse(src.as_bytes()).expect("valid shape xml").root;
    el.attrs.retain(|(k, _)| !k.starts_with("xmlns:"));
    el
}

fn fill_contents(el: &mut Element, paras: &[Element]) {
    fn go(e: &mut Element, paras: &[Element]) {
        for c in e.elements_mut() {
            if c.name == "w:txbxContent" {
                c.children = paras.iter().cloned().map(Node::Element).collect();
            } else {
                go(c, paras);
            }
        }
    }
    go(el, paras);
}

fn vml_fill(fill: &Option<String>) -> String {
    match fill {
        Some(h) => format!(r##" fillcolor="#{h}""##),
        None => r#" filled="f""#.into(),
    }
}

fn vml_stroke(line: &Option<LineLook>) -> String {
    match line {
        Some(l) => format!(r##" strokecolor="#{}" strokeweight="{}pt""##, l.color, trim(l.width_pt)),
        None => r#" stroked="f""#.into(),
    }
}

fn trim(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// The VML a shape is also written as, for readers that do not know DrawingML shapes. Rectangles, rounded
/// rectangles, ovals, text boxes and lines are exact; other presets fall back to their bounding rectangle.
pub fn vml_xml(spec: &Spec, name: &str) -> String {
    let id = spec.id;
    let spid = format!("_x0000_s{}", 1024 + id);
    let (w, h) = (trim(spec.w), trim(spec.h));
    let style = if spec.floating {
        format!(
            "position:absolute;margin-left:{}pt;margin-top:{}pt;width:{w}pt;height:{h}pt;z-index:{};visibility:visible;mso-wrap-style:square;mso-wrap-distance-left:9pt;mso-wrap-distance-top:0;mso-wrap-distance-right:9pt;mso-wrap-distance-bottom:0;mso-position-horizontal:absolute;mso-position-horizontal-relative:text;mso-position-vertical:absolute;mso-position-vertical-relative:text{}",
            trim(spec.x),
            trim(spec.y),
            251658240 + id as u64,
            if spec.text_box { ";v-text-anchor:top" } else { ";v-text-anchor:middle" }
        )
    } else {
        format!("width:{w}pt;height:{h}pt;visibility:visible;mso-wrap-style:square{}", if spec.text_box { ";v-text-anchor:top" } else { ";v-text-anchor:middle" })
    };
    let wrap = if spec.floating { r#"<w10:wrap type="square"/>"# } else { "" };
    let textbox = if spec.text.is_some() && !is_line(&spec.geometry) { r#"<v:textbox><w:txbxContent/></v:textbox>"# } else { "" };
    if is_line(&spec.geometry) {
        return format!(
            r#"<w:pict><v:line id="{name}" o:spid="{spid}" style="{style}"{} from="0,0" to="{w}pt,{h}pt">{}{wrap}</v:line></w:pict>"#,
            vml_stroke(&spec.line),
            vml_line_children(&spec.line)
        );
    }
    let common = format!(r#" id="{name}" o:spid="{spid}" style="{style}"{}{}"#, vml_fill(&spec.fill), vml_stroke(&spec.line));
    if spec.text_box {
        return format!(
            r##"<w:pict><v:shapetype id="_x0000_t202" coordsize="21600,21600" o:spt="202" path="m,l,21600r21600,l21600,xe"><v:stroke joinstyle="miter"/><v:path gradientshapeok="t" o:connecttype="rect"/></v:shapetype><v:shape{common} type="#_x0000_t202">{textbox}{wrap}</v:shape></w:pict>"##
        );
    }
    let tag = match spec.geometry.as_str() {
        "roundRect" => "v:roundrect",
        "ellipse" => "v:oval",
        _ => "v:rect",
    };
    let arc = if tag == "v:roundrect" { r#" arcsize="10923f""# } else { "" };
    format!(r#"<w:pict><{tag}{common}{arc}>{textbox}{wrap}</{tag}></w:pict>"#)
}

fn vml_line_children(line: &Option<LineLook>) -> String {
    let Some(l) = line else { return String::new() };
    let arrow = |a: &Option<String>| -> Option<&'static str> {
        a.as_deref().map(|t| match t {
            "triangle" => "block",
            "stealth" => "classic",
            "diamond" => "diamond",
            "oval" => "oval",
            _ => "open",
        })
    };
    let dash = l.dash.as_deref().map(|d| match d {
        "dot" | "sysDot" => "dot",
        "lgDash" => "longdash",
        "dashDot" | "sysDashDot" => "dashdot",
        "lgDashDot" => "longdashdot",
        "lgDashDotDot" | "sysDashDotDot" => "longdashdotdot",
        _ => "dash",
    });
    if dash.is_none() && l.head.is_none() && l.tail.is_none() {
        return String::new();
    }
    let mut s = String::from("<v:stroke");
    if let Some(d) = dash {
        s.push_str(&format!(r#" dashstyle="{d}""#));
    }
    if let Some(a) = arrow(&l.head) {
        s.push_str(&format!(r#" startarrow="{a}""#));
    }
    if let Some(a) = arrow(&l.tail) {
        s.push_str(&format!(r#" endarrow="{a}""#));
    }
    s.push_str("/>");
    s
}

/// Make `root` declare the namespaces shapes need, and list `wp14` as ignorable (as Word does).
pub fn ensure_root_ns(root: &mut Element) {
    for (p, u) in ROOT_NS {
        let key = format!("xmlns:{p}");
        if root.attr(&key).is_none() {
            root.set_attr(key, *u);
        }
    }
    let mut tokens: Vec<String> = root.attr("mc:Ignorable").map(|v| v.split_whitespace().map(String::from).collect()).unwrap_or_default();
    if !tokens.iter().any(|t| t == "wp14") {
        tokens.push("wp14".into());
    }
    root.set_attr("mc:Ignorable", tokens.join(" "));
}
