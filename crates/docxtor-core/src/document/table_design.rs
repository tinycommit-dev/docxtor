//! The op for table designs: Word's built-in table styles and the options that switch parts of them on.

use super::*;
use crate::ops::TableLookPatch;
use crate::shapes::theme::ShapeTheme;
use crate::table_designs;
use crate::table_style::TableLook;

impl Document {
    /// Every design of the gallery drawn small with the given look, in the colours of this document's theme.
    pub fn table_design_previews(&self, look: &TableLook) -> Vec<table_designs::Preview> {
        table_designs::previews(look, &ShapeTheme::load(&self.pkg))
    }

    /// One design an extension offers, drawn small in the colours of this document's theme.
    pub fn table_design_preview(&self, xml: &str, look: &TableLook) -> Result<table_designs::Preview, String> {
        table_designs::preview_definition(xml, look, &ShapeTheme::load(&self.pkg))
    }

    pub(super) fn op_table_design(&mut self, at: NodeId, style: Option<&str>, definition: Option<&str>, patch: Option<&TableLookPatch>) -> Result<OpResult, EditError> {
        // The style first: a name that is not a design is an error before anything changes.
        let style_id = match (style, definition) {
            // A design an extension brought: the document keeps its own style of that id, else it gets this one.
            (_, Some(xml)) => {
                let (id, _) = table_designs::parse_definition(xml).map_err(EditError::Invalid)?;
                if self.styles.exists(&id) {
                    Some(id)
                } else {
                    self.styles.ensure_builtin("TableNormal");
                    Some(self.styles.insert_style_xml(&id, xml).ok_or_else(|| EditError::Invalid("could not add the table design".into()))?)
                }
            }
            (Some(name), None) => {
                let design = table_designs::find(name, &self.styles).ok_or_else(|| EditError::Invalid(format!("there is no table design called \"{name}\"")))?;
                // A style the document already has is used as it is; a built-in one is added as Word writes it.
                let id = if self.styles.exists(&design) {
                    design
                } else {
                    let theme = ShapeTheme::load(&self.pkg);
                    table_designs::ensure(&mut self.styles, &design, &theme).ok_or_else(|| EditError::Invalid(format!("could not add the table design \"{name}\"")))?
                };
                Some(id)
            }
            (None, None) => None,
        };
        self.touch(at);
        let found = with_table_body(&mut self.body, at, &mut |t: &mut Table, _, _| {
            let current = t.tblpr().and_then(TableLook::parse);
            // A table with no look starts from Word's default for a new table.
            let mut look = current.unwrap_or_default();
            let mut changed = current.is_none() && style_id.is_some();
            if let Some(p) = patch {
                for (value, slot) in [
                    (p.first_row, &mut look.first_row),
                    (p.last_row, &mut look.last_row),
                    (p.first_column, &mut look.first_col),
                    (p.last_column, &mut look.last_col),
                    (p.banded_rows, &mut look.h_band),
                    (p.banded_columns, &mut look.v_band),
                ] {
                    if let Some(v) = value {
                        *slot = v;
                        changed = true;
                    }
                }
            }
            crate::table::set_design(t, style_id.as_deref(), changed.then_some(look));
        });
        found.ok_or_else(|| EditError::Invalid(format!("paragraph {at} is not inside a table")))?;
        Ok(OpResult::default())
    }
}
