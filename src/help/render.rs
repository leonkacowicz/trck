//! Laying help text out: wrapping a paragraph, and the two-column blocks every page is
//! made of.
//!
//! Separate from the pages themselves because it is the part that does not change. The
//! tables are edited constantly and the prose with them; how a description is folded into
//! the space left by the term column is decided once.

/// Wrap `text` to `width` columns, prefixing every line with `indent`.
///
/// Written out rather than pre-wrapped in the table because the table is edited far more
/// often than this is, and a paragraph stored as one string is one a person can rewrite
/// without re-flowing it by hand.
pub(super) fn wrap(text: &str, width: usize, indent: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    let mut out = String::new();
    for l in &lines {
        out.push_str(indent);
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// A two-column block: the term, then its description wrapped in the remaining width.
pub(super) fn columns(rows: &[(&str, &str)], width: usize) -> String {
    columns_at(rows, width, rows.iter().map(|(t, _)| t.chars().count()).max().unwrap_or(0) + 2)
}

/// `columns` with the term column given rather than measured, for the overview: its groups
/// are one table broken by headings, and a gutter measured per group would step in and out
/// down the page as each group's longest verb differs.
pub(super) fn columns_at(rows: &[(&str, &str)], width: usize, gutter: usize) -> String {
    let mut out = String::new();
    for (term, desc) in rows {
        let indent = " ".repeat(gutter + 2);
        let body = wrap(desc, width.saturating_sub(gutter + 2), &indent);
        out.push_str("  ");
        out.push_str(term);
        out.push_str(&" ".repeat(gutter.saturating_sub(term.chars().count())));
        out.push_str(body.trim_start());
    }
    out
}
