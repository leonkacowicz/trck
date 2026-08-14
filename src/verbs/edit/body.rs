//! What `set` does to an issue's markdown body.
//!
//! The row and the file have to agree: the filename carries the slug and the first line
//! carries the title, so an edit to either moves or rewrites the body. Both are *described*
//! here and applied by the backend, which is why the "before" has to be captured before the
//! row is edited — once the slug changes, the row no longer says where its own file is.

use super::super::{Edit, body_rel_path};
use super::set::SetOpts;
use crate::discovery::Ctx;
use crate::issue::Issue;
use std::path::PathBuf;

/// Where the body was, and what it said, before the edit.
///
/// `text` is read only when `--title` will rewrite it — and a body that is not there reads as
/// `None` rather than an error, because a vanished file is `check`'s business, not this
/// verb's.
pub(super) struct Before {
    path: PathBuf,
    text: Option<String>,
}

pub(super) fn before(ctx: &Ctx, rows: &[Issue], iid: &str, opts: &SetOpts) -> Result<Before, String> {
    let row = rows.iter().find(|r| r.id == iid).ok_or_else(|| format!("no issue matching '{iid}'"))?;
    Ok(Before { path: body_rel_path(row), text: opts.title.and_then(|_| ctx.read_body(row).ok()) })
}

/// Bring the body back in line with the row it belongs to: renamed when the slug moved,
/// re-headed when the title did.
///
/// The rename comes first and the rewrite addresses the *new* name, which is the order the
/// backend applies them in — a write to the old name would land on a file about to move away.
pub(super) fn edits(before: &Before, rows: &[Issue], iid: &str, title: Option<&str>) -> Vec<Edit> {
    let new = rows.iter().find(|r| r.id == iid).map_or_else(|| before.path.clone(), body_rel_path);
    let mut edits = Vec::new();
    if before.path != new {
        edits.push(Edit::Rename { from: before.path.clone(), to: new.clone() });
    }
    if let Some(title) = title
        && let Some(text) = &before.text
    {
        edits.push(Edit::Write { path: new, contents: retitled(text, title) });
    }
    edits
}

/// The body with its first heading rewritten, so the file does not contradict the index. Only
/// the first line, and only when it is a heading — the rest is hand-authored prose.
fn retitled(text: &str, title: &str) -> String {
    let rewritten: Vec<String> =
        text.lines().enumerate().map(|(i, line)| if i == 0 && line.starts_with("# ") { format!("# {title}") } else { line.to_string() }).collect();
    let mut body = rewritten.join("\n");
    if text.ends_with('\n') {
        body.push('\n');
    }
    body
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// A row whose body path is `items/<id>-<slug>.md`, parsed the way a real index is.
    fn rows(slug: &str) -> Vec<Issue> {
        crate::index::parse_index(
            &format!("{{\"id\": \"aaaaaaa\", \"slug\": \"{slug}\", \"title\": \"t\", \"status\": \"backlog\", \"priority\": \"medium\"}}\n"),
            "index.jsonl",
        )
        .expect("parses")
    }

    fn was(slug: &str, text: Option<&str>) -> Before {
        Before { path: PathBuf::from(format!("items/aaaaaaa-{slug}.md")), text: text.map(str::to_string) }
    }

    /// Only the first line, and only when it is a heading. The rest is hand-authored prose
    /// and a `#` further down is someone's section, not the title.
    #[test]
    fn retitling_rewrites_the_first_heading_and_nothing_else() {
        assert_eq!(retitled("# old\n\n## Summary\n# not a title\n", "new"), "# new\n\n## Summary\n# not a title\n");
    }

    /// A body that does not open with a heading is left alone rather than having one forced
    /// onto its first line.
    #[test]
    fn retitling_leaves_a_body_without_a_heading_alone() {
        assert_eq!(retitled("prose first\n# later\n", "new"), "prose first\n# later\n");
    }

    /// The trailing newline is preserved or not, as it was — otherwise every retitle would
    /// show up in a diff as a whitespace change to the last line.
    #[test]
    fn retitling_preserves_whether_the_body_ended_in_a_newline() {
        assert_eq!(retitled("# old", "new"), "# new");
        assert_eq!(retitled("# old\n", "new"), "# new\n");
    }

    /// A slug change moves the body; the rewrite that follows it must address the name the
    /// rename produced, because the backend applies them in this order.
    #[test]
    fn a_rename_precedes_the_rewrite_and_targets_the_new_name() {
        let edits = edits(&was("old", Some("# old\n")), &rows("new"), "aaaaaaa", Some("new title"));
        assert_eq!(
            edits,
            vec![
                Edit::Rename { from: PathBuf::from("items/aaaaaaa-old.md"), to: PathBuf::from("items/aaaaaaa-new.md") },
                Edit::Write { path: PathBuf::from("items/aaaaaaa-new.md"), contents: "# new title\n".into() },
            ]
        );
    }

    /// A title change with no slug change rewrites in place — no rename to make.
    #[test]
    fn a_retitle_without_a_move_is_a_write_alone() {
        let edits = edits(&was("same", Some("# old\n")), &rows("same"), "aaaaaaa", Some("new title"));
        assert_eq!(edits, vec![Edit::Write { path: PathBuf::from("items/aaaaaaa-same.md"), contents: "# new title\n".into() }]);
    }

    /// An edit that touches neither slug nor title leaves the body out of the changeset
    /// entirely — `set --priority` must not rewrite prose.
    #[test]
    fn an_edit_that_touches_neither_leaves_the_body_alone() {
        assert!(edits(&was("same", None), &rows("same"), "aaaaaaa", None).is_empty());
    }

    /// A vanished body is `check`'s business: `set --title --slug` still moves the row and
    /// emits no write for a file it could not read.
    #[test]
    fn a_missing_body_produces_no_write() {
        let edits = edits(&was("old", None), &rows("new"), "aaaaaaa", Some("new title"));
        assert_eq!(
            edits,
            vec![Edit::Rename { from: PathBuf::from("items/aaaaaaa-old.md"), to: PathBuf::from("items/aaaaaaa-new.md") }],
            "the rename still stands"
        );
    }
}
