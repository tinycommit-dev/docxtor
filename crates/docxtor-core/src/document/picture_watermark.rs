//! Picture watermarks: an image behind the text of every page, written as Word does (a VML picture shape
//! in each header, related to the image part from the header). Text watermarks are in `document.rs`.

use super::*;
use std::collections::HashSet;

impl Document {
    pub(super) fn op_set_picture_watermark(&mut self, media: &str, width: f64, height: f64, scale: Option<f64>, washout: bool) -> Result<OpResult, EditError> {
        if !self.pkg.has(media) {
            return Err(EditError::Invalid(format!("media '{media}' not found; add it first")));
        }
        if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
            return Err(EditError::Invalid("Give the picture's width and height in points.".into()));
        }
        if scale.is_some_and(|s| !(0.05..=10.0).contains(&s)) {
            return Err(EditError::Invalid("The scale is a number from 0.05 (5%) to 10 (1000%); leave it out for automatic.".into()));
        }
        let title = media.rsplit('/').next().unwrap_or(media).to_string();
        let uses = self.watermark_targets()?;
        let header_style = self.styles.ensure_builtin("Header").unwrap_or_else(|| "Header".into());
        for (serial, (part, area)) in uses.iter().enumerate() {
            let size = match scale {
                Some(s) => (width * s, height * s),
                // Its own size, shrunk (never enlarged) until it fits the text area.
                None => {
                    let k = (area.0 / width).min(area.1 / height).min(1.0);
                    (width * k, height * k)
                }
            };
            let rid = self.pkg.add_rel(part, package::REL_IMAGE, &relative_target(part, media), false);
            let pict = marks::picture_pict(&marks::PictureMark { rid: &rid, title: &title, size, washout, serial: serial as u32 });
            self.put_watermark_in(part, pict, &header_style, true);
        }
        self.stories_touched = true;
        Ok(OpResult::default())
    }

    /// What to leave out of the saved file: image relationships of watermark headers that the header no
    /// longer uses, and the images nothing else uses. Returns the parts to skip and the rewritten
    /// relationship files. The package keeps everything, so undo and redo still find their images.
    pub(super) fn unused_header_images(&self) -> (BTreeSet<String>, HashMap<String, Vec<u8>>) {
        let mut skip = BTreeSet::new();
        let mut replace: HashMap<String, Vec<u8>> = HashMap::new();
        let mut dropped: Vec<String> = Vec::new();
        for part in &self.watermark_headers {
            let Some(xml) = self.pkg.get(part).map(String::from_utf8_lossy) else { continue };
            let Ok(rels) = self.pkg.rels_for(part) else { continue };
            let unused: Vec<&package::Relationship> =
                rels.iter().filter(|r| r.rel_type == package::REL_IMAGE && !r.external && !xml.contains(&format!("\"{}\"", r.id))).collect();
            if unused.is_empty() {
                continue;
            }
            let path = package::rels_path(part);
            let Some(mut doc) = self.pkg.get(&path).and_then(|d| xml::parse(d).ok()) else { continue };
            doc.root.children.retain(|n| !matches!(n, Node::Element(e) if e.attr("Id").is_some_and(|id| unused.iter().any(|u| u.id == id))));
            dropped.extend(unused.iter().map(|u| package::resolve_target(part, &u.target)));
            if doc.root.elements().next().is_none() {
                skip.insert(path);
            } else {
                replace.insert(path, doc.to_bytes());
            }
        }
        if dropped.is_empty() {
            return (skip, replace);
        }
        // An image stays while any relationship, in the file as it will be saved, still points at it.
        let mut used: HashSet<String> = HashSet::new();
        for p in self.pkg.parts.iter().filter(|p| p.name.ends_with(".rels") && !skip.contains(&p.name)) {
            let data = replace.get(&p.name).unwrap_or(&p.data);
            let Ok(doc) = xml::parse(data) else { continue };
            let source = rels_source(&p.name);
            for e in doc.root.elements().filter(|e| e.local() == "Relationship" && e.attr("TargetMode") != Some("External")) {
                used.insert(package::resolve_target(&source, e.attr("Target").unwrap_or_default()));
            }
        }
        skip.extend(dropped.into_iter().filter(|m| !used.contains(m)));
        (skip, replace)
    }
}

/// The part a relationships file belongs to (`word/_rels/header1.xml.rels` is for `word/header1.xml`).
pub(super) fn rels_source(rels: &str) -> String {
    let name = rels.strip_suffix(".rels").unwrap_or(rels);
    match name.rsplit_once("/_rels/") {
        Some((dir, file)) => format!("{dir}/{file}"),
        None => name.strip_prefix("_rels/").filter(|f| !f.is_empty() && *f != ".rels").map(String::from).unwrap_or_default(),
    }
}
