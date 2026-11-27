//! Document themes (`word/theme/theme1.xml`): the colour scheme, the font scheme and the format scheme that
//! Word's Design tab switches. This module holds Word's built-in colour schemes, font schemes and themes, writes a
//! theme part as Word does, and changes the colours or fonts of one a document already has without touching the
//! rest of it.
//!
//! The colours and fonts of Office, Office 2007-2010 and the colour schemes are Word's own. A built-in theme here is
//! its colour scheme and font scheme; the format scheme (the fills, lines and effects shapes pick from) is Office's
//! for all of them, so a few themes differ from Word's in how a shape's effects look.

use crate::xml::{self, Element, Node, XmlDoc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const CT_THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";

/// The twelve slots of a colour scheme, in the order of Word's dialog.
pub const SLOTS: [&str; 12] = crate::shapes::theme::SLOTS;

/// A colour scheme: a name and hex colours (no `#`, upper case) by slot (`dk1`, `lt1`, `dk2`, `lt2`, `accent1` to
/// `accent6`, `hlink`, `folHlink`). A slot left out keeps the colour the document has.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ColorScheme {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub colors: BTreeMap<String, String>,
}

/// A font scheme: the heading font (major) and the body font (minor).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FontScheme {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub major: String,
    #[serde(default)]
    pub minor: String,
}

/// A whole theme as the Themes gallery offers it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ThemeDef {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub colors: ColorScheme,
    pub fonts: FontScheme,
}

/// What a document's theme is now.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeInfo {
    /// The theme's name (`Office Theme`, `Facet`).
    pub name: String,
    pub colors: ColorScheme,
    pub fonts: FontScheme,
}

// ---------------------------------------------------------------------------
// Word's built-in schemes and themes
// ---------------------------------------------------------------------------

/// `(name, [dk2, lt2, accent1..accent6, hlink, folHlink])`; dk1 and lt1 are black and white in all of them.
type Palette = (&'static str, [&'static str; 10]);

const COLOR_SCHEMES: &[Palette] = &[
    ("Office", ["44546A", "E7E6E6", "4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47", "0563C1", "954F72"]),
    ("Office 2007-2010", ["1F497D", "EEECE1", "4F81BD", "C0504D", "9BBB59", "8064A2", "4BACC6", "F79646", "0000FF", "800080"]),
    ("Grayscale", ["000000", "F7F7F7", "DDDDDD", "B2B2B2", "969696", "808080", "5F5F5F", "4D4D4D", "5F5F5F", "919191"]),
    ("Blue Warm", ["242852", "ACCBF9", "4A66AC", "629DD1", "297FD5", "7F8FA9", "5AA2AE", "9D90A0", "9454C3", "3EBBF0"]),
    ("Blue", ["17406D", "DBEFF9", "0F6FC6", "009DD9", "0BD0D9", "10CF9B", "7CCA62", "A5C249", "F49100", "85DFD0"]),
    ("Blue II", ["335B74", "DFE3E5", "1CADE4", "2683C6", "27CED7", "42BA97", "3E8853", "62A39F", "6B9F25", "B26B02"]),
    ("Blue Green", ["373545", "CEDBE6", "3494BA", "58B6C0", "75BDA7", "7A8C8E", "84ACB6", "2683C6", "6B9F25", "9F6715"]),
    ("Green", ["455F51", "E3DED1", "549E39", "8AB833", "C0CF3A", "029676", "4AB5C4", "0989B1", "6B9F25", "BA6906"]),
    ("Yellow", ["39302A", "E5DEDB", "FFCA08", "F8931D", "CE8D3E", "EC7016", "E64823", "9C6A6A", "9454C3", "3EBBF0"]),
    ("Orange", ["637052", "CCDDEA", "E48312", "BD582C", "865640", "9B8357", "C2BC80", "94A088", "2998E3", "8C8C8C"]),
    ("Orange Red", ["505046", "EEECE1", "D34817", "9B2D1F", "A28E6A", "956251", "918485", "855D5D", "CC9900", "96A9A9"]),
    ("Red Orange", ["505046", "EEECE1", "E84C22", "FFBD47", "B64926", "FF8427", "CC9900", "B22600", "6B9F25", "9F6715"]),
    ("Red", ["454551", "D8D9DC", "A5300F", "D55816", "E19825", "B19C7D", "7F5F52", "B27D49", "6B9F25", "9F6715"]),
    ("Red Violet", ["632E62", "E4DE95", "E32D91", "C830CC", "4EA6DC", "4775E7", "8971E1", "D54773", "3EBBF0", "9454C3"]),
    ("Median", ["775F55", "EBDDC3", "94B6D2", "DD8047", "A5AB81", "D8B25C", "7BA79D", "968C8C", "F7B615", "704404"]),
    ("Paper", ["444D26", "FEFAC9", "A5B592", "F3A447", "E7BC29", "D092A7", "9C85C0", "809EC2", "8C8C8C", "8C8C8C"]),
    ("Marquee", ["5E5E5E", "FFFFFF", "418AB3", "A6B727", "F69200", "838383", "FEC306", "DF5327", "F59E00", "B2B2B2"]),
    ("Slipstream", ["212745", "B4DCFA", "4E67C8", "5ECCF3", "A7EA52", "5DCEAF", "FF8021", "F14124", "56C7AA", "59A8D1"]),
    ("Aspect", ["323F4F", "F5C201", "F07F09", "9F2936", "1B587C", "4E8542", "604878", "C19859", "6B9F25", "9F6715"]),
];

