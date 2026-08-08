//! The committed half of `setup-git`: `<tracker>/.gitattributes`.
//!
//! This file *names* the merge drivers and is shared, unlike the per-clone `.git/config` that
//! defines what they run. It is also where the line endings the engine's formats require are
//! pinned, which is not a style preference — see [`GITATTRIBUTES_LINES`].

// Matched as a prefix so a header written by an older version is recognised as ours and
// refreshed in place, rather than accumulating one comment per release.
pub(super) const GITATTRIBUTES_HEADER_PREFIX: &str = "# Managed by `trck repo setup-git`";
pub(super) const GITATTRIBUTES_HEADER: &str =
    concat!("# Managed by `trck repo setup-git`", " — trck's merge drivers, and the line endings its formats require.");
// `text eol=lf` is not a style preference. `index.jsonl` and `SUMMARY.md` are rendered with
// `\n` and compared byte for byte, and the bodies are rewritten by `edit --title`. Checked
// out as CRLF, the working tree disagrees with the engine from the first verb onwards and
// every commit shows the whole file as changed.
pub(super) const GITATTRIBUTES_LINES: &[&str] =
    &["index.jsonl merge=trck-index text eol=lf", "SUMMARY.md merge=trck-summary text eol=lf", "items/*.md text eol=lf"];

/// The lines to write, or `None` when the file already says all of this.
pub(super) fn gitattributes_update(existing: &[&str]) -> Option<Vec<String>> {
    let mut out: Vec<String> = existing.iter().map(|s| (*s).to_string()).collect();
    let mut changed = false;
    let mut missing: Vec<String> = Vec::new();
    let mut anchor: Option<usize> = None;

    for want in GITATTRIBUTES_LINES {
        match ours_at(&out, want) {
            Some(i) => {
                changed |= refresh(&mut out, i, want);
                anchor = Some(i);
            },
            None => missing.push((*want).to_string()),
        }
    }

    let header = out.iter().position(|l| l.starts_with(GITATTRIBUTES_HEADER_PREFIX));
    if let Some(i) = header {
        changed |= refresh(&mut out, i, GITATTRIBUTES_HEADER);
    }

    if !missing.is_empty() {
        splice_in(&mut out, missing, anchor, header.is_some());
        changed = true;
    }
    changed.then_some(out)
}

/// Where a line we may replace already sits, if one does.
///
/// *Ours to replace* means it names one of our paths and carries nothing beyond the attributes
/// we manage — which is how a tracker set up before an attribute was added gets upgraded in
/// place instead of growing a second, stale rule for the same path. A rule carrying anything
/// else is somebody's decision, so ours goes beside it and git resolves the pair.
fn ours_at(have: &[String], want: &str) -> Option<usize> {
    let mut fields = want.split_whitespace();
    let pattern = fields.next().unwrap_or(want);
    let ours: Vec<&str> = fields.collect();
    have.iter().position(|line| {
        let mut got = line.split_whitespace();
        got.next() == Some(pattern) && got.all(|a| ours.contains(&a))
    })
}

/// Overwrite line `i` unless it already says `want`. Reports whether it wrote.
fn refresh(out: &mut [String], i: usize, want: &str) -> bool {
    match out.get_mut(i) {
        Some(line) if line != want => {
            *line = want.to_string();
            true
        },
        _ => false,
    }
}

