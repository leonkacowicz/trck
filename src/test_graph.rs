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

/// `id[:parent][ ->dep,dep][ @status][ !priority][ #points]`, so a graph reads as
/// one line per issue instead of six lines of struct literal.
pub(crate) fn issue(spec: &str) -> Issue {
    let mut parent = String::new();
    let mut deps: Vec<String> = Vec::new();
    let mut status = "backlog".to_string();
    let mut priority = "medium".to_string();
    let mut points = 1i64;
    for part in spec.split_whitespace().skip(1) {
        match part.chars().next() {
            Some('-') => deps = part[2..].split(',').map(str::to_string).collect(),
            Some('@') => status = part[1..].to_string(),
            Some('!') => priority = part[1..].to_string(),
            Some('#') => points = part[1..].parse().unwrap_or(1),
            _ => {},
        }
    }
    let mut id = spec.split_whitespace().next().unwrap_or("x").to_string();
    if let Some((a, b)) = id.clone().split_once(':') {
        id = a.to_string();
        parent = b.to_string();
    }
    let json = format!(
        r#"{{"id": "{id}", "slug": "{id}", "title": "{id}", "status": "{status}",
             "priority": "{priority}", "points": {points}{}{}}}"#,
        if parent.is_empty() { String::new() } else { format!(r#", "parent": "{parent}""#) },
        if deps.is_empty() {
            String::new()
        } else {
            format!(r#", "depends_on": [{}]"#, deps.iter().map(|d| format!("\"{d}\"")).collect::<Vec<_>>().join(", "))
        }
    );
    Issue::from_json(&parse(&json).expect("valid json")).expect("valid issue")
}

pub(crate) fn graph(specs: &[&str]) -> Graph {
    Graph::new(specs.iter().map(|s| issue(s)).collect())
}