/// Font schemes: `(name, heading font, body font)`.
const FONT_SCHEMES: &[(&str, &str, &str)] = &[
    ("Office", "Calibri Light", "Calibri"),
    ("Office 2023", "Aptos Display", "Aptos"),
    ("Office 2007-2010", "Cambria", "Calibri"),
    ("Calibri", "Calibri", "Calibri"),
    ("Arial", "Arial", "Arial"),
    ("Corbel", "Corbel", "Corbel"),
    ("Candara", "Candara", "Candara"),
    ("Franklin Gothic", "Franklin Gothic Medium", "Franklin Gothic Book"),
    ("Century Gothic", "Century Gothic", "Century Gothic"),
    ("Tw Cen MT", "Tw Cen MT", "Tw Cen MT"),
    ("Tw Cen MT Condensed", "Tw Cen MT Condensed", "Tw Cen MT"),
    ("Cambria", "Cambria", "Cambria"),
    ("Garamond", "Garamond", "Garamond"),
    ("Georgia", "Georgia", "Georgia"),
    ("Trebuchet MS", "Trebuchet MS", "Trebuchet MS"),
    ("Verdana", "Verdana", "Verdana"),
    ("Times New Roman", "Times New Roman", "Times New Roman"),
    ("Rockwell", "Rockwell Condensed", "Rockwell"),
    ("Impact and Gill Sans", "Impact", "Gill Sans MT"),
    ("Arial Black and Arial", "Arial Black", "Arial"),
    ("Consolas and Verdana", "Consolas", "Verdana"),
    ("Calibri Light and Constantia", "Calibri Light", "Constantia"),
    ("Georgia and Verdana", "Georgia", "Verdana"),
    ("Cambria and Calibri", "Cambria", "Calibri"),
];

