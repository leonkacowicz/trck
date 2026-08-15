//! Building objects, without a checkout.
//!
//! The shape a caller assembles from these is always the same: hash each file into a blob,
//! build a tree out of the blobs, commit the tree. Nothing here touches the working tree or
//! the caller's index, which is the point — a tracker write must not disturb whatever the
//! operator was doing, and must work from a dirty tree on an unrelated branch.
//!
//! Only objects are made here. Moving a ref onto one of them — locally or on a remote, always
//! under a compare-and-swap — is [`super::refs`].

use super::exec;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Output;

/// Tracker files are ordinary non-executable files; nothing here writes any other mode.
const BLOB_MODE: &str = "100644";

/// A distinct scratch index per call: several `trck` processes may be writing at once, and
/// a fixed path would have them build each other's trees.
///
/// Outside the repository rather than inside `.git`, so an interrupted run leaves nothing
/// for a later `git status` to trip over.
fn temp_index() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("trck-index-{}-{n}", std::process::id()))
}

/// A successful command's trimmed stdout, or an error naming the command and git's own words.
fn succeeded(out: &Output, label: &str) -> Result<String, String> {
    if !out.status.success() {
        return Err(format!("git {label}: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Write `content` into the object store as a blob, answering with its sha.
pub(crate) fn hash_object(cwd: &Path, content: &str) -> Result<String, String> {
    let out = exec(cwd, &["hash-object", "-w", "--stdin"], &[], Some(content.as_bytes()))?;
    succeeded(&out, "hash-object")
}

/// Build a tree from `entries`, each a repo-relative path and the blob sha to place there.
///
/// The tree holds exactly what is passed and nothing else — it is built in a scratch index
/// that starts empty, so a caller describes the whole tree rather than a patch to an
/// existing one. For a branch whose root *is* the tracker that is the honest model: the
/// engine knows every file it owns.
///
/// Nested paths (`items/<id>-<slug>.md`) need no special handling; `write-tree` builds the
/// subtrees.
pub(crate) fn write_tree(cwd: &Path, entries: &[(&str, &str)]) -> Result<String, String> {
    let index = temp_index();
    let Some(index_path) = index.to_str() else {
        return Err("temp index path is not valid UTF-8".to_string());
    };
    let env = [("GIT_INDEX_FILE", index_path)];
    // `update-index --index-info` reads one `<mode> <sha>\t<path>` line per entry, which is
    // what makes the whole tree one invocation rather than one per file.
    let spec = entries.iter().fold(String::new(), |mut spec, (path, blob)| {
        let _ = writeln!(spec, "{BLOB_MODE} {blob}\t{path}");
        spec
    });
    let built = build_tree(cwd, &env, &spec);
    let _ = std::fs::remove_file(&index);
    built
}

/// The two commands [`write_tree`] runs, split out so the scratch index is removed on the
/// failing path as well as the succeeding one.
fn build_tree(cwd: &Path, env: &[(&str, &str)], spec: &str) -> Result<String, String> {
    let staged = exec(cwd, &["update-index", "--index-info"], env, Some(spec.as_bytes()))?;
    succeeded(&staged, "update-index")?;
    let tree = exec(cwd, &["write-tree"], env, None)?;
    succeeded(&tree, "write-tree")
}

/// Commit `tree` with `parents`, answering with the new commit's sha.
///
/// The message goes in on stdin, so a title carrying a newline or a leading dash is not a
/// command-line problem. An empty `parents` makes a root commit, which is how a tracker
/// branch begins.
pub(crate) fn commit_tree(cwd: &Path, tree: &str, parents: &[&str], message: &str) -> Result<String, String> {
    let mut args = vec!["commit-tree", tree];
    for parent in parents {
        args.push("-p");
        args.push(parent);
    }
    let out = exec(cwd, &args, &[], Some(message.as_bytes()))?;
    succeeded(&out, "commit-tree")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::git::refs::update_ref;
    use crate::git::stdout;
    use crate::git::tests::{commit_file, repo};
    use crate::git::{rev_parse, show};

    /// A tracker-shaped tree — an index at the root, a body under `items/` — committed and
    /// anchored at `refname`. This is the whole write path in miniature.
    fn commit_tracker(dir: &Path, refname: &str, index: &str) -> String {
        let index_blob = hash_object(dir, index).expect("blob");
        let body_blob = hash_object(dir, "# a\n").expect("blob");
        let tree = write_tree(dir, &[("index.jsonl", &index_blob), ("items/a-a.md", &body_blob)]).expect("tree");
        let commit = commit_tree(dir, &tree, &[], "new #a: a\n\nTrck-Op: new a\n").expect("commit");
        update_ref(dir, refname, &commit, None).expect("create ref");
        commit
    }

    #[test]
    fn a_blob_written_this_way_is_in_the_object_store() {
        let Some((_tmp, dir)) = repo("git-blob") else { return };
        let sha = hash_object(&dir, "{\"id\": \"a\"}\n").expect("blob");
        assert_eq!(stdout(&dir, &["cat-file", "-p", &sha]).unwrap(), "{\"id\": \"a\"}");
    }

    #[test]
    fn a_tree_carries_nested_paths_and_the_commit_reads_back_as_a_tracker() {
        let Some((_tmp, dir)) = repo("git-tree") else { return };
        let commit = commit_tracker(&dir, "refs/heads/trck-issues", "{\"id\": \"a\"}\n");
        assert_eq!(rev_parse(&dir, "refs/heads/trck-issues").unwrap(), Some(commit));
        assert_eq!(show(&dir, "refs/heads/trck-issues", "index.jsonl").unwrap(), Some("{\"id\": \"a\"}\n".to_string()));
        assert_eq!(show(&dir, "refs/heads/trck-issues", "items/a-a.md").unwrap(), Some("# a\n".to_string()));
    }

    #[test]
    fn the_working_tree_and_the_callers_index_are_untouched() {
        let Some((_tmp, dir)) = repo("git-clean") else { return };
        commit_file(&dir, "code.rs", "fn main() {}\n");
        std::fs::write(dir.join("dirty.txt"), "uncommitted\n").expect("write");
        commit_tracker(&dir, "refs/heads/trck-issues", "{\"id\": \"a\"}\n");
        // Only the file the test dirtied, and none of the tracker's paths.
        assert_eq!(stdout(&dir, &["status", "--porcelain"]).unwrap(), "?? dirty.txt");
        assert_eq!(stdout(&dir, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap(), "main");
    }

    #[test]
    fn a_commit_chains_onto_its_parent() {
        let Some((_tmp, dir)) = repo("git-chain") else { return };
        let first = commit_tracker(&dir, "refs/heads/trck-issues", "{\"id\": \"a\"}\n");
        let blob = hash_object(&dir, "{\"id\": \"b\"}\n").expect("blob");
        let tree = write_tree(&dir, &[("index.jsonl", &blob)]).expect("tree");
        let second = commit_tree(&dir, &tree, &[&first], "done #a (fixed)\n").expect("commit");
        update_ref(&dir, "refs/heads/trck-issues", &second, Some(&first)).expect("advance");
        assert_eq!(stdout(&dir, &["rev-list", "--count", "refs/heads/trck-issues"]).unwrap(), "2");
    }
}
