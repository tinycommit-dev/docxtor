//! Content controls and legacy form fields applied to a document: the ops, the clean-up that keeps placeholder
//! text and bound values right after any edit, and the check that locked controls stay as they are.

use super::*;
use crate::controls::{self, ControlOp, ControlProps, ControlRef, RControl};
use crate::forms::{self, FormOp};

mod bound;
mod ctl;
mod detached;
mod ops;
mod repeat;

pub(in crate::document) use bound::*;
use ctl::*;
use detached::*;
