//! How a shape looks beyond a flat colour and a plain line: the view model for fills (gradients, pictures,
//! patterns), effects (shadow, glow, soft edges, reflection, bevel, extrusion), and the op inputs that change
//! them. Reading is in `fill` and `effects`, writing in `fill` and `effects` too; `body` handles the text frame.

use serde::{Deserialize, Serialize};

pub(super) fn is_one(v: &f64) -> bool {
    (*v - 1.0).abs() < 1e-9
}

// ---------------------------------------------------------------------------
// View model
// ---------------------------------------------------------------------------

/// One stop of a gradient. `pos` is 0 to 100, `alpha` 0 to 1.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RStop {
    pub pos: f64,
    pub color: String,
    #[serde(skip_serializing_if = "is_one")]
    pub alpha: f64,
}

/// Four percentages (0 to 100, negative or above 100 allowed) from the left, top, right and bottom edges.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct Rect4 {
    #[serde(default)]
    pub l: f64,
    #[serde(default)]
    pub t: f64,
    #[serde(default)]
    pub r: f64,
    #[serde(default)]
    pub b: f64,
}

/// A tiled picture fill. `tx` and `ty` are offsets in points, `sx` and `sy` scales in percent.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RTile {
    pub tx: f64,
    pub ty: f64,
    pub sx: f64,
    pub sy: f64,
    /// `tl`, `t`, `tr`, `l`, `ctr`, `r`, `bl`, `b` or `br`.
    pub align: String,
    /// `none`, `x`, `y` or `xy`.
    pub flip: String,
}

/// A shadow, inner shadow or similar. Distances are points, `angle` degrees clockwise from the right.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RShadow {
    pub color: String,
    #[serde(skip_serializing_if = "is_one")]
    pub alpha: f64,
    pub blur_pt: f64,
    pub dist_pt: f64,
    pub angle: f64,
    /// Scale in percent (an outer shadow only); absent for 100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sx: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sy: Option<f64>,
    /// Where the scaled shadow stays anchored (`b`, `ctr`, `tl`, ...); absent for the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,    /// Whether the shadow turns with the shape (`rotWithShape`); absent when the file does not say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rot_with_shape: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RGlow {
    pub color: String,
    #[serde(skip_serializing_if = "is_one")]
    pub alpha: f64,
    pub radius_pt: f64,
}

/// A reflection below the shape. Alphas are 0 to 1, positions percent, directions degrees.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RReflection {
    pub blur_pt: f64,
    pub start_alpha: f64,
    pub end_alpha: f64,
    pub dist_pt: f64,
    pub direction: f64,
    pub fade_direction: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sx: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_pos: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_pos: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rot_with_shape: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RBevel {
    /// DrawingML's bevel preset: `circle`, `relaxedInset`, `cross`, `coolSlant`, `angle`, `softRound`, `convex`,
    /// `slope`, `divot`, `riblet`, `hardEdge`, `artDeco`.
    pub preset: String,
    pub w_pt: f64,
    pub h_pt: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RExtrusion {
    pub height_pt: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Where the depth points, as a camera preset or rotation reads: a unit step `[dx, dy]` per point of depth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<[f64; 2]>,
}

/// The effects of a shape: its own `a:effectLst` (and 3-D), else those of the theme style it refers to.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct REffects {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<RShadow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inner_shadow: Option<RShadow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glow: Option<RGlow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_edge_pt: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflection: Option<RReflection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bevel: Option<RBevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extrusion: Option<RExtrusion>,
}

impl REffects {
    pub fn is_empty(&self) -> bool {
        *self == REffects::default()
    }
}

// ---------------------------------------------------------------------------
// Op inputs
// ---------------------------------------------------------------------------

/// A value that an op can set or take away (`"none"` or `null` removes it).
#[derive(Debug, Clone, PartialEq)]
pub enum Opt<T> {
    Remove,
    Set(T),
}

impl<T: Serialize> Serialize for Opt<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Opt::Remove => s.serialize_str("none"),
            Opt::Set(v) => v.serialize(s),
        }
    }
}

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for Opt<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match &v {
            serde_json::Value::Null => Ok(Opt::Remove),
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("none") => Ok(Opt::Remove),
            _ => serde_json::from_value(v).map(Opt::Set).map_err(serde::de::Error::custom),
        }
    }
}

/// For a field of type `Option<Opt<T>>`: a key that is present (even as `null`) is `Some`.
fn opt_field<'de, D: serde::Deserializer<'de>, T: serde::de::DeserializeOwned>(d: D) -> Result<Option<Opt<T>>, D::Error> {
    Opt::<T>::deserialize(d).map(Some)
}

/// One gradient stop in an op. `pos` is 0 to 100.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StopIn {
    pub pos: f64,
    pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileIn {
    #[serde(default)]
    pub tx: f64,
    #[serde(default)]
    pub ty: f64,
    #[serde(default = "hundred")]
    pub sx: f64,
    #[serde(default = "hundred")]
    pub sy: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip: Option<String>,
}

fn hundred() -> f64 {
    100.0
}

