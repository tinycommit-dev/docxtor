//! Word shapes and text boxes.
//!
//! A shape is an inline object in a paragraph (`w:drawing` with `wps:wsp`, `wpg:wgp` or `wpc:wpc`, usually
//! inside `mc:AlternateContent` next to a VML fallback, or VML alone in `w:pict`). The model keeps the
//! element exactly as it was read; this module reads it into the view model (`RShape`), turns the text
//! inside it into editable *stories* (see `story`), builds new shapes as Word writes them (`build`) and
//! edits the markup of existing ones in place (`patch`).

pub mod ai;
pub mod appearance;
pub mod body;
pub mod build;
pub mod connector;
pub mod freeform;
pub mod effects;
pub mod fill;
pub mod group;
pub mod member;
pub mod object_props;
pub mod order;
pub mod patch;
pub mod stack_move;
pub mod read;
pub mod smartart;
pub mod smartart_layout;
pub mod warp;
pub mod story;
pub mod theme;

use crate::image::ImageLayout;
use crate::model::Block;
use crate::render::RBlock;
use crate::xml::Element;
pub use appearance::{BodyPatch, EffectsSpec, FillSpec, REffects, RStop, RTile, Rect4};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

pub const EMU_PER_PT: f64 = 12700.0;

/// A shape as the renderer draws it. Sizes and offsets are points; angles are degrees.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RShape {
    /// The drawing's id (`wp:docPr`), 0 when the markup has none. Operations address a shape by the position
    /// of its object in the text (`{p, o}`), not by this id.
    pub id: u32,
    /// `shape`, `group`, `canvas` or `picture` (a picture inside a group or canvas).
    pub kind: &'static str,
    /// Where the markup came from: `drawingml` (`wps`) or `vml` (`v:`).
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    /// A preset name (`rect`, `roundRect`, `ellipse`, `line`, `straightConnector1`, `triangle`, `rightArrow`,
    /// ...), or `custom` when the shape has its own path (see `custom`).
    pub geometry: String,
    /// The preset's adjust values (`adj`: 16667), in the file's units (100000ths for DrawingML).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub adj: BTreeMap<String, f64>,
    /// Paths of a `custom` shape, simplified to SVG path data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<RCustom>,
    pub w: f64,
    pub h: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub rot: f64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub flip_h: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub flip_v: bool,
    /// Absent when the shape has no fill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<RFill>,
    /// Absent when the shape has no outline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<RLine>,
    /// A text box (`txBox="1"`, or a VML text box shape).
    pub text_box: bool,
    /// The blocks of the text inside, with their paragraph ids: edit them with the ordinary text ops.
    /// Absent when the shape has no text frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Vec<RBlock>>,
    pub insets: RInsets,
    /// `top`, `middle` or `bottom`.
    pub v_anchor: &'static str,
    /// Text wraps inside the shape (false: one line, as `wrap="none"`).
    pub wrap_text: bool,
    /// The shape grows to fit its text.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub auto_fit: bool,
    /// Vertical or rotated text (`vert270`, `eaVert`, ...); absent for horizontal text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vert: Option<String>,
    /// How the text frame fits its text: `shape` (`spAutoFit`) or `text` (`normAutofit`); absent for none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_fit_mode: Option<&'static str>,
    /// With `normAutofit`: the font scale (percent) and the line spacing cut (percent) the file says.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing_reduction: Option<f64>,
    /// The text block is centred in the shape as a whole (`anchorCtr`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub anchor_center: bool,
    /// Text columns (`numCol`) and the gap between them in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_gap_pt: Option<f64>,
    /// The text stays upright when the shape turns (`upright`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub upright: bool,
    /// Shadow, glow, reflection, soft edges, bevel and extrusion; absent when there are none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects: Option<REffects>,
    /// How the shape sits in the text: inline, or floating with its wrap and position.
    #[serde(flatten)]
    pub layout: ImageLayout,
    /// For a picture inside a group or canvas: its part name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    /// Members of a group or canvas, back to front.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<RShape>,
    /// For a member of a group or canvas: its position inside the parent's box (points, from its top-left
    /// corner), already scaled to the parent's size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_y: Option<f64>,
    /// For a connector glued to shapes: what its ends are glued to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connect: Option<connector::RConnect>,
    /// SmartArt (`kind` is `diagram`): where its shapes (`children`) came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagram: Option<smartart::RDiagram>,
    /// A member of a diagram: its text, drawn read only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtext: Option<smartart::RDText>,
    /// WordArt: the text bent to a preset (the shape's own text, or for VML WordArt the warp's `text`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warp: Option<warp::RWarp>,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RFill {
    /// `solid`, `gradient`, `picture` or `pattern`.
    pub kind: &'static str,
    /// Hex colour without `#` (a gradient's first stop).
    pub color: String,
    /// 0 to 1; absent when opaque.
    #[serde(skip_serializing_if = "is_one")]
    pub alpha: f64,
    /// A gradient's last stop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color2: Option<String>,
    /// A linear gradient's direction in degrees (0: left to right, 90: top to bottom).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    /// A gradient's stops, all of them (`color` and `color2` are the first and last).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stops: Vec<RStop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaled: Option<bool>,
    /// A path gradient: `circle`, `rect` or `shape`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_to: Option<Rect4>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rot_with_shape: Option<bool>,
    /// A picture fill: the media part name (`Engine.mediaUrl` gives the URL).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stretch: Option<Rect4>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop: Option<Rect4>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tile: Option<RTile>,
    /// A pattern fill's DrawingML preset (`pct5`, `ltDnDiag`, ...); `color` is the foreground, `color2` the background.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