/// Built-in themes: `(id, name, colour scheme, heading font, body font)`. The colours are looked up in `COLOR_SCHEMES`
/// or given in full by `OWN_PALETTES`.
const THEMES: &[(&str, &str, &str, &str, &str)] = &[
    ("office", "Office", "Office", "Calibri Light", "Calibri"),
    ("office-2023", "Office 2023", "Office 2023", "Aptos Display", "Aptos"),
    ("office-2007", "Office 2007-2010", "Office 2007-2010", "Cambria", "Calibri"),
    ("facet", "Facet", "Facet", "Trebuchet MS", "Trebuchet MS"),
    ("integral", "Integral", "Blue II", "Tw Cen MT Condensed", "Tw Cen MT"),
    ("ion", "Ion", "Ion", "Century Gothic", "Century Gothic"),
    ("ion-boardroom", "Ion Boardroom", "Ion Boardroom", "Century Gothic", "Century Gothic"),
    ("organic", "Organic", "Organic", "Garamond", "Garamond"),
    ("retrospect", "Retrospect", "Orange", "Calibri Light", "Calibri"),
    ("slice", "Slice", "Slice", "Century Gothic", "Century Gothic"),
    ("wisp", "Wisp", "Wisp", "Century Gothic", "Century Gothic"),
    ("basis", "Basis", "Basis", "Corbel", "Corbel"),
    ("berlin", "Berlin", "Berlin", "Trebuchet MS", "Trebuchet MS"),
    ("circuit", "Circuit", "Circuit", "Tw Cen MT", "Tw Cen MT"),
    ("damask", "Damask", "Damask", "Corbel", "Corbel"),
    ("depth", "Depth", "Depth", "Calibri", "Calibri"),
    ("droplet", "Droplet", "Droplet", "Tw Cen MT", "Tw Cen MT"),
    ("frame", "Frame", "Frame", "Corbel", "Corbel"),
    ("gallery", "Gallery", "Gallery", "Garamond", "Garamond"),
    ("main-event", "Main Event", "Main Event", "Impact", "Gill Sans MT"),
    ("mesh", "Mesh", "Mesh", "Century Gothic", "Century Gothic"),
    ("parallax", "Parallax", "Parallax", "Corbel", "Corbel"),
    ("savon", "Savon", "Savon", "Century Gothic", "Century Gothic"),
    ("view", "View", "View", "Calibri", "Calibri"),
    ("vapor-trail", "Vapor Trail", "Vapor Trail", "Century Gothic", "Century Gothic"),
    ("wood-type", "Wood Type", "Wood Type", "Rockwell Condensed", "Rockwell"),
];

