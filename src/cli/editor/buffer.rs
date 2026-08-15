//! What an editor buffer means, with no editor in sight.
//!
//! Split from [`super`] because these are string rules and nothing else — no process, no
//! temp file, no terminal — which is also why every one of them is a plain unit test.

/// Lines the editor buffer carries for the operator's benefit and the issue never keeps.
///
/// An HTML comment rather than a `#` line — `#` is a markdown heading, so git's
/// commit-message convention would put an H1 in every issue body. The marker is specific
/// enough that prose containing an ordinary HTML comment survives untouched.
const BANNER: &str = "<!-- trck: ";

/// Is this buffer a refusal rather than an answer?
///
/// Both forms mean "I changed my mind": nothing typed, or nothing changed. `git commit`
/// treats an empty message the same way, and for the same reason — the alternative is
/// filing whatever happened to be on screen.
pub(super) fn is_abort(buffer: &str, offered: &str) -> bool {
    strip_banner(buffer).trim().is_empty() || strip_banner(buffer) == strip_banner(offered)
}

/// The buffer as it would be filed: the operator's text, with our own annotations removed.
pub(super) fn strip_banner(buffer: &str) -> String {
    let kept: Vec<&str> = buffer.lines().filter(|l| !l.trim_start().starts_with(BANNER)).collect();
    let joined = kept.join("\n");
    format!("{}\n", joined.trim_start_matches('\n').trim_end())
}

/// What is wrong with this buffer, if anything.
///
/// One rule, narrowly drawn: a heading that is *there* has to be the issue's title. The
/// template offers one, so an operator who edits its text has almost certainly renamed the
/// issue by accident — `show` would then print a body contradicting its own metadata.
///
/// A buffer with no heading at all is fine, because `--body` writes exactly that and
/// `check` accepts it. Requiring one here and not there would be two rules for one thing.
pub(super) fn complaint(buffer: &str, title: &str) -> Option<String> {
    let body = strip_banner(buffer);
    let first = body.lines().find(|l| !l.trim().is_empty())?;
    let heading = first.strip_prefix("# ")?;
    (heading.trim() != title)
        .then(|| format!("the heading says {:?} but the issue is titled {title:?} — change the heading back, or drop it entirely", heading.trim()))
}

/// The buffer to re-open after a complaint: the annotation, then the operator's own text.
pub(super) fn annotated(buffer: &str, why: &str) -> String {
    format!("{BANNER}{why} -->\n{BANNER}this line and the one above are removed before the issue is filed -->\n{}", strip_banner(buffer))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn an_empty_or_unchanged_buffer_aborts() {
        let offered = "# T\n\n## Summary\n";
        assert!(is_abort("", offered), "empty");
        assert!(is_abort("   \n\n", offered), "whitespace only");
        assert!(is_abort(offered, offered), "unchanged");
        assert!(!is_abort("# T\n\n## Summary\nwrote something\n", offered), "edited");
    }

    /// The annotation is ours, not the operator's, so a buffer that is unchanged *except*
    /// for it still counts as unchanged.
    #[test]
    fn an_unchanged_buffer_is_still_unchanged_under_an_annotation() {
        let offered = "# T\n\n## Summary\n";
        assert!(is_abort(&annotated(offered, "something was wrong"), offered));
    }

    #[test]
    fn the_annotation_never_reaches_the_issue() {
        let buffer = annotated("# T\n\nprose\n", "the heading is wrong");
        assert!(buffer.contains("the heading is wrong"), "the complaint must be visible: {buffer}");
        assert_eq!(strip_banner(&buffer), "# T\n\nprose\n");
    }

    /// An ordinary HTML comment is prose. Only our own marker is ours to remove — the
    /// template itself is full of the former.
    #[test]
    fn an_ordinary_html_comment_survives() {
        let buffer = "# T\n\n<!-- a note the author wrote -->\n";
        assert!(strip_banner(buffer).contains("a note the author wrote"));
    }

    #[test]
    fn a_heading_that_disagrees_with_the_title_is_the_complaint() {
        let why = complaint("# Something else\n\nprose\n", "The Title").expect("complaint");
        assert!(why.contains("Something else"), "{why}");
        assert!(why.contains("The Title"), "{why}");
    }

    #[test]
    fn a_matching_heading_passes() {
        assert!(complaint("# The Title\n\nprose\n", "The Title").is_none());
    }

    /// `--body` writes prose with no heading and `check` accepts it. Requiring one here
    /// would be two rules for one thing.
    #[test]
    fn a_buffer_with_no_heading_passes() {
        assert!(complaint("just prose, no heading\n", "The Title").is_none());
        assert!(complaint("## Summary\n\nprose\n", "The Title").is_none(), "an H2 is not the title");
    }

    /// The complaint is about what will be filed, so the annotation must not be mistaken
    /// for the operator's first line.
    #[test]
    fn the_annotation_is_not_read_as_the_heading() {
        let buffer = annotated("# The Title\n\nprose\n", "an earlier complaint");
        assert!(complaint(&buffer, "The Title").is_none(), "the banner was read as content");
    }
}
