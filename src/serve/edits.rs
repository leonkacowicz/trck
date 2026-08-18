//! The page's staged edits, as operations this process runs.
//!
//! `app.js` has always known exactly what each staged edit *means* — `commandFor` renders the
//! `trck` command that would do it, and the pending panel's whole job was to be copied into a
//! terminal. This is the other end of that: the same edits arrive as data, become the same
//! [`Op`], and are applied through the verb functions the CLI calls.
//!
//! **The engine never shells out to itself.** A rendered command string is what the *page*
//! shows a person; it is not a transport. Going out through `trck mv …` and back would put a
//! quoting layer between the page and the tracker, on a path where a title is arbitrary text —
//! and it would mean a `serve` running from one binary could drive whichever other one happens
//! to be first on `PATH`.
//!
//! **The edit vocabulary is the page's, and the mapping to ops lives here.** An edit says
//! which issue, which field, and what that field should now hold; nothing in the request names
//! a verb. That keeps the server the authority on what an edit means, so a page cannot ask for
//! an operation this endpoint did not intend to offer — and it is why [`ops_for`] is a `match`
//! over a closed list rather than anything that reads a verb out of the request. What keeps
//! the two ends honest is `tests/app_js.rs`, which runs the page's own `commandFor` under node
//! and compares it to what this builds.

use crate::issue::Issue;
use crate::json::Json;
use crate::verbs::Op;

/// One staged edit: `state.edits`' key split back apart, with its value.
#[derive(Debug)]
pub(crate) struct Edit {
    pub(crate) id: String,
    pub(crate) field: String,
    /// A scalar field's new value. A list field carries its whole desired contents instead —
    /// which is how a control that edits a list works, and it means the request never has to
    /// describe a delta the page would have to compute against a row it may have stale.
    pub(crate) value: Json,
}

/// Read the request document: `{"edits": [{"id", "field", "value"}, …]}`.
///
/// Strict about shape, because the one client is a page this binary rendered: a request that
/// is not that shape is a bug somewhere, and guessing at what it meant would hide it.
pub(crate) fn parse_request(body: &str) -> Result<Vec<Edit>, String> {
    let doc = crate::json::parse(body)?;
    let Some(Json::Array(items)) = doc.get("edits") else {
        return Err("expected a JSON object with an \"edits\" array".to_string());
    };
    items.iter().map(one_edit).collect()
}

fn one_edit(item: &Json) -> Result<Edit, String> {
    let text = |key: &str| item.get(key).and_then(Json::as_str).ok_or_else(|| format!("an edit is missing its \"{key}\""));
    let value = item.get("value").ok_or_else(|| "an edit is missing its \"value\"".to_string())?;
    Ok(Edit { id: text("id")?.to_string(), field: text("field")?.to_string(), value: value.clone() })
}

/// A scalar field's value as the command line spells it.
///
/// Numbers keep their source text — `points` is an integer to the tracker and a JSON number to
/// the page, and re-formatting one is how two languages quietly disagree about `2` and `2.0`.
fn scalar(value: &Json) -> Result<&str, String> {
    match value {
        Json::String(s) => Ok(s),
        Json::Number(n) => Ok(n),
        other => Err(format!("expected a string, got {}", other.type_name())),
    }
}

fn list(value: &Json) -> Result<Vec<&str>, String> {
    match value {
        Json::Array(items) => items.iter().map(|i| i.as_str().ok_or_else(|| format!("expected a list of strings, got a {}", i.type_name()))).collect(),
        other => Err(format!("expected a list, got {}", other.type_name())),
    }
}

/// The operations one staged edit means, against the issue as it stands.
///
/// A scalar field is exactly one op — the `mv` or `set` the panel renders for it. A **list**
/// field is the ops that make the list true, which is more than one when more than one edge
/// changed: `label` takes its whole add-and-remove set in a single call, while `dep` guards one
/// edge at a time against the cycle rule, so a two-edge change is genuinely two operations and
/// the panel renders two commands.
///
/// `row` is only read for the list fields, which need the current contents to diff against.
/// A scalar field replaces whatever was there, so it needs nothing.
pub(crate) fn ops_for(row: &Issue, field: &str, value: &Json) -> Result<Vec<Op>, String> {
    let id = row.id.as_str();
    let set = |flag: &str| -> Result<Vec<Op>, String> { Ok(vec![Op::new("set").operand(id).flag(flag, Some(scalar(value)?))]) };
    match field {
        // `mv` rather than `set --status`: status is the one field with a verb of its own,
        // because moving is what stamps `started`/`closed` and derives a parent's rollup.
        "status" => Ok(vec![Op::new("mv").operand(id).operand(scalar(value)?)]),
        "priority" => set("--priority"),
        "points" => set("--points"),
        "parent" => set("--parent"),
        "labels" | "requires" => Ok(list_ops(row, field, &list(value)?)),
        other => Err(format!("`{other}` is not an editable field")),
    }
}

/// A list field: the operations that make the list true.
///
/// One function for both, because they differ only in which list they are about and which verb
/// edits it — which is what makes "a list field becomes the ops that make it true" one rule
/// rather than two that happen to agree.
fn list_ops(row: &Issue, field: &str, want: &[&str]) -> Vec<Op> {
    let labels = field == "labels";
    let (add, remove) = difference(if labels { &row.labels } else { &row.depends_on }, want);
    let id = row.id.as_str();
    if labels { label_ops(id, &add, &remove) } else { dep_ops(id, &add, &remove) }
}

/// One `label` op carrying every change, or none at all when the list already matches.
fn label_ops(id: &str, add: &[String], remove: &[String]) -> Vec<Op> {
    if add.is_empty() && remove.is_empty() {
        return Vec::new();
    }
    vec![Op::new("label").operand(id).repeated("--add", add).repeated("--remove", remove)]
}

/// One `dep` op per changed edge.
///
/// Pairing an add with a removal in one op would be shorter and would also be a lie: `dep`
/// guards an added edge against the cycle rule *as the graph stands*, so two edges added in
/// one call would be checked against a graph neither of them is in yet.
fn dep_ops(id: &str, add: &[String], remove: &[String]) -> Vec<Op> {
    let added = add.iter().map(|t| Op::new("dep").operand(id).flag("--add", Some(t)));
    let removed = remove.iter().map(|t| Op::new("dep").operand(id).flag("--remove", Some(t)));
    // Removals first: an edge being replaced should not have to coexist with the one replacing
    // it, which is the difference between a legal swap and a refusal for a cycle that was only
    // ever going to be momentary.
    removed.chain(added).collect()
}

/// What has to be added and removed to turn `have` into `want`.
fn difference(have: &[String], want: &[&str]) -> (Vec<String>, Vec<String>) {
    let mut add = Vec::new();
    let mut remove = Vec::new();
    for w in want {
        if !have.iter().any(|h| h == w) {
            add.push((*w).to_string());
        }
    }
    for h in have {
        if !want.contains(&h.as_str()) {
            remove.push(h.clone());
        }
    }
    (add, remove)
}
