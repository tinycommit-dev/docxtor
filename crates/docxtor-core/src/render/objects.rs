//! Rendering drawn objects: images, charts and the labels of other objects.

use super::*;

impl<'a> RenderCtx<'a> {
    pub(super) fn object_label(&self, e: &Element) -> String {
        if e.find("c:chart").is_some() || crate::chart::chartex::is_chartex_drawing(e) {
            return "Chart".into();
        }
        if e.find("w:txbxContent").is_some() {
            let mut t = String::new();
            let mut ps = Vec::new();
            e.find_all("w:p", &mut ps);
            for p in ps.iter().take(6) {
                t.push_str(p.text().trim());
                t.push(' ');
            }
            return format!("Text box: {}", t.trim());
        }
        match e.name.as_str() {
            "w:footnoteReference" | "w:endnoteReference" => {
                let n = self.footnote_counter.get() + 1;
                self.footnote_counter.set(n);
                format!("^{n}")
            }
            "w:commentReference" => String::new(),
            "w:object" => "Embedded object".into(),
            "w:drawing" | "mc:AlternateContent" | "w:pict" => "Shape".into(),
            "w:ptab" => "\t".into(),
            other => other.trim_start_matches("w:").to_string(),
        }
    }

    /// A drawing that shows a chart: its size and placement, and the chart read from its part.
    pub(super) fn chart(&self, e: &Element) -> Option<(RImage, crate::chart::ChartModel)> {
        let rid = crate::chart::chart_rid(e)?;
        let rel = self.rels.get(rid)?;
        let model = crate::chart::read_part(self.pkg, &rel.target).unwrap_or(crate::chart::ChartModel { kind: crate::chart::ChartKind::Other, ..Default::default() });
        let container = e.find("wp:inline").or_else(|| e.find("wp:anchor"));
        let floating = container.is_some_and(|c| c.name == "wp:anchor");
        let emu = |a: &str| container.and_then(|c| c.child("wp:extent")).and_then(|x| x.attr(a)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        let alt = container.and_then(|c| c.child("wp:docPr")).and_then(|d| d.attr("descr")).filter(|s| !s.is_empty()).map(String::from);
        Some((RImage { src: rel.target.clone(), w: emu("cx") / 12700.0, h: emu("cy") / 12700.0, floating, alt, name: crate::shapes::object_props::docpr_name(e), layout: crate::image::layout(e), pic: None, place: crate::picture::read_place(e) }, model))
    }

    /// A drawing that shows one of the new chart types (`mc:AlternateContent` with a chartex choice and a picture
    /// as its fallback): its size and placement, and the chart read from its part. None when Docxtor can't draw it
    /// and the fallback picture can be shown instead.
    pub(super) fn chartex(&self, e: &Element) -> Option<(RImage, crate::chart::chartex::ChartExModel)> {
        use crate::chart::chartex;
        let rid = chartex::chartex_rid(e)?;
        let rel = self.rels.get(rid)?;
        let model = chartex::read_part(self.pkg, &rel.target);
        // The picture stands in for what Docxtor can't read or draw (a region map, a damaged part).
        let drawable = model.as_ref().is_some_and(|m| m.kind.drawable() && m.has_data());
        if !drawable && self.image(e).is_some() {
            return None;
        }
        // No picture to show: a placeholder with the title.
        let model = match model {
            Some(m) if drawable => m,
            Some(m) => chartex::ChartExModel { kind: if m.kind.drawable() { chartex::ExKind::Other } else { m.kind }, title: m.title, ..Default::default() },
            None => chartex::ChartExModel::default(),
        };
        let container = e.find("wp:inline").or_else(|| e.find("wp:anchor"));
        let floating = container.is_some_and(|c| c.name == "wp:anchor");
        let emu = |a: &str| container.and_then(|c| c.child("wp:extent")).and_then(|x| x.attr(a)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        let alt = container.and_then(|c| c.child("wp:docPr")).and_then(|d| d.attr("descr")).filter(|s| !s.is_empty()).map(String::from);
        Some((RImage { src: rel.target.clone(), w: emu("cx") / 12700.0, h: emu("cy") / 12700.0, floating, alt, name: crate::shapes::object_props::docpr_name(e), layout: crate::image::layout(e), pic: None, place: crate::picture::read_place(e) }, model))
    }

    pub fn image(&self, e: &Element) -> Option<RImage> {
        // DrawingML (w:drawing, possibly inside mc:AlternateContent).
        let blip = e.find("a:blip");
        if let Some(blip) = blip {
            let rid = blip.attr("r:embed").or(blip.attr("r:link"))?;
            let rel = self.rels.get(rid)?;
            let container = e.find("wp:inline").or_else(|| e.find("wp:anchor"));
            let floating = container.is_some_and(|c| c.name == "wp:anchor");
            let ext = container.and_then(|c| c.child("wp:extent")).or_else(|| e.find("a:ext"));
            let emu = |a: &str| ext.and_then(|x| x.attr(a)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let alt = container.and_then(|c| c.child("wp:docPr")).and_then(|d| d.attr("descr")).filter(|s| !s.is_empty()).map(String::from);
            let rel_of = |id: &str| self.rels.get(id).map(|r| (r.target.clone(), r.external));
            let mut fmt = crate::picture::read(e, self.shapes.theme(self.pkg), &rel_of);
            // A picture that is only linked has nothing in the file to show.
            if blip.attr("r:embed").is_none() || rel.external {
                fmt.link = Some(rel.target.clone());
            }
            return Some(RImage {
                src: rel.target.clone(),
                w: emu("cx") / 12700.0,
                h: emu("cy") / 12700.0,
                floating,
                alt,
                name: crate::shapes::object_props::docpr_name(e),
                layout: crate::image::layout(e),
                pic: (!fmt.is_empty()).then_some(fmt),
                place: crate::picture::read_place(e),
            });
        }
        // VML (w:pict / w:object).
        let data = e.find("v:imagedata")?;
        let rid = data.attr("r:id")?;
        let rel = self.rels.get(rid)?;
        let style = e.find("v:shape").and_then(|s| s.attr("style")).unwrap_or("");
        let dim = |k: &str| {
            style
                .split(';')
                .find_map(|kv| kv.trim().strip_prefix(&format!("{k}:")))
                .and_then(|v| v.trim().trim_end_matches("pt").parse::<f64>().ok())
                .unwrap_or(72.0)
        };
        let fmt = crate::picture::PicFmt { crop: crate::picture::read_vml_crop(data), ..Default::default() };
        Some(RImage {
            src: rel.target.clone(),
            w: dim("width"),
            h: dim("height"),
            floating: false,
            alt: None,
            name: None,
            layout: crate::image::ImageLayout { wrap: "inline".into(), ..Default::default() },
            pic: (!fmt.is_empty()).then_some(fmt),
            place: None,
        })
    }
}
