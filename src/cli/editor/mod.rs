//! Handing the operator a template to fill in, the way `visudo` does.
//!
//! Edit a temp copy, check it before accepting, and on a bad answer re-open the buffer with
//! the complaint at the top rather than throwing the work away. An empty or unmodified
//! buffer aborts — which is the whole reason `--empty` exists, since a deliberately
//! title-only issue needs a way to say so that is not "I changed nothing".
//!
//! visudo's lock does not transfer. Contention here is on a remote ref and the write path's
//! retry is the equivalent, with no stale lock to clean up afterwards.

pub(super) mod buffer;
mod scratch;

use buffer::{annotated, complaint, is_abort, strip_banner};
use scratch::Scratch;
use std::path::Path;

/// What to run when neither `$EDITOR` nor `$VISUAL` says.
///
/// `vi` because POSIX requires it to exist; a machine that has neither the variables nor
/// `vi` gets the spawn failure, which names what it tried.
pub(super) const FALLBACK_EDITOR: &str = "vi";

/// The editor command, in the order the issue specifies.
///
/// Note this is `$EDITOR` first and `$VISUAL` second, which is the reverse of the usual
/// convention — most tools prefer `$VISUAL` and fall back to `$EDITOR`.
pub(super) fn choose<'a>(editor: Option<&'a str>, visual: Option<&'a str>) -> &'a str {
    let set = |v: Option<&'a str>| v.map(str::trim).filter(|s| !s.is_empty());
    set(editor).or_else(|| set(visual)).unwrap_or(FALLBACK_EDITOR)
}

/// Open the template in the operator's editor and hand back what they wrote.
///
/// Loops on a complaint rather than giving up: the work stays in the buffer, and the two
/// ways out — empty it, or leave the editor non-zero — are both under the operator's hand.
pub(super) fn edit(title: &str) -> Result<String, String> {
    let (editor, visual) = (std::env::var("EDITOR").ok(), std::env::var("VISUAL").ok());
    edit_with(title, choose(editor.as_deref(), visual.as_deref()))
}

/// The loop itself, with the editor named rather than looked up.
///
/// Separated so the tests can drive it with a script that edits the buffer the way a person
/// would. Testing it through the environment instead would need a terminal, which is the one
/// thing a test harness cannot conjure.
fn edit_with(title: &str, command: &str) -> Result<String, String> {
    let mut offered = crate::verbs::TEMPLATE.replace("{title}", title);
    let scratch = Scratch::new(&offered)?;
    loop {
        run(command, scratch.path())?;

        let buffer = scratch.read()?;
        if is_abort(&buffer, &offered) {
            return Err("new: the buffer came back empty or unchanged, so nothing was filed".to_string());
        }
        let Some(why) = complaint(&buffer, title) else {
            return Ok(strip_banner(&buffer));
        };
        offered = annotated(&buffer, &why);
        scratch.write(&offered)?;
    }
}