/// The colour schemes that belong to one built-in theme and are not in Word's list of colour schemes.
const OWN_PALETTES: &[Palette] = &[
    ("Office 2023", ["0E2841", "E8E8E8", "156082", "E97132", "196B24", "0F9ED5", "A02B93", "4EA72E", "467886", "96607D"]),
    ("Facet", ["2C3C43", "EBEBEB", "90C226", "54A021", "E6B91E", "E76618", "C42F1A", "918655", "99CA3C", "B9D181"]),
    ("Ion", ["1E5155", "B6DDE8", "B01513", "EA6312", "E6B729", "6AAC90", "54849A", "9E5E9B", "58C1BA", "9DE9E3"]),
    ("Ion Boardroom", ["3E3D2D", "DCD8DC", "B31166", "E33D6F", "E45F3C", "E9943A", "9B6BF2", "D53DD0", "5EAEFF", "AFAFAF"]),
    ("Organic", ["212121", "B2B2B2", "83992A", "3C9770", "44709D", "A23C33", "D97828", "DEB340", "6B9F25", "B26B02"]),
    ("Slice", ["212121", "DADADA", "052F61", "A50E82", "14967C", "6A9E1F", "E87D37", "C62324", "F49100", "85DFD0"]),
    ("Wisp", ["4E3B30", "FBEEC9", "A53010", "DE7E18", "9F8351", "728653", "92AA4C", "6AAC91", "F49100", "85DFD0"]),
    ("Basis", ["3F3F3F", "DADADA", "F8B323", "656A59", "46B2B5", "8CAA7E", "D36F68", "826276", "6B9F25", "B26B02"]),
    ("Berlin", ["5E5E5E", "E2DDD6", "F09415", "C1B56B", "4BAF73", "5AA6C0", "D17DF9", "FA7E5C", "FFDB4D", "B1B1B1"]),
    ("Circuit", ["B1E8FE", "1B2B3A", "9ACD4C", "FAA93A", "D35940", "B258D3", "63A0CC", "8AC4A7", "FFB25B", "FFE0BE"]),
    ("Damask", ["3B4550", "E8E3D6", "9EC544", "50BEA3", "4A9CBB", "A5A5A5", "D9964A", "C5BFA4", "6B9F25", "B26B02"]),
    ("Depth", ["2F5370", "DAE5ED", "41AEBD", "97E9D5", "A2CF49", "608F3D", "F4DE3A", "FCB11C", "6B9F25", "B26B02"]),
    ("Droplet", ["1C2E3F", "DBE5ED", "2FA3EE", "4BCAAD", "86C157", "D99C3F", "CE6633", "A35DD1", "6B9F25", "B26B02"]),
    ("Frame", ["3C4B57", "E0E3E6", "40BAD2", "FAB900", "90BB23", "EE7008", "1AB39F", "D5393D", "F49100", "85DFD0"]),
    ("Gallery", ["5A4A42", "E9DFD8", "B71E42", "DE478E", "BC72F0", "795FAF", "586EC9", "6892E0", "6B9F25", "B26B02"]),
    ("Main Event", ["4A4A48", "E4DCD0", "B80E0F", "7D6D5B", "E2A200", "5B9179", "3F7CA0", "814D4F", "6B9F25", "B26B02"]),
    ("Mesh", ["464646", "D8D8D8", "6F6F74", "92A9B9", "A7B789", "B9A489", "8D6374", "9B7362", "6B9F25", "B26B02"]),
    ("Parallax", ["2A3A4F", "E3E8EE", "2D84B0", "32A5CD", "6BCDD3", "B8C93F", "E8A03B", "D2603A", "6B9F25", "B26B02"]),
    ("Savon", ["2E4A5E", "DCE6EB", "1CADE4", "2683C6", "27CED7", "42BA97", "3E8853", "62A39F", "6B9F25", "B26B02"]),
    ("View", ["4A4A4A", "E6E6E6", "6F6F74", "92A9B9", "A7B789", "B9A489", "8D6374", "9B7362", "6B9F25", "B26B02"]),
    ("Vapor Trail", ["2A1F3D", "E0DCE6", "6A1F7C", "E1226E", "FF5A33", "FFC12C", "5CBA6E", "3D8FD1", "F49100", "85DFD0"]),
    ("Wood Type", ["4B3A2F", "EBE3D0", "D34817", "9B2D1F", "A28E6A", "956251", "918485", "855D5D", "CC9900", "96A9A9"]),
];

fn palette(name: &str) -> Option<&'static Palette> {
    COLOR_SCHEMES.iter().chain(OWN_PALETTES).find(|p| p.0 == name)
}

fn scheme_of(p: &Palette) -> ColorScheme {
    let mut colors = BTreeMap::new();
    colors.insert("dk1".to_string(), "000000".to_string());
    colors.insert("lt1".to_string(), "FFFFFF".to_string());
    for (slot, hex) in SLOTS[2..].iter().zip(p.1.iter()) {
        colors.insert((*slot).to_string(), (*hex).to_string());
    }
    ColorScheme { name: p.0.to_string(), colors }
}

/// Word's colour schemes (Design, Colors).
pub fn color_schemes() -> Vec<ColorScheme> {
    COLOR_SCHEMES.iter().map(scheme_of).collect()
}

/// Word's font schemes (Design, Fonts).
pub fn font_schemes() -> Vec<FontScheme> {
    FONT_SCHEMES.iter().map(|(n, a, b)| FontScheme { name: (*n).into(), major: (*a).into(), minor: (*b).into() }).collect()
}

/// Word's built-in themes (Design, Themes).
pub fn themes() -> Vec<ThemeDef> {
    THEMES
        .iter()
        .filter_map(|(id, name, colors, major, minor)| {
            let colors = scheme_of(palette(colors)?);
            Some(ThemeDef { id: (*id).into(), name: (*name).into(), colors: ColorScheme { name: (*name).into(), ..colors }, fonts: FontScheme { name: (*name).into(), major: (*major).into(), minor: (*minor).into() } })
        })
        .collect()
}

fn norm(s: &str) -> String {
    s.to_lowercase().replace([' ', '-', '_'], "")
}

