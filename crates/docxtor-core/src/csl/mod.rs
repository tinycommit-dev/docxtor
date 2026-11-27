//! A CSL 1.0.2 processor of Docxtor's own: it reads a Citation Style Language style and formats citations and
//! bibliography entries from Docxtor's sources.
//!
//! What it covers: style info and the options that cascade, macros, the citation and bibliography layouts,
//! `text` (variable, macro, term, value), `names` (name, name-part, et-al, label, substitute), `date` and
//! `date-part`, `number`, `label`, `group` (left out when its variables are empty), `choose` (type, variable,
//! is-numeric, is-uncertain-date, locator, position, disambiguate), text case, fonts, quotes, affixes, strip
//! periods, sorting by variable or macro, citation numbers and their collapse into ranges, positions (first,
//! subsequent, ibid, ibid with locator) and year-suffix disambiguation.
//!
//! What it leaves out: other languages than English (terms come from a small en-US locale and the terms a
//! style defines), disambiguation by adding names or given names, collapsing author-date citations, the
//! `near-note` position, `cite-group-delimiter`, and variables Docxtor's source model doesn't hold (they are
//! empty). See `docs/extensions/index.md`.

mod eval;
mod locale;
mod model;
mod names;
mod process;
mod text;
mod vars;

pub use eval::Position;
pub use model::{Class, Style};
pub use process::{process, CiteItem, CiteSite, Entry, Output};
