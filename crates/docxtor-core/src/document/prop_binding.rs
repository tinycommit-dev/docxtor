//! Content controls bound to document properties. Word binds the title, subject and author of a cover page to the
//! core properties (`docProps/core.xml`), the company to the extended properties (`docProps/app.xml`), and the
//! date and abstract to a custom XML part of its own (see `cover`). The first two are not custom XML parts, but
//! Word names them with fixed store ids, so a control's `w:dataBinding` can point at them like at any other node.
//! The document keeps both property parts in memory (with undo), so reading and writing them goes through the
//! document and not the package.

use super::*;

/// The store id Word gives the core properties (`dc:title`, `dc:creator`...).
pub const CORE_STORE: &str = "{6C3C8BC8-F283-45AE-878A-BAB7291924A1}";
/// The store id Word gives the extended properties (`Company`, `Manager`...).
pub const APP_STORE: &str = "{6668398D-A668-4E3E-A5EB-62B293D839F1}";

/// The prefix mappings Word writes for a binding to a core property (`ns0` is `dc`, `ns1` is `cp`).
pub const CORE_PREFIXES: &str = "xmlns:ns0='http://purl.org/dc/elements/1.1/' xmlns:ns1='http://schemas.openxmlformats.org/package/2006/metadata/core-properties' ";
/// The same for the extended properties.
pub const APP_PREFIXES: &str = "xmlns:ns0='http://schemas.openxmlformats.org/officeDocument/2006/extended-properties' ";

/// The local name a binding's XPath ends in (`/ns1:coreProperties[1]/ns0:title[1]` gives `title`).
fn last_name(xpath: &str) -> Option<&str> {
    let step = xpath.rsplit('/').find(|s| !s.is_empty() && !s.starts_with('@') && *s != "text()")?;
    let step = step.split('[').next()?;
    Some(step.rsplit(':').next().unwrap_or(step))
}

fn same_store(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b)
}

impl Document {
    /// The text of the node a control is bound to: a core or extended property (empty when the file has none), else
    /// a node of a custom XML part.
    pub(super) fn bound_get(&self, store: &str, xpath: &str, prefixes: &str) -> Option<String> {
        if same_store(store, CORE_STORE) {
            let local = last_name(xpath)?;
            if !CORE_NAMES.contains(&local) {
                return None;
            }
            return Some(self.core.as_ref().and_then(|c| c.root.elements().find(|e| e.local() == local)).map(|e| e.text()).unwrap_or_default());
        }
        if same_store(store, APP_STORE) {
            let local = last_name(xpath)?;
            if !APP_NAMES.contains(&local) {
                return None;
            }
            return Some(self.app.as_ref().and_then(|a| a.root.elements().find(|e| e.local() == local)).map(|e| e.text()).unwrap_or_default());
        }
        crate::binding::get(&self.pkg, store, xpath, prefixes)
    }

    /// Give the node a control is bound to a value. Returns whether the node changed.
    pub(super) fn bound_set(&mut self, store: &str, xpath: &str, prefixes: &str, value: &str) -> bool {
        if same_store(store, CORE_STORE) {
            let Some(local) = last_name(xpath).filter(|l| CORE_NAMES.contains(l)) else { return false };
            let core = self.core.get_or_insert_with(docprops::new_core_doc);
            let before = core.clone();
            let mut c = docprops::CoreChanges { exact: true, ..Default::default() };
            match local {
                "title" => c.title = Some(value),
                "subject" => c.subject = Some(value),
                "creator" => c.author = Some(value),
                "keywords" => c.keywords = Some(value),
                "description" => c.description = Some(value),
                "category" => c.category = Some(value),
                "contentStatus" => c.status = Some(value),
                _ => return false,
            }
            docprops::apply(core, &c);
            let changed = *core != before;
            if changed || self.pkg.get(&self.core_path).is_none() {
                self.core_dirty = true;
            }
            return changed;
        }
        if same_store(store, APP_STORE) {
            let Some(local) = last_name(xpath).filter(|l| APP_NAMES.contains(l)) else { return false };
            let app = self.app.get_or_insert_with(docmeta::new_app_doc);
            let before = app.clone();
            docmeta::set_app_value(app, local, value, true);
            let changed = *app != before;
            if changed || self.pkg.get(&self.app_path).is_none() {
                self.app_dirty = true;
            }
            return changed;
        }
        crate::binding::set(&mut self.pkg, store, xpath, prefixes, value)
    }

    /// After the properties change (Document details, the AI tools): the controls bound to them show the new values.
    pub(super) fn refresh_property_controls(&mut self) {
        if self.sync_bound_controls() {
            self.body_dirty = true;
        }
    }
}

/// The core properties a control can be bound to.
const CORE_NAMES: &[&str] = &["title", "subject", "creator", "keywords", "description", "category", "contentStatus"];
/// The extended properties a control can be bound to.
const APP_NAMES: &[&str] = &["Company", "Manager"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xpath_ends_in_the_property() {
        assert_eq!(last_name("/ns1:coreProperties[1]/ns0:title[1]"), Some("title"));
        assert_eq!(last_name("/ns0:Properties[1]/ns0:Company[1]/text()"), Some("Company"));
        assert_eq!(last_name("/ns0:a[1]/@x"), Some("a"));
    }
}
