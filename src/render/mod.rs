//! Human-facing rendering: id emphasis, the trailing annotations, and the one-line row
//! format the read verbs share.
//!
//! Four modules, and the read verbs import this one rather than any of them: [`colour`]
//! decides which colour anything is and whether there is colour at all, [`fields`] reads one
//! field off an issue as text, [`annotate`] builds the note at the end of a row, and [`rows`]
//! assembles the row itself. What is left here is the two pieces of id presentation everything
//! else uses, and the Python list form that is a contract rather than a style.

mod annotate;
mod colour;
mod fields;
mod rows;

pub(crate) use annotate::Annotation;
pub(crate) use colour::{LANE_PALETTE, gutter, lane_palette_index, paint, priority_codes, status_codes};
pub(crate) use fields::{field_value, field_value_raw};
pub(crate) use rows::{RowOpts, render_rows};

use std::collections::BTreeMap;

/// Each id mapped to the length of its shortest prefix that identifies it uniquely —
/// git-short-hash style, the fewest characters you would have to type. When an id is
/// itself a prefix of another, no shorter unique prefix exists, so its full length is
/// used.
pub(crate) fn unique_prefix_lens<'a>(ids: impl IntoIterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let mut uniq: Vec<&str> = ids.into_iter().collect();
    uniq.sort_unstable();
    uniq.dedup();
    let shared = |a: &str, b: &str| a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    let mut out = BTreeMap::new();
    for (i, id) in uniq.iter().enumerate() {
        let before = i.checked_sub(1).map_or(0, |j| shared(uniq[j], id));
        let after = uniq.get(i + 1).map_or(0, |n| shared(id, n));
        let need = before.max(after) + 1;
        out.insert((*id).to_string(), need.min(id.chars().count()).max(1));
    }
    out
}

/// An id with its unique prefix emphasised and the rest dimmed. `hash` prepends `#`,
/// which is the row and graph form; `show` wants the bare id.
pub(crate) fn hl_id(id: &str, abbrev: Option<&BTreeMap<String, usize>>, hash: bool) -> String {
    let head = if hash { "#" } else { "" };
    if let Some(cut) = abbrev.and_then(|a| a.get(id)) {
        let cut = (*cut).min(id.len());
        let (pre, rest) = id.split_at(cut);
        let mut out = format!("{head}{}", paint(pre, &["bold"]));
        if !rest.is_empty() {
            out.push_str(&paint(rest, &["dim"]));
        }
        return out;
    }
    paint(&format!("{head}{id}"), &["bold"])
}

/// A Python list literal. `label` and `dep` echo one back and `show` prints one, and the
/// conformance suite compares stdout literally — so the bracket-and-quote style is a
/// contract, not an accident of the first implementation.
pub(crate) fn python_list(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", "))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn unique_prefixes_are_the_fewest_characters_youd_type() {
        let lens = unique_prefix_lens(["aaaaaaa", "aabbbbb", "zzzzzzz"]);
        assert_eq!(lens["aaaaaaa"], 3); // shares "aa" with aabbbbb
        assert_eq!(lens["aabbbbb"], 3);
        assert_eq!(lens["zzzzzzz"], 1);
    }

    #[test]
    fn an_id_that_is_a_prefix_of_another_needs_all_of_itself() {
        let lens = unique_prefix_lens(["ab", "abcd"]);
        assert_eq!(lens["ab"], 2);
    }

    #[test]
    fn a_lone_id_needs_one_character() {
        assert_eq!(unique_prefix_lens(["k3m9x2a"])["k3m9x2a"], 1);
    }

    /// A duplicate id must not make one of them look ambiguous against itself.
    #[test]
    fn a_repeated_id_is_deduplicated_before_measuring() {
        let lens = unique_prefix_lens(["aaa", "aaa"]);
        assert_eq!(lens["aaa"], 1);
    }

    #[test]
    fn no_ids_is_an_empty_map_rather_than_a_panic() {
        assert!(unique_prefix_lens(std::iter::empty()).is_empty());
    }

    /// The bracket-and-quote style is compared literally by the conformance suite.
    #[test]
    fn a_python_list_quotes_every_item_and_separates_with_comma_space() {
        assert_eq!(python_list(&[]), "[]");
        assert_eq!(python_list(&["a".to_string()]), "['a']");
        assert_eq!(python_list(&["a".to_string(), "b".to_string()]), "['a', 'b']");
    }

    /// Without colour, `hl_id` is the id and nothing else — the abbreviation is emphasis, not
    /// truncation, so no view can lose characters to it.
    #[test]
    fn an_abbreviated_id_still_prints_in_full() {
        let mut abbrev = BTreeMap::new();
        abbrev.insert("aaaaaaa".to_string(), 3);
        let out = hl_id("aaaaaaa", Some(&abbrev), true);
        let bare: String = out.chars().filter(|c| *c == '#' || c.is_alphanumeric()).collect();
        assert_eq!(bare, "#aaaaaaa");
        assert!(!hl_id("aaaaaaa", None, false).contains('#'), "no hash when not asked");
    }
}