/// A built-in theme by id or name (`facet`, `Ion Boardroom`).
pub fn find_theme(name: &str) -> Option<ThemeDef> {
    let n = norm(name);
    themes().into_iter().find(|t| norm(&t.id) == n || norm(&t.name) == n)
}

/// A built-in colour scheme by name (`Blue Warm`).
pub fn find_colors(name: &str) -> Option<ColorScheme> {
    let n = norm(name);
    color_schemes().into_iter().find(|c| norm(&c.name) == n)
}

/// A built-in font scheme by name (`Garamond`).
pub fn find_fonts(name: &str) -> Option<FontScheme> {
    let n = norm(name);
    font_schemes().into_iter().find(|c| norm(&c.name) == n)
}

fn hex6(v: &str) -> Option<String> {
    crate::colour::hex_hash(v)
}

impl ColorScheme {
    /// This scheme with the slots it leaves out taken from `base`; invalid colours are an error.
    pub fn complete(&self, base: &ColorScheme) -> Result<ColorScheme, String> {
        let mut colors = base.colors.clone();
        for (k, v) in &self.colors {
            if !SLOTS.contains(&k.as_str()) {
                return Err(format!("\"{k}\" is not a theme colour (use {})", SLOTS.join(", ")));
            }
            colors.insert(k.clone(), hex6(v).ok_or_else(|| format!("\"{v}\" is not a colour (six hex digits)"))?);
        }
        Ok(ColorScheme { name: if self.name.is_empty() { "Custom".into() } else { self.name.clone() }, colors })
    }
}

