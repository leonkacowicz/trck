//! Asking a revision what it holds.
//!
//! The four questions this engine ever puts to git's object store, and none of them about
//! trackers: does this revision exist, what is at this path, what is in this tree, where does
//! this repository start. Split from [`super`] because that file is the *process* wrapper —
//! how git is spawned is a different concern from what it is asked — and because only this
//! half grows as the ref-backed tracker learns to read more of one.

use super::run;
use std::path::{Path, PathBuf};

/// The commit `rev` names, or `None` when it names nothing.
///
/// `^{commit}` rather than a bare resolve, so a tag or a tree answers with the commit or not
/// at all — a caller asking for a revision wants something it can read a tree out of.
pub(crate) fn rev_parse(cwd: &Path, rev: &str) -> Result<Option<String>, String> {
    let out = run(cwd, &["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")])?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_string()))
}

/// The contents of `path` at `rev`, or `None` when the revision does not hold it.
///
/// Untrimmed, unlike [`stdout`]: this is file content, and a tracker's `index.jsonl` is
/// newline-terminated by definition. Absence is `None` rather than an error because it is
/// a legitimate answer — a revision from before the tracker existed holds no index, and
/// that means "everything is new", not "something went wrong".
pub(crate) fn show(cwd: &Path, rev: &str, path: &str) -> Result<Option<String>, String> {
    let out = run(cwd, &["show", &format!("{rev}:{path}")])?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// The names directly inside `rev:dir`, sorted, or `None` when the revision has no such
/// directory.
///
/// `--name-only` and no `-r`, so this answers with one level the way `read_dir` does rather
/// than the whole subtree. Absence is `None` for the same reason it is in [`show`]: a
/// tracker whose `items/` has not been created yet is empty, not broken.
pub(crate) fn ls_tree(cwd: &Path, rev: &str, dir: &str) -> Result<Option<Vec<String>>, String> {
    let out = run(cwd, &["ls-tree", "--name-only", "-z", &format!("{rev}:{dir}")])?;
    if !out.status.success() {
        return Ok(None);
    }
    // NUL-separated: a name git would otherwise quote and escape comes back verbatim, and
    // an issue slug is not guaranteed to be free of anything git considers unusual.
    let mut names: Vec<String> = String::from_utf8_lossy(&out.stdout).split('\0').filter(|n| !n.is_empty()).map(str::to_string).collect();
    names.sort();
    Ok(Some(names))
}

/// Every blob `rev` holds, as `(path, sha)` pairs — the whole tree, flattened.
///
/// The write path needs this because a tree is built from an index that starts empty: a
/// changeset names the handful of files it touches, and everything else has to be carried
/// forward explicitly. `-z` rather than the default output because git quotes and escapes
/// unusual path names otherwise, and an issue slug is not something to trust to that.
///
/// Non-blob entries are dropped. A tracker holds files; a submodule or a symlink in one is
/// not something this engine can round-trip, and silently rewriting it as a blob would be
/// worse than leaving it out of a tree it was never in.
pub(crate) fn tree_blobs(cwd: &Path, rev: &str) -> Result<Vec<(String, String)>, String> {
    let out = run(cwd, &["ls-tree", "-r", "-z", rev])?;
    if !out.status.success() {
        return Err(format!("git ls-tree -r {rev}: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).split('\0').filter_map(parse_ls_tree_record).collect())
}

/// One `<mode> <type> <sha>\t<path>` record, or `None` when it is not a blob.
fn parse_ls_tree_record(record: &str) -> Option<(String, String)> {
    let (meta, path) = record.split_once('\t')?;
    let mut fields = meta.split_whitespace();
    let (_mode, kind, sha) = (fields.next()?, fields.next()?, fields.next()?);
    (kind == "blob").then(|| (path.to_string(), sha.to_string()))
}

/// Is `ancestor` reachable from `descendant`?
///
/// `merge-base --is-ancestor` answers by exit status, so `false` here is an answer rather
/// than a failure — which is why it reads the status instead of going through [`super::stdout`].
/// A revision is its own ancestor; callers that care about equality check it first.
pub(crate) fn is_ancestor(cwd: &Path, ancestor: &str, descendant: &str) -> Result<bool, String> {
    Ok(super::run(cwd, &["merge-base", "--is-ancestor", ancestor, descendant])?.status.success())
}

/// The working tree's root, or `None` when `cwd` is not inside a repository.
pub(crate) fn repo_root(cwd: &Path) -> Result<Option<PathBuf>, String> {
    let out = run(cwd, &["rev-parse", "--show-toplevel"])?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. See the note in `discovery::tests`.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::git::stdout;
    use crate::git::tests::{commit_file, repo};

    #[test]
    fn rev_parse_answers_with_the_commit_and_with_none_for_an_unknown_revision() {
        let Some((_tmp, dir)) = repo("git-revparse") else { return };
        let head = commit_file(&dir, "a.txt", "one\n");
        assert_eq!(rev_parse(&dir, "HEAD").unwrap(), Some(head));
        assert_eq!(rev_parse(&dir, "no-such-branch").unwrap(), None);
    }

    #[test]
    fn show_reads_content_untrimmed_and_answers_none_for_a_path_the_revision_lacks() {
        let Some((_tmp, dir)) = repo("git-show") else { return };
        commit_file(&dir, "index.jsonl", "{\"id\": \"a\"}\n");
        assert_eq!(show(&dir, "HEAD", "index.jsonl").unwrap(), Some("{\"id\": \"a\"}\n".to_string()));
        assert_eq!(show(&dir, "HEAD", "items/nope.md").unwrap(), None);
    }

    /// A record that is not a blob is dropped rather than rewritten: a tracker holds files,
    /// and a tree this engine rebuilt would silently turn anything else into one.
    #[test]
    fn tree_records_parse_to_blobs_and_drop_everything_else() {
        assert_eq!(parse_ls_tree_record("100644 blob abc123\titems/a-a.md"), Some(("items/a-a.md".to_string(), "abc123".to_string())));
        assert_eq!(parse_ls_tree_record("160000 commit abc123\tvendor"), None);
        assert_eq!(parse_ls_tree_record(""), None, "the trailing field of a -z stream");
    }

    /// The whole tree, flattened, with nested paths intact — what a rebuilt tree has to carry
    /// forward for every file the changeset does not mention.
    #[test]
    fn tree_blobs_lists_every_blob_the_revision_holds() {
        let Some((_tmp, dir)) = repo("git-lstree") else { return };
        std::fs::create_dir_all(dir.join("items")).expect("mkdir");
        std::fs::write(dir.join("index.jsonl"), "{}\n").expect("write");
        std::fs::write(dir.join("items/a-a.md"), "# a\n").expect("write");
        stdout(&dir, &["add", "-A"]).expect("add");
        stdout(&dir, &["commit", "-q", "-m", "c"]).expect("commit");
        let paths: Vec<String> = tree_blobs(&dir, "HEAD").unwrap().into_iter().map(|(p, _)| p).collect();
        assert_eq!(paths, vec!["index.jsonl".to_string(), "items/a-a.md".to_string()]);
    }

    #[test]
    fn repo_root_finds_the_top_level_and_answers_none_outside_a_repository() {
        let Some((tmp, dir)) = repo("git-root") else { return };
        commit_file(&dir, "a.txt", "one\n");
        let nested = dir.join("sub");
        std::fs::create_dir_all(&nested).expect("mkdir");
        let root = repo_root(&nested).unwrap().expect("inside a repo");
        assert_eq!(root.canonicalize().ok(), dir.canonicalize().ok());
        // `Tmp`'s own root is a plain directory: the repository is the one made inside it.
        let outside = tmp.path().parent().expect("temp dir has a parent").to_path_buf();
        if repo_root(&outside).unwrap().is_some() {
            return; // the system temp dir is itself inside a repository; nothing to assert.
        }
        assert_eq!(repo_root(&outside).unwrap(), None);
    }
}