impl RFill {
    /// A fill of `kind` with nothing else set.
    pub fn new(kind: &'static str, color: String, alpha: f64) -> RFill {
        RFill { kind, color, alpha, color2: None, angle: None, stops: vec![], scaled: None, path: None, fill_to: None, rot_with_shape: None, src: None, stretch: None, crop: None, tile: None, pattern: None }
    }

    pub fn solid(color: String, alpha: f64) -> RFill {
        RFill::new("solid", color, alpha)
    }
}

fn is_one(v: &f64) -> bool {
    (*v - 1.0).abs() < 1e-9
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RLine {
    pub color: String,
    #[serde(skip_serializing_if = "is_one")]
    pub alpha: f64,
    pub width_pt: f64,
    /// `solid`, `dot`, `dash`, `lgDash`, `dashDot`, ... (DrawingML's names).
    pub dash: String,
    /// Arrow at the start of an open line (`triangle`, `stealth`, `diamond`, `oval`, `arrow`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_arrow: Option<String>,
    /// Arrow at the end of an open line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail_arrow: Option<String>,
    /// How the ends of the line are finished (`round`, `square`; absent for flat).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap: Option<&'static str>,
    /// How corners are joined (`round`, `bevel`; absent for mitred).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join: Option<&'static str>,
    /// `dbl`, `thickThin`, `thinThick` or `tri`; absent for a single line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compound: Option<String>,
    /// Arrowhead width and length at the start and the end: `sm`, `med` or `lg`; absent for medium.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_w: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_len: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail_w: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail_len: Option<&'static str>,
    /// The miter limit in percent (800 is Word's default); absent for the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub miter: Option<f64>,
}

/// Text frame insets in points.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct RInsets {
    pub l: f64,
    pub t: f64,
    pub r: f64,
    pub b: f64,
}

impl Default for RInsets {
    fn default() -> Self {
        RInsets { l: 7.2, t: 3.6, r: 7.2, b: 3.6 }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RCustom {
    pub paths: Vec<RPath>,
}

/// One path of a custom shape. `d` is SVG path data in a `w` by `h` coordinate space.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RPath {
    pub w: f64,
    pub h: f64,
    pub d: String,
    /// The same path as structured commands in the same space (`M`, `L`, `C` and `Z`; arcs and quadratic curves
    /// are cubic curves here): what "Edit points" works on.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cmds: Vec<RCmd>,
    pub fill: bool,
    pub stroke: bool,
}

/// A path command of a custom shape: `M`, `L`, `C` (two handles and an end point) or `Z`.
#[derive(Debug, Clone, Serialize)]
pub struct RCmd {
    pub t: &'static str,
    pub pts: Vec<[f64; 2]>,
}

impl RCmd {
    pub fn new(t: &'static str, pts: &[(f64, f64)]) -> RCmd {
        RCmd { t, pts: pts.iter().map(|p| [p.0, p.1]).collect() }
    }
}

/// What rendering shapes needs besides the document: the text of each text box, and the theme.
#[derive(Default)]
pub struct ShapeEnv<'a> {
    /// Story blocks of the text boxes by key (see `story`).
    pub texts: HashMap<String, &'a [Block]>,
    theme: std::cell::OnceCell<theme::ShapeTheme>,
    /// The document's theme as it keeps it, read once each time the theme changes (`Document::shape_theme`). Without
    /// it the part is read on the first use in each render: on every keystroke, as the paragraph is drawn again.
    pub known_theme: Option<&'a theme::ShapeTheme>,
}

impl ShapeEnv<'_> {
    pub fn theme(&self, pkg: &crate::package::Package) -> &theme::ShapeTheme {
        if let Some(t) = self.known_theme {
            return t;
        }
        self.theme.get_or_init(|| theme::ShapeTheme::load(pkg))
    }
}

// ---------------------------------------------------------------------------
// Ops
// ---------------------------------------------------------------------------

/// An outline in an op: `"none"`, or `{color, width_pt, dash, head_arrow, tail_arrow}` (all optional).
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum LineSpec {
    None,
    Set(LineProps),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LineProps {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_pt: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<String>,
    /// `none`, `triangle`, `stealth`, `diamond`, `oval` or `arrow`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_arrow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_arrow: Option<String>,
    /// 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<f64>,
    /// `single`, `dbl`, `thickThin`, `thinThick` or `tri`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compound: Option<String>,
    /// `flat`, `round` or `square`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap: Option<String>,
    /// `miter`, `round` or `bevel`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join: Option<String>,
    /// Miter limit in percent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub miter: Option<f64>,
    /// Arrowhead sizes: `sm`, `med` or `lg`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_w: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_len: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_w: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_len: Option<String>,
}

impl Serialize for LineSpec {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            LineSpec::None => s.serialize_str("none"),
            LineSpec::Set(p) => p.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for LineSpec {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match &v {
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("none") => Ok(LineSpec::None),
            serde_json::Value::Object(_) => serde_json::from_value(v).map(LineSpec::Set).map_err(serde::de::Error::custom),
            _ => Err(serde::de::Error::custom("line must be \"none\" or {color, width_pt}")),
        }
    }
}

/// Whether an object element is a shape this module reads (a DrawingML shape, group or canvas, or a VML shape
/// that is neither a picture nor WordArt nor a watermark).
pub fn is_shape(e: &Element) -> bool {
    read::primary(e).is_some()
}

/// A hex colour as six uppercase digits (`#ff4f00`, `FF4F00` or a VML name), or None.
pub fn hex6(v: &str) -> Option<String> {
    // Six hex digits, or a name VML knows (`silver`) and the three-digit shorthand: all of it ends as `RRGGBB` or as nothing.
    crate::colour::hex_hash(v).or_else(|| crate::marks::colour_hex(v))
}