// ---------------------------------------------------------------------------
// The theme part
// ---------------------------------------------------------------------------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn clr_scheme_xml(c: &ColorScheme) -> String {
    let mut out = format!(r#"<a:clrScheme name="{}">"#, esc(&c.name));
    for slot in SLOTS {
        let hex = c.colors.get(slot).map(String::as_str).unwrap_or("000000");
        let inner = match (slot, hex) {
            ("dk1", "000000") => r#"<a:sysClr val="windowText" lastClr="000000"/>"#.to_string(),
            ("lt1", "FFFFFF") => r#"<a:sysClr val="window" lastClr="FFFFFF"/>"#.to_string(),
            _ => format!(r#"<a:srgbClr val="{hex}"/>"#),
        };
        out.push_str(&format!("<a:{slot}>{inner}</a:{slot}>"));
    }
    out.push_str("</a:clrScheme>");
    out
}

fn font_scheme_xml(f: &FontScheme) -> String {
    let side = |tag: &str, face: &str| format!(r#"<a:{tag}><a:latin typeface="{}"/><a:ea typeface=""/><a:cs typeface=""/></a:{tag}>"#, esc(face));
    format!(r#"<a:fontScheme name="{}">{}{}</a:fontScheme>"#, esc(&f.name), side("majorFont", &f.major), side("minorFont", &f.minor))
}

/// Office's format scheme: three fills, three lines, three effects and three background fills.
const FMT_SCHEME: &str = r#"<a:fmtScheme name="Office"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"><a:lumMod val="110000"/><a:satMod val="105000"/><a:tint val="67000"/></a:schemeClr></a:gs><a:gs pos="50000"><a:schemeClr val="phClr"><a:lumMod val="105000"/><a:satMod val="103000"/><a:tint val="73000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"><a:lumMod val="105000"/><a:satMod val="109000"/><a:tint val="81000"/></a:schemeClr></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"><a:satMod val="103000"/><a:lumMod val="102000"/><a:tint val="94000"/></a:schemeClr></a:gs><a:gs pos="50000"><a:schemeClr val="phClr"><a:satMod val="110000"/><a:lumMod val="100000"/><a:shade val="100000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"><a:lumMod val="99000"/><a:satMod val="120000"/><a:shade val="78000"/></a:schemeClr></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln><a:ln w="12700" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln><a:ln w="19050" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst><a:outerShdw blurRad="57150" dist="19050" dir="5400000" algn="ctr" rotWithShape="0"><a:srgbClr val="000000"><a:alpha val="63000"/></a:srgbClr></a:outerShdw></a:effectLst></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:tint val="95000"/><a:satMod val="170000"/></a:schemeClr></a:solidFill><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"><a:tint val="93000"/><a:satMod val="150000"/><a:shade val="98000"/><a:lumMod val="102000"/></a:schemeClr></a:gs><a:gs pos="50000"><a:schemeClr val="phClr"><a:tint val="98000"/><a:satMod val="130000"/><a:shade val="90000"/><a:lumMod val="103000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"><a:shade val="63000"/><a:satMod val="120000"/></a:schemeClr></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill></a:bgFillStyleLst></a:fmtScheme>"#;

/// A whole theme part, as Word writes one.
pub fn theme_xml(def: &ThemeDef) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="{NS_A}" name="{}"><a:themeElements>{}{}{}</a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#,
        esc(&def.name),
        clr_scheme_xml(&def.colors),
        font_scheme_xml(&def.fonts),
        FMT_SCHEME
    )
}

/// The Office theme (Calibri Light and Calibri), which new documents carry.
pub fn office_theme() -> ThemeDef {
    find_theme("office").expect("the Office theme is built in")
}

fn parse_fragment(xml_src: &str) -> Element {
    let doc = xml::parse(format!(r#"<x xmlns:a="{NS_A}">{xml_src}</x>"#).as_bytes()).expect("a built theme fragment is well formed");
    let el = doc.root.elements().next().cloned().expect("a fragment has an element");
    el
}

/// Put `new` where the element called `name` is inside `a:themeElements` (or at the end when it is missing).
fn put(root: &mut Element, name: &str, new: Element) {
    let Some(te) = root.child_mut("a:themeElements") else { return };
    match te.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == name)) {
        Some(i) => te.children[i] = Node::Element(new),
        None => te.children.push(Node::Element(new)),
    }
}

/// Change a theme document's colour scheme to `scheme` (slots it leaves out keep their colours).
pub fn apply_colors(root: &mut Element, scheme: &ColorScheme) -> Result<(), String> {
    let current = read_colors(root);
    let full = scheme.complete(&current)?;
    put(root, "a:clrScheme", parse_fragment(&clr_scheme_xml(&full)));
    Ok(())
}

/// Change a theme document's heading and body fonts. The script lists and the other fonts stay.
pub fn apply_fonts(root: &mut Element, fonts: &FontScheme) -> Result<(), String> {
    if fonts.major.trim().is_empty() || fonts.minor.trim().is_empty() {
        return Err("a font scheme needs a heading font and a body font".into());
    }
    let Some(te) = root.child_mut("a:themeElements") else { return Err("the theme has no theme elements".into()) };
    if te.child("a:fontScheme").is_none() {
        put(root, "a:fontScheme", parse_fragment(&font_scheme_xml(fonts)));
        return Ok(());
    }
    let fs = te.child_mut("a:fontScheme").unwrap();
    fs.set_attr("name", fonts.name.clone());
    for (tag, face) in [("a:majorFont", &fonts.major), ("a:minorFont", &fonts.minor)] {
        let Some(side) = fs.child_mut(tag) else { continue };
        match side.child_mut("a:latin") {
            Some(l) => {
                l.set_attr("typeface", face.clone());
                // The panose number describes the old face.
                l.remove_attr("panose");
            }
            None => side.children.insert(0, Node::Element(Element::new("a:latin").with_attr("typeface", face.clone()))),
        }
    }
    Ok(())
}

/// The name of a theme document.
pub fn set_name(root: &mut Element, name: &str) {
    root.set_attr("name", name.to_string());
}

/// A theme document made from nothing: Office's format scheme with the given colours and fonts.
pub fn new_theme_doc(def: &ThemeDef) -> XmlDoc {
    xml::parse(theme_xml(def).as_bytes()).expect("a built theme is well formed")
}

fn read_colors(root: &Element) -> ColorScheme {
    let mut s = ColorScheme::default();
    if let Some(c) = root.find("a:clrScheme") {
        s.name = c.attr("name").unwrap_or_default().to_string();
        for e in c.elements() {
            let hex = e.elements().next().and_then(|v| match v.name.as_str() {
                "a:srgbClr" => v.attr("val").and_then(hex6),
                "a:sysClr" => v.attr("lastClr").or(v.attr("val")).and_then(hex6),
                _ => None,
            });
            if let Some(h) = hex {
                s.colors.insert(e.local().to_string(), h);
            }
        }
    }
    // A slot the file leaves out reads as the Office colour.
    let office = scheme_of(&COLOR_SCHEMES[0]);
    for (k, v) in office.colors {
        s.colors.entry(k).or_insert(v);
    }
    s
}

/// What a theme document says about itself.
pub fn read_info(root: &Element) -> ThemeInfo {
    let face = |tag: &str| root.find(tag).and_then(|f| f.child("a:latin")).and_then(|l| l.attr("typeface")).unwrap_or_default().to_string();
    ThemeInfo {
        name: root.attr("name").unwrap_or_default().to_string(),
        colors: read_colors(root),
        fonts: FontScheme { name: root.find("a:fontScheme").and_then(|f| f.attr("name")).unwrap_or_default().to_string(), major: face("a:majorFont"), minor: face("a:minorFont") },
    }
}

/// Everything the Design tab offers, for the app to show (and for extensions to add to).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeLibrary {
    pub themes: Vec<ThemeDef>,
    pub color_schemes: Vec<ColorScheme>,
    pub font_schemes: Vec<FontScheme>,
}

