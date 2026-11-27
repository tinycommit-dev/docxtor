//! What the document can tell the picture tools: a picture as the renderer sees it, and the theme's colours.

use super::*;
use crate::render::RImage;
use crate::shapes::theme::ShapeTheme;

impl Document {
    /// The theme's colours as `[name, hex]` pairs in Word's order (`dk1`, `lt1`, `dk2`, `lt2`, `accent1` to `accent6`,
    /// `hlink`, `folHlink`), for pickers that offer the document's own palette.
    pub fn theme_colors(&self) -> Vec<(String, String)> {
        let theme = ShapeTheme::load(&self.pkg);
        ["dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink"]
            .iter()
            .filter_map(|n| theme.scheme(n).map(|h| (n.to_string(), h.to_string())))
            .collect()
    }

    /// The picture whose object sits at a position (the object occupies `[o, o+1)`), with its size, placement and
    /// formatting, as the renderer sees it.
    pub fn picture_at(&self, at: Pos) -> Option<RImage> {
        let (blocks, part) = self.locate(at.p.0);
        let p = find_para(blocks, at.p.0)?;
        let e = super::shapes::object_at(&p.inlines, at.o)?;
        let labels = HashMap::new();
        let notes = self.note_state();
        let ctx = self.story_ctx(part, &labels, &notes);
        ctx.image(e)
    }

    /// A picture's media in a form a browser can show: TIFF as PNG, EMF and WMF as SVG. None when the browser shows it
    /// as it is, or when it can't be converted.
    pub fn media_display(&self, path: &str) -> Option<crate::imgconv::Display> {
        let bytes = self.pkg.get(path)?;
        crate::imgconv::display_form(path.rsplit('.').next().unwrap_or(""), bytes)
    }

    /// `media_display` for the browser bindings: one byte of kind then the bytes (1 PNG, 2 SVG), empty when the browser
    /// shows the media as it is, or `[0]` for a TIFF, EMF or WMF that could not be converted.
    pub fn media_display_bytes(&self, path: &str) -> Vec<u8> {
        match self.media_display(path) {
            Some(d) => std::iter::once(if d.mime == "image/png" { 1 } else { 2 }).chain(d.bytes).collect(),
            None => {
                let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
                if matches!(ext.as_str(), "tif" | "tiff" | "emf" | "wmf" | "emz" | "wmz") && self.pkg.get(path).is_some() {
                    vec![0]
                } else {
                    Vec::new()
                }
            }
        }
    }
}
