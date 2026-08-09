//! Explaining an effective dependency cycle in terms of what someone actually typed.
//!
//! The node loop is the symptom, and on its own it is not actionable: an effective cycle is
//! usually *implied* by lifting rather than authored, so naming only the loop would leave
//! nothing to go and fix. Every edge of the loop therefore gets a **witness** — the authored
//! edge that makes it hold — plus a note for each lifting step that connects the two.

use crate::graph::Graph;
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// A human-readable reason for an effective cycle: the node loop plus the authored edges
/// and parent links that induce it.
pub(crate) fn describe_cycle(g: &Graph, cyc: &[String]) -> String {
    let seq = closed_loop(cyc);
    let chain = seq.iter().map(|c| format!("#{c}")).collect::<Vec<_>>().join(" -> ");
    let mut authored: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    for pair in seq.windows(2) {
        explain_edge(g, pair, &mut authored, &mut notes);
    }
    assemble(chain, &authored, &notes)
}

/// The loop with its first node repeated at the end, so `windows(2)` yields every edge —
/// including the one that closes it, which is the edge most likely to be the culprit.
fn closed_loop(cyc: &[String]) -> Vec<String> {
    let mut seq: Vec<String> = cyc.to_vec();
    if let Some(first) = cyc.first() {
        seq.push(first.clone());
    }
    seq
}

/// Record what makes one edge of the loop hold.
#[allow(clippy::many_single_char_names, reason = "u/v are the loop edge, a/b the witness")]
fn explain_edge(g: &Graph, pair: &[String], authored: &mut Vec<String>, notes: &mut Vec<String>) {
    let (Some(u), Some(v)) = (pair.first(), pair.get(1)) else {
        return;
    };
    let Some((a, b)) = witness(g, u, v) else {
        return;
    };
    let edge = format!("#{a} -> #{b}");
    if !authored.contains(&edge) {
        authored.push(edge);
    }
    // Only the lifting steps are worth saying. Where `a == u` and `b == v` the authored edge
    // *is* the loop edge, and repeating it as a note would add nothing.
    if a != *u {
        notes.push(format!("#{u} inherits #{a}'s deps"));
    }
    if b != *v {
        notes.push(format!("#{v} is under #{b}"));
    }
}

/// The authored edge `(a -> b)` that makes `u` reach `v`: `a` an ancestor-or-self of `u`, and
/// `v` somewhere inside `subtree(b)`. That pair is the thing someone typed.
fn witness(g: &Graph, u: &str, v: &str) -> Option<(String, String)> {
    for a in std::iter::once(u.to_string()).chain(g.ancestors_of(u)) {
        for b in g.requires_of(&a) {
            if g.subtree(&b).iter().any(|n| n == v) {
                return Some((a, b));
            }
        }
    }
    None
}

/// The chain, then the authored edges, then the lifting notes — each section only when it has
/// something to say, and the notes deduplicated because one lifting step can explain several
/// edges of the same loop.
fn assemble(chain: String, authored: &[String], notes: &[String]) -> String {
    let mut reason = chain;
    if !authored.is_empty() {
        let _ = write!(reason, "; authored: {}", authored.join(", "));
    }
    if !notes.is_empty() {
        let mut seen = BTreeSet::new();
        let unique: Vec<&String> = notes.iter().filter(|n| seen.insert((*n).clone())).collect();
        let _ = write!(reason, "; {}", unique.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
    }
    reason
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::test_graph::graph;

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| (*s).to_string()).collect()
    }

    /// A directly authored loop needs no lifting notes: each edge is its own witness.
    #[test]
    fn a_directly_authored_loop_names_both_edges_and_nothing_else() {
        let g = graph(&["a ->b", "b ->a"]);
        let out = describe_cycle(&g, &ids(&["a", "b"]));
        assert_eq!(out, "#a -> #b -> #a; authored: #a -> #b, #b -> #a");
    }

    /// The loop is closed, so the edge back to the start is explained too — three nodes give
    /// three authored edges, not two.
    #[test]
    fn the_closing_edge_is_explained_as_well() {
        let g = graph(&["a ->b", "b ->c", "c ->a"]);
        let out = describe_cycle(&g, &ids(&["a", "b", "c"]));
        assert_eq!(out, "#a -> #b -> #c -> #a; authored: #a -> #b, #b -> #c, #c -> #a");
    }

    /// The case the witness exists for: a one-node loop nobody typed. `a` requires `c`, and
    /// `c` is under `b` which is under `a` — so `c` inherits a dependency on itself.
    #[test]
    fn an_inherited_loop_names_the_authored_edge_and_the_lifting_step() {
        let g = graph(&["a ->c", "b:a", "c:b"]);
        let out = describe_cycle(&g, &ids(&["c"]));
        assert!(out.starts_with("#c -> #c"), "{out}");
        assert!(out.contains("authored: #a -> #c"), "{out}");
        assert!(out.contains("#c inherits #a's deps"), "{out}");
    }

    /// A lifting step explaining more than one edge of the loop is said once.
    #[test]
    fn a_repeated_lifting_note_is_not_repeated() {
        let g = graph(&["a ->c", "b:a", "c:b"]);
        let out = describe_cycle(&g, &ids(&["c", "c"]));
        assert_eq!(out.matches("inherits").count(), 1, "{out}");
    }

    /// An edge with no witness at all — malformed data reaches here — leaves the chain alone
    /// rather than dropping the whole explanation.
    #[test]
    fn an_unexplainable_edge_still_yields_the_chain() {
        let g = graph(&["a", "b"]);
        assert_eq!(describe_cycle(&g, &ids(&["a", "b"])), "#a -> #b -> #a");
    }

    #[test]
    fn an_empty_cycle_is_an_empty_chain() {
        assert_eq!(describe_cycle(&graph(&["a"]), &[]), "");
    }
}
