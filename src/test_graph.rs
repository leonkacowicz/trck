//! A compact graph builder for tests.
//!
//! Its own module rather than a helper inside one test module: `graph` and `rank` both
//! reason about the same derived structure, and a second copy of the builder is a second
//! thing to keep in step with [`Issue`].

// Tests assert; that is their job. The crate denies unwrap/expect/panic because a
// malformed tracker must produce a diagnostic rather than a stack trace, but a test
// that cannot panic cannot fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::graph::Graph;
use crate::issue::Issue;
use crate::json::parse;

/// Whatever a spec line carries after its id, with the defaults an omitted one means.
struct Attrs {
    deps: Vec<String>,
    labels: Vec<String>,
    status: String,
    priority: String,
    points: i64,
}

/// The sigil-prefixed parts, in any order: `->dep,dep`, `@status`, `!priority`, `#points`,
/// `+label`. Anything unrecognised is ignored rather than rejected — a spec is a test fixture,
/// and a typo should fail the assertion it was written for, not this parser.
fn attrs(spec: &str) -> Attrs {
    let mut a = Attrs { deps: Vec::new(), labels: Vec::new(), status: "backlog".to_string(), priority: "medium".to_string(), points: 1 };
    for part in spec.split_whitespace().skip(1) {
        match part.chars().next() {
            Some('-') => a.deps = part[2..].split(',').map(str::to_string).collect(),
            Some('@') => a.status = part[1..].to_string(),
            Some('!') => a.priority = part[1..].to_string(),
            Some('#') => a.points = part[1..].parse().unwrap_or(1),
            Some('+') => a.labels.push(part[1..].to_string()),
            _ => {},
        }
    }
    a
}

/// `id[:parent][ ->dep,dep][ @status][ !priority][ #points][ +label]`, so a graph reads as
/// one line per issue instead of six lines of struct literal.
pub(crate) fn issue(spec: &str) -> Issue {
    let a = attrs(spec);
    let head = spec.split_whitespace().next().unwrap_or("x");
    let (id, parent) = head.split_once(':').unwrap_or((head, ""));
    let list = |key: &str, items: &[String]| {
        if items.is_empty() { String::new() } else { format!(r#", "{key}": [{}]"#, items.iter().map(|d| format!("\"{d}\"")).collect::<Vec<_>>().join(", ")) }
    };
    let json = format!(
        r#"{{"id": "{id}", "slug": "{id}", "title": "{id}", "status": "{}",
             "priority": "{}", "points": {}{}{}{}}}"#,
        a.status,
        a.priority,
        a.points,
        if parent.is_empty() { String::new() } else { format!(r#", "parent": "{parent}""#) },
        list("depends_on", &a.deps),
        list("labels", &a.labels)
    );
    Issue::from_json(&parse(&json).expect("valid json")).expect("valid issue")
}

pub(crate) fn graph(specs: &[&str]) -> Graph {
    Graph::new(specs.iter().map(|s| issue(s)).collect())
}
