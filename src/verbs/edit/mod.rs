//! The verbs that change an issue: create, move, edit, label, depend.
//!
//! They share a shape — load, resolve the id, mutate, guard, `commit` — and the guards are
//! where the interest is. Everything that could leave the tracker inconsistent is checked
//! against the *candidate* state before anything is written, so a refusal leaves the files
//! exactly as they were.
//!
//! One file per verb, because the shape is all they share: each carries its own options type
//! and its own guard, and together they were the longest file in `src/`. [`body`] is the
//! exception — it is `set`'s, but the question it answers (what an edit does to the markdown
//! file) is separable from the question `set` answers (what it does to the row).

mod body;
mod edges;
mod mv;
mod new;
mod set;

pub(crate) use edges::{cmd_dep, cmd_label};
pub(crate) use mv::{MvOpts, cmd_mv};
pub(crate) use new::{NewOpts, cmd_new};
pub(crate) use set::{SetOpts, cmd_set};