/// A fill in an op, as `fill` takes it: `"none"`, a hex colour, or an object with a `kind`.
#[derive(Debug, Clone, PartialEq)]
pub enum FillSpec {
    None,
    /// A plain hex colour (opaque).
    Hex(String),
    Obj(FillObj),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FillObj {
    Solid {
        color: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alpha: Option<f64>,
    },
    Gradient {
        stops: Vec<StopIn>,
        /// Linear direction in degrees (0 left to right, 90 top to bottom).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        angle: Option<f64>,
        /// `circle`, `rect` or `shape` for a path gradient.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill_to: Option<Rect4>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scaled: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rot_with_shape: Option<bool>,
    },
    Picture {
        /// The name `engine.addMedia` returned.
        media: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tile: Option<TileIn>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stretch: Option<Rect4>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        crop: Option<Rect4>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alpha: Option<f64>,
    },
    Pattern {
        pattern: String,
        fg: String,
        bg: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alpha: Option<f64>,
    },
}

impl Serialize for FillSpec {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            FillSpec::None => s.serialize_str("none"),
            FillSpec::Hex(h) => s.serialize_str(h),
            FillSpec::Obj(o) => o.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for FillSpec {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match &v {
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("none") => Ok(FillSpec::None),
            serde_json::Value::String(s) => Ok(FillSpec::Hex(s.clone())),
            serde_json::Value::Object(_) => serde_json::from_value(v).map(FillSpec::Obj).map_err(serde::de::Error::custom),
            _ => Err(serde::de::Error::custom("fill must be \"none\", a hex colour or {kind, ...}")),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShadowIn {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur_pt: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dist_pt: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sx: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sy: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GlowIn {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius_pt: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReflectionIn {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur_pt: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_alpha: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_alpha: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dist_pt: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_direction: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sx: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sy: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_pos: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_pos: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BevelIn {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w_pt: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_pt: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExtrusionIn {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_pt: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// The parts of `effects` to change; a part left out stays, `"none"` removes it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EffectsIn {
    #[serde(default, deserialize_with = "opt_field", skip_serializing_if = "Option::is_none")]
    pub shadow: Option<Opt<ShadowIn>>,
    #[serde(default, deserialize_with = "opt_field", skip_serializing_if = "Option::is_none")]
    pub inner_shadow: Option<Opt<ShadowIn>>,
    #[serde(default, deserialize_with = "opt_field", skip_serializing_if = "Option::is_none")]
    pub glow: Option<Opt<GlowIn>>,
    #[serde(default, deserialize_with = "opt_field", skip_serializing_if = "Option::is_none")]
    pub soft_edge_pt: Option<Opt<f64>>,
    #[serde(default, deserialize_with = "opt_field", skip_serializing_if = "Option::is_none")]
    pub reflection: Option<Opt<ReflectionIn>>,
    #[serde(default, deserialize_with = "opt_field", skip_serializing_if = "Option::is_none")]
    pub bevel: Option<Opt<BevelIn>>,
    #[serde(default, deserialize_with = "opt_field", skip_serializing_if = "Option::is_none")]
    pub extrusion: Option<Opt<ExtrusionIn>>,
}

/// `effects` in an op: `"none"` removes every effect, else the parts to change.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum EffectsSpec {
    None,
    Set(EffectsIn),
}

impl Serialize for EffectsSpec {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            EffectsSpec::None => s.serialize_str("none"),
            EffectsSpec::Set(e) => e.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for EffectsSpec {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match &v {
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("none") => Ok(EffectsSpec::None),
            serde_json::Value::Object(_) => serde_json::from_value(v).map(EffectsSpec::Set).map_err(serde::de::Error::custom),
            _ => Err(serde::de::Error::custom("effects must be \"none\" or an object")),
        }
    }
}

/// The text frame's settings in an op (`bodyPr`); fields left out stay.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BodyPatch {
    /// Insets in points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ins_l: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ins_t: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ins_r: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ins_b: Option<f64>,
    /// `top`, `middle` or `bottom`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v_anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_center: Option<bool>,
    /// `none`, `shape` (resize the shape to fit the text) or `text` (shrink the text on overflow).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_fit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap_text: Option<bool>,
    /// `horz`, `vert`, `vert270`, `eaVert`, `wordArtVert` or `mongolianVert`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vert: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_gap_pt: Option<f64>,
    /// Text stays upright when the shape is rotated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upright: Option<bool>,
    /// With `auto_fit: "text"`: the font scale in percent (100 = none) and the line spacing cut in percent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_spacing_reduction: Option<f64>,
}

/// What a shape looks like beyond a flat colour, in words, for the AI's reading of the document:
/// `gradient fill, shadow, glow`. Empty for a plain shape.
pub fn looks_of(sh: &super::RShape) -> String {
    let mut v: Vec<&str> = Vec::new();
    match sh.fill.as_ref().map(|f| f.kind) {
        Some("gradient") => v.push("gradient fill"),
        Some("picture") => v.push("picture fill"),
        Some("pattern") => v.push("pattern fill"),
        _ => {}
    }
    if let Some(e) = &sh.effects {
        let named = [
            (e.shadow.is_some(), "shadow"),
            (e.inner_shadow.is_some(), "inner shadow"),
            (e.glow.is_some(), "glow"),
            (e.soft_edge_pt.is_some(), "soft edges"),
            (e.reflection.is_some(), "reflection"),
            (e.bevel.is_some(), "bevel"),
            (e.extrusion.is_some(), "3-D depth"),
        ];
        for (on, name) in named {
            if on {
                v.push(name);
            }
        }
    }
    v.join(", ")
}
