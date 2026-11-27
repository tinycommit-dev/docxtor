//! Charts: Word's DrawingML charts read into a model the editor can draw, edited in place, and new ones
//! written the way Word writes them (a chart part, its embedded workbook, relationships and content types).
//!
//! A chart in a document is a `w:drawing` whose `a:graphicData` points (`c:chart r:id`) at
//! `word/charts/chartN.xml`; that part has its own relationships to the workbook (`word/embeddings/*.xlsx`)
//! and optional `colorsN.xml`/`styleN.xml` parts.

pub mod build;
pub mod chartex;
pub mod format;
pub mod model;
pub mod palette;
pub mod read;
pub mod write;
pub mod xlsx;
pub mod xlsx_patch;

pub use model::*;
pub use read::{read, theme_colors};
pub use write::{ChartEdit, SeriesIn};

use crate::package::{self, Package};
use crate::xml::Element;

pub const URI_CHART: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
pub const REL_CHART: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
pub const REL_PACKAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package";
pub const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
pub const CT_XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

/// Whether a drawing (or an `mc:AlternateContent` holding one) shows a chart.
pub fn is_chart_drawing(e: &Element) -> bool {
    chart_rid(e).is_some()
}

/// The relationship id of the chart a drawing shows.
pub fn chart_rid(drawing: &Element) -> Option<&str> {
    let mut all = Vec::new();
    drawing.find_all("a:graphicData", &mut all);
    all.into_iter().filter(|g| g.attr("uri") == Some(URI_CHART)).find_map(|g| g.child("c:chart")).and_then(|c| c.attr("r:id"))
}

/// The theme's colours (`accent1`, `dk1`, ...), found through the main document's relationships.
pub fn theme_for(pkg: &Package) -> Vec<(String, String)> {
    let main = pkg.main_document_path();
    let Some(rel) = pkg.rels_for(&main).ok().and_then(|r| r.into_iter().find(|r| r.rel_type == package::REL_THEME)) else { return Vec::new() };
    let path = package::resolve_target(&main, &rel.target);
    pkg.get(&path).map(theme_colors).unwrap_or_default()
}

/// A chart part read into a model (with the palette of its `colorsN.xml`, when it has one).
pub fn read_part(pkg: &Package, part: &str) -> Option<ChartModel> {
    let bytes = pkg.get(part)?;
    let mut m = read(bytes, &theme_for(pkg))?;
    m.palette = palette_of(pkg, part);
    Some(m)
}

pub const REL_COLORS: &str = "http://schemas.microsoft.com/office/2011/relationships/chartColorStyle";
pub const CT_COLORS: &str = "application/vnd.ms-office.chartcolorstyle+xml";

/// The palette a chart's `colorsN.xml` names.
pub fn palette_of(pkg: &Package, chart_part: &str) -> Option<String> {
    let rel = pkg.rels_for(chart_part).ok()?.into_iter().find(|r| r.rel_type == REL_COLORS && !r.external)?;
    let doc = pkg.xml(&package::resolve_target(chart_part, &rel.target)).ok()?;
    palette::palette_from_number(doc.root.attr("id")?.parse().ok()?)
}

/// Write (or replace) the chart's `colorsN.xml` for a palette, as Word does when colours are changed.
pub fn write_colors_part(pkg: &mut Package, chart_part: &str, palette_id: &str) {
    let Some(xml) = palette::colors_part_xml(palette_id) else { return };
    let existing = pkg.rels_for(chart_part).ok().and_then(|r| r.into_iter().find(|r| r.rel_type == REL_COLORS && !r.external));
    match existing {
        Some(rel) => pkg.set(&package::resolve_target(chart_part, &rel.target), xml.into_bytes()),
        None => {
            let name = free_name(pkg, "word/charts", "colors", "xml");
            pkg.add_rel(chart_part, REL_COLORS, name.rsplit('/').next().unwrap_or("colors1.xml"), false);
            pkg.ensure_override(&name, CT_COLORS);
            pkg.set(&name, xml.into_bytes());
        }
    }
}

/// Where a chart's data lives.
#[derive(Debug, Clone, PartialEq)]
pub enum Workbook {
    /// An embedded workbook: the part name and its relationship id.
    Embedded { part: String, rid: String },
    /// A link to a file elsewhere (Word can't be asked to change it from here).
    Linked,
    None,
}

pub fn workbook_of(pkg: &Package, chart_part: &str) -> Workbook {
    let rels = pkg.rels_for(chart_part).unwrap_or_default();
    let doc = pkg.xml(chart_part).ok();
    let rid = doc.as_ref().and_then(|d| d.root.child("c:externalData").and_then(|e| e.attr("r:id").map(String::from)));
    let rel = match &rid {
        Some(id) => rels.iter().find(|r| &r.id == id),
        None => rels.iter().find(|r| r.rel_type == REL_PACKAGE),
    };
    match rel {
        Some(r) if r.external => Workbook::Linked,
        Some(r) => Workbook::Embedded { part: package::resolve_target(chart_part, &r.target), rid: r.id.clone() },
        None => Workbook::None,
    }
}

/// The first free `word/<dir>/<stem>N.<ext>`.
pub fn free_name(pkg: &Package, dir: &str, stem: &str, ext: &str) -> String {
    let mut n = 1;
    loop {
        let name = format!("{dir}/{stem}{n}.{ext}");
        if !pkg.has(&name) {
            return name;
        }
        n += 1;
    }
}