/// Run the editor on `path`, inheriting the terminal it needs.
///
/// A non-zero exit aborts: an editor says "I quit without saving" that way, and treating it
/// as success would file whatever was on disk when they gave up.
fn run(command: &str, path: &Path) -> Result<(), String> {
    // Split on whitespace so `EDITOR="code --wait"` works, which is how people actually set
    // it. No shell: a command needing one can be a script, and going through `sh -c` would
    // make the path an interpolation hazard.
    let mut parts = command.split_whitespace();
    let program = parts.next().ok_or_else(|| "new: the editor is set to nothing".to_string())?;
    let status = std::process::Command::new(program).args(parts).arg(path).status().map_err(|e| format!("new: cannot run the editor {program:?}: {e}"))?;
    if status.success() {
        return Ok(());
    }
    Err(format!("new: the editor {program:?} exited without saving, so nothing was filed"))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn editor_is_preferred_then_visual_then_the_fallback() {
        assert_eq!(choose(Some("ed"), Some("vim")), "ed");
        assert_eq!(choose(None, Some("vim")), "vim");
        assert_eq!(choose(None, None), FALLBACK_EDITOR);
    }

    /// An exported-but-empty variable is how a shell says "unset" by accident. Running the
    /// empty string as a program would fail with something unrecognisable.
    #[test]
    fn a_blank_variable_is_treated_as_unset() {
        assert_eq!(choose(Some("  "), Some("vim")), "vim");
        assert_eq!(choose(Some(""), None), FALLBACK_EDITOR);
    }

    /// The loop itself, driven by a shell script standing in for a person.
    ///
    /// Unix only: it needs an executable script, and `#[cfg(windows)]` has no cheap
    /// equivalent. The rules the script exercises are platform-independent; what is skipped
    /// is the evidence that they hold when a real process does the editing.
    #[cfg(unix)]
    mod driven {
        use super::*;
        use std::path::PathBuf;

        /// A script that stands in for the editor. `$1` is the buffer.
        struct Editor(PathBuf);

        impl Editor {
            fn new(tag: &str, script: &str) -> Editor {
                use std::sync::atomic::{AtomicUsize, Ordering};
                static N: AtomicUsize = AtomicUsize::new(0);
                let n = N.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!("trck-fake-editor-{tag}-{}-{n}.sh", std::process::id()));
                std::fs::write(&path, script).expect("write script");
                Editor(path)
            }

            /// Run through `sh` rather than executing the script directly.
            ///
            /// Exec'ing a file this process has just written races with itself: `cargo test`
            /// runs these threads in parallel, and a thread still holding the file open for
            /// writing makes another thread's `exec` fail with `ETXTBSY`. Handing the path
            /// to `sh` as an argument makes the exec target `sh`, which nobody is writing.
            fn command(&self) -> String {
                format!("sh {}", self.0.display())
            }
        }

        impl Drop for Editor {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }

        #[test]
        fn a_written_buffer_becomes_the_body() {
            let ed = Editor::new("writes", "printf '# T\\n\\nwhat I typed.\\n' > \"$1\"");
            assert_eq!(edit_with("T", &ed.command()).expect("body"), "# T\n\nwhat I typed.\n");
        }

        #[test]
        fn an_editor_that_saves_nothing_aborts() {
            let ed = Editor::new("noop", "exit 0");
            let err = edit_with("T", &ed.command()).expect_err("aborted");
            assert!(err.contains("unchanged"), "{err}");
        }

        #[test]
        fn an_editor_that_exits_non_zero_aborts() {
            let ed = Editor::new("angry", "exit 3");
            let err = edit_with("T", &ed.command()).expect_err("aborted");
            assert!(err.contains("without saving"), "{err}");
        }

        /// The heart of it: a bad answer comes back annotated, with the operator's own text
        /// still there, and a second save is accepted.
        #[test]
        fn a_complaint_re_opens_the_buffer_with_the_work_intact() {
            let ed = Editor::new(
                "fixes",
                concat!(
                    // First pass writes a wrong heading; the second finds the complaint already
                    // in the buffer and corrects only the heading, leaving its prose alone.
                    "if grep -q 'trck:' \"$1\"; then\n",
                    "  sed -i.bak 's/^# Wrong$/# T/' \"$1\" && rm -f \"$1.bak\"\n",
                    "else\n",
                    "  printf '# Wrong\\n\\nprose worth keeping.\\n' > \"$1\"\n",
                    "fi",
                ),
            );
            let body = edit_with("T", &ed.command()).expect("body");
            assert_eq!(body, "# T\n\nprose worth keeping.\n", "the second pass did not survive");
            assert!(!body.contains("trck:"), "the annotation was filed: {body}");
        }

        #[test]
        fn the_scratch_file_is_gone_after_an_abort() {
            // The script records where it was asked to edit, so the test can look for it
            // after the abort it causes.
            let seen = std::env::temp_dir().join(format!("trck-seen-{}.txt", std::process::id()));
            let _ = std::fs::remove_file(&seen);
            let ed = Editor::new("records", &format!("printf '%s' \"$1\" > {}; exit 1", seen.display()));
            assert!(edit_with("T", &ed.command()).is_err(), "expected the abort");
            let scratch = std::fs::read_to_string(&seen).expect("the script ran");
            let _ = std::fs::remove_file(&seen);
            assert!(!Path::new(&scratch).exists(), "scratch survived an abort: {scratch}");
        }
    }
}
