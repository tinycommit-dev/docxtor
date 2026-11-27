//! Chart options: axis titles and scales, gridlines, series colours, data labels, legend, trendlines, error bars,
//! looks and palettes, combination charts, bubble, stock and surface charts. Each option is written the way Word
//! writes it (schema order, `c:style`, `colorsN.xml`), read back, undone, and leaves what Docxtor doesn't model alone.

use docxtor_core::chart::format::{AX_ORDER, DLBLS_ORDER, ERR_ORDER, GROUP_ORDER, SER_ORDER, SPPR_ORDER, TREND_ORDER};
use docxtor_core::chart::{self, ChartKind, ChartModel};
use docxtor_core::model::walk_paragraphs;
use docxtor_core::xml::Element;
use docxtor_core::{package::Package, Document, Pos};
use serde_json::{json, Value};

mod helpers;
mod titles_axes_and_legend;
mod colours_labels_and_trendlines;
mod looks_and_unknowns;
mod combination_and_bubble;
mod stock_surface_and_workbook;
mod word_written;

#[allow(unused_imports)]
pub(super) use helpers::*;
#[allow(unused_imports)]
pub(super) use titles_axes_and_legend::*;
#[allow(unused_imports)]
pub(super) use colours_labels_and_trendlines::*;
#[allow(unused_imports)]
pub(super) use looks_and_unknowns::*;
#[allow(unused_imports)]
pub(super) use combination_and_bubble::*;
#[allow(unused_imports)]
pub(super) use stock_surface_and_workbook::*;
#[allow(unused_imports)]
pub(super) use word_written::*;
