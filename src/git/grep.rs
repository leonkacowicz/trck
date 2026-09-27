//! Searching bodies for text, in a directory or in a revision.
//!
//! Split from [`super::read`] because it is a different kind of question: that file asks a
//! revision what it *holds*, this asks which of its files *contain* something — and it is the
//! one git call here that has to work on a plain directory too.

use super::run;
use std::path::Path;

/// The files under `dir` holding `needle` as a case-insensitive literal substring, as git
/// printed their paths.
///
/// `rev` picks the backend: `Some` reads the blobs of a revision with no checkout, `None`
/// is `--no-index`, which searches a plain directory whether or not it is inside a
/// repository and whether or not its files are tracked or ignored. One matcher either way,
/// which is the reason this is git's job rather than Rust's — `to_lowercase` is
/// Unicode-aware and git's folding is not, so a second, in-process matcher for the
/// directory case would answer differently on any body that is not pure ASCII.
///
/// The **pathspec** is the one thing the two forms cannot share. A relative pathspec
/// resolves against the working directory, and a revision is read from wherever the caller
/// happens to be — possibly several levels below the repository root — so it needs
/// `:(top)`. `--no-index` anchors `:(top)` at the enclosing *repository* root instead of at
/// the directory it was pointed at, which is not where a tracker's items are, so it needs
/// the plain spelling. Matching is what must not drift; addressing is per backend.
///
/// Exit 1 is git's "nothing matched", an empty answer rather than a failure. Paths come
/// back as printed — `<rev>:<path>` for a revision, working-directory-relative otherwise —
/// because only the caller knows which part of them it wanted.
pub(crate) fn grep_files(cwd: &Path, rev: Option<&str>, needle: &str, dir: &str) -> Result<Vec<String>, String> {
    let pathspec = if rev.is_some() { format!(":(top){dir}/") } else { format!("{dir}/") };
    let mut args = vec!["grep", "--files-with-matches", "-z", "--fixed-strings", "--ignore-case"];
    if rev.is_none() {
        args.push("--no-index");
    }
    // `-e` rather than a bare operand: the needle is user text and may begin with a dash.
    args.extend(["-e", needle]);
    if let Some(rev) = rev {
        args.push(rev);
    }
    args.extend(["--", pathspec.as_str()]);
    let out = run(cwd, &args)?;
    match out.status.code() {
        Some(0) => {},
        Some(1) => return Ok(Vec::new()),
        _ => return Err(format!("git grep: {}", String::from_utf8_lossy(&out.stderr).trim())),
    }
    // NUL-separated for the same reason [`super::ls_tree`] is: git quotes and escapes unusual path
    // names otherwise, and an issue slug is not guaranteed to be free of them.
    Ok(String::from_utf8_lossy(&out.stdout).split('\0').filter(|p| !p.is_empty()).map(str::to_string).collect())
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. See the note in `discovery::tests`.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::git::stdout;
    use crate::git::tests::repo;

    /// The two forms are one matcher: the same needle finds the same body whether it is read
    /// off disk or out of a revision, which is the property `list --contains` rests on.
    #[test]
    fn grep_files_finds_the_same_body_in_a_directory_and_in_a_revision() {
        let Some((_tmp, dir)) = repo("git-grep") else { return };
        std::fs::create_dir_all(dir.join("items")).expect("mkdir");
        std::fs::write(dir.join("items/aaa1111-a.md"), "# a\n\nA Race Condition in the parser.\n").expect("write");
        std::fs::write(dir.join("items/bbb2222-b.md"), "# b\n\nnothing to see.\n").expect("write");

        // Case-insensitive and literal: the pattern is neither spelled nor cased like the body.
        let on_disk = grep_files(&dir, None, "race condition", "items").unwrap();
        assert_eq!(on_disk, vec!["items/aaa1111-a.md".to_string()]);

        stdout(&dir, &["add", "-A"]).expect("add");
        stdout(&dir, &["commit", "-q", "-m", "c"]).expect("commit");
        let in_rev = grep_files(&dir, Some("HEAD"), "race condition", "items").unwrap();
        assert_eq!(in_rev.len(), 1, "{in_rev:?}");
        assert!(in_rev[0].ends_with("items/aaa1111-a.md"), "{in_rev:?}");
    }

    /// A pattern nothing holds is an empty answer, not a failure: git says so with exit 1,
    /// which is the same status it uses for a real error in other commands.
    #[test]
    fn grep_files_reports_no_match_as_an_empty_answer() {
        let Some((_tmp, dir)) = repo("git-grep-none") else { return };
        std::fs::create_dir_all(dir.join("items")).expect("mkdir");
        std::fs::write(dir.join("items/aaa1111-a.md"), "# a\n").expect("write");
        assert!(grep_files(&dir, None, "nothing holds this", "items").unwrap().is_empty());
        // And so is a tracker whose items directory has not been created yet.
        assert!(grep_files(&dir, None, "a", "nosuchdir").unwrap().is_empty());
    }

    /// A needle beginning with a dash is text, not an option. `-e` is what says so.
    #[test]
    fn grep_files_treats_a_leading_dash_as_part_of_the_pattern() {
        let Some((_tmp, dir)) = repo("git-grep-dash") else { return };
        std::fs::create_dir_all(dir.join("items")).expect("mkdir");
        std::fs::write(dir.join("items/aaa1111-a.md"), "# a\n\n--fanout is documented here.\n").expect("write");
        assert_eq!(grep_files(&dir, None, "--fanout", "items").unwrap(), vec!["items/aaa1111-a.md".to_string()]);
    }
}