pub fn library() -> ThemeLibrary {
    ThemeLibrary { themes: themes(), color_schemes: color_schemes(), font_schemes: font_schemes() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_builds_a_valid_part() {
        for t in themes() {
            let doc = xml::parse(theme_xml(&t).as_bytes()).unwrap_or_else(|e| panic!("{}: {e}", t.name));
            xml::check_namespaces(&doc).unwrap();
            let info = read_info(&doc.root);
            assert_eq!(info.name, t.name);
            assert_eq!(info.fonts.major, t.fonts.major);
            assert_eq!(info.colors.colors.len(), 12);
            assert_eq!(info.colors.colors, t.colors.colors);
        }
    }

    #[test]
    fn lookups_ignore_case_and_spacing() {
        assert_eq!(find_theme("ion boardroom").unwrap().id, "ion-boardroom");
        assert_eq!(find_theme("Vapor-Trail").unwrap().name, "Vapor Trail");
        assert_eq!(find_colors("blue warm").unwrap().colors["accent1"], "4A66AC");
        assert_eq!(find_fonts("Tw Cen MT Condensed").unwrap().major, "Tw Cen MT Condensed");
        assert!(find_theme("nope").is_none());
    }

    #[test]
    fn colors_and_fonts_change_alone() {
        let mut doc = new_theme_doc(&office_theme());
        let before = doc.root.find("a:fmtScheme").unwrap().to_xml();
        let mut scheme = ColorScheme { name: "Mine".into(), ..Default::default() };
        scheme.colors.insert("accent1".into(), "#ff0000".into());
        apply_colors(&mut doc.root, &scheme).unwrap();
        apply_fonts(&mut doc.root, &FontScheme { name: "Mine".into(), major: "Georgia".into(), minor: "Verdana".into() }).unwrap();
        let info = read_info(&doc.root);
        assert_eq!(info.colors.colors["accent1"], "FF0000");
        assert_eq!(info.colors.colors["accent2"], "ED7D31");
        assert_eq!(info.colors.name, "Mine");
        assert_eq!((info.fonts.major.as_str(), info.fonts.minor.as_str()), ("Georgia", "Verdana"));
        assert_eq!(doc.root.find("a:fmtScheme").unwrap().to_xml(), before);
        assert!(apply_colors(&mut doc.root, &ColorScheme { name: "x".into(), colors: [("accent1".to_string(), "red".to_string())].into() }).is_err());
    }
}