/// Add the lines the file does not have yet.
///
/// Under an existing managed line when there is one, so the block stays contiguous under the
/// header it already has. With nothing to anchor to, the block is appended — after a blank
/// separator, and behind a header of its own when the file carries none.
fn splice_in(out: &mut Vec<String>, missing: Vec<String>, anchor: Option<usize>, has_header: bool) {
    if let Some(i) = anchor {
        for (k, line) in missing.into_iter().enumerate() {
            out.insert(i + 1 + k, line);
        }
        return;
    }
    if out.last().is_some_and(|l| !l.trim().is_empty()) {
        out.push(String::new());
    }
    if !has_header {
        out.push(GITATTRIBUTES_HEADER.to_string());
    }
    out.extend(missing);
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn update(existing: &str) -> Option<Vec<String>> {
        gitattributes_update(&existing.lines().collect::<Vec<_>>())
    }

    fn line_for<'a>(lines: &'a [String], pattern: &str) -> Vec<&'a String> {
        lines.iter().filter(|l| l.split_whitespace().next() == Some(pattern)).collect()
    }

    /// A CRLF checkout would put the working tree at odds with the engine.
    ///
    /// `index.jsonl` and `SUMMARY.md` are rendered with `\n` and compared byte for byte,
    /// and the bodies are rewritten by `edit --title`. Clone any of them onto a machine
    /// with `core.autocrlf=true` and the next verb rewrites the whole file back, so every
    /// commit shows it as wholly changed.
    #[test]
    fn everything_the_engine_writes_is_pinned_to_lf() {
        let out = update("").expect("an absent file is written");
        for pattern in ["index.jsonl", "SUMMARY.md", "items/*.md"] {
            let found = line_for(&out, pattern);
            assert_eq!(found.len(), 1, "{pattern} in {out:?}");
            let attrs: Vec<&str> = found[0].split_whitespace().skip(1).collect();
            assert!(attrs.contains(&"text"), "{}", found[0]);
            assert!(attrs.contains(&"eol=lf"), "{}", found[0]);
        }
    }

    #[test]
    fn a_file_that_already_says_all_of_it_is_left_alone() {
        let text = format!("{GITATTRIBUTES_HEADER}\n{}\n", GITATTRIBUTES_LINES.join("\n"));
        assert!(update(&text).is_none(), "rewrote an up-to-date file");
    }

    /// The old line is replaced, not joined by a second one for the same path. Two lines
    /// naming `index.jsonl` would in fact work — git applies the last value for each
    /// attribute — but a managed block that grows a stale copy of itself on every upgrade
    /// is one nobody can read.
    #[test]
    fn a_tracker_set_up_before_the_pin_is_upgraded_in_place() {
        let out = update(
            "# Managed by `trck repo setup-git` — trck merge drivers.\n\
             index.jsonl merge=trck-index\n\
             SUMMARY.md merge=trck-summary\n",
        )
        .expect("an out-of-date file is rewritten");
        let found = line_for(&out, "index.jsonl");
        assert_eq!(found.len(), 1, "{out:?}");
        assert!(found[0].contains("eol=lf"), "{}", found[0]);
        // One header, refreshed rather than duplicated, and the block stays contiguous.
        let headers: Vec<&String> = out.iter().filter(|l| l.starts_with(GITATTRIBUTES_HEADER_PREFIX)).collect();
        assert_eq!(headers, vec![&GITATTRIBUTES_HEADER.to_string()], "{out:?}");
    }

    /// Replacing in place is for *our* stale lines. A rule carrying anything we do not
    /// manage is someone's decision, so ours is added beside it and git resolves the two.
    #[test]
    fn a_users_own_rule_for_our_path_is_not_overwritten() {
        let out = update("index.jsonl -diff\n").expect("ours is still missing");
        assert!(out.iter().any(|l| l == "index.jsonl -diff"), "{out:?}");
        assert!(out.iter().any(|l| l.contains("merge=trck-index")), "{out:?}");
    }

    #[test]
    fn unrelated_content_survives() {
        let out = update("*.png binary\n").expect("ours is missing");
        assert!(out.iter().any(|l| l == "*.png binary"), "{out:?}");
        assert!(out.iter().any(|l| l.contains("merge=trck-index")), "{out:?}");
    }

    #[test]
    fn applying_twice_changes_nothing_the_second_time() {
        let once = update("*.png binary\n").expect("ours is missing");
        assert!(gitattributes_update(&once.iter().map(String::as_str).collect::<Vec<_>>()).is_none(), "not idempotent: {once:?}");
    }

    /// A partial upgrade keeps the managed block together rather than appending the new line
    /// at the end of the file, which would leave the block split around whatever sits between.
    #[test]
    fn a_missing_line_joins_the_block_it_belongs_to() {
        let out = update(
            "# Managed by `trck repo setup-git` — old.\n\
             index.jsonl merge=trck-index text eol=lf\n\
             SUMMARY.md merge=trck-summary text eol=lf\n\
             \n\
             *.png binary\n",
        )
        .expect("items/*.md is missing");
        let at = |pat: &str| out.iter().position(|l| l.split_whitespace().next() == Some(pat));
        assert_eq!(at("items/*.md"), at("SUMMARY.md").map(|i| i + 1), "block is not contiguous: {out:?}");
        assert!(out.iter().any(|l| l == "*.png binary"), "{out:?}");
    }
}
