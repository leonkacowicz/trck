//! The vocabulary, `trck.json`, and the format guard.
//!
//! Much smaller than its Python counterpart, because the vocabulary stopped being
//! configurable: statuses, priorities and resolutions are constants here, and the 58
//! call sites that used to read them from a config read predicates instead. What is left
//! in `trck.json` is the format version and where `update` pulls from — neither of which
//! is a decision about how to track work.

use crate::json::{Json, parse};

// --------------------------------------------------------------------------- //
// the vocabulary
// --------------------------------------------------------------------------- //
// Four statuses. Fixed — not configured, not renameable, not extensible.
//
//   backlog     not started.                      what `new` assigns, and all `ready` offers.
//   ongoing     started, someone is on it.        claimed; what a mixed parent rolls up to.
//   in-review   started, output pending judgement. claimed; nothing to pick up.
//   done        finished.                         satisfies a dependency.
//
// The middle two are the *claim*: there is no assignee field, so a status is how a
// tracker records that work is taken. Both still block their dependents.
//
// `in-review` is the one that needs a rule, because it looks like it overlaps
// `depends_on`: use `depends_on` when the blocker is real work someone will do and
// close, `in-review` when making it a task would be inventing one. A code review forces
// the distinction — the reviewer judges your deliverable rather than producing one.
pub(crate) const BACKLOG: &str = "backlog";
pub(crate) const ONGOING: &str = "ongoing";
pub(crate) const IN_REVIEW: &str = "in-review";
pub(crate) const DONE: &str = "done";
pub(crate) const STATUSES: &[&str] = &[BACKLOG, ONGOING, IN_REVIEW, DONE];

/// verb -> status, for the `start` / `review` / `done` aliases.
pub(crate) const VERB_STATUS: &[(&str, &str)] = &[("start", ONGOING), ("review", IN_REVIEW), ("done", DONE)];

/// Five priorities, ordered by precedence. Fixing the count also fixes the shape of the
/// demand vector, which is one slot per priority.
pub(crate) const PRIORITIES: &[&str] = &["urgent", "high", "medium", "low", "lowest"];

/// Three resolutions, valid only on `done`, all meaning *closed without shipping*. The
/// normal case is to carry none, and that absence is load-bearing: the changelog lists
/// issues closed without one, which is why there is deliberately no `fixed`.
pub(crate) const RESOLUTIONS: &[&str] = &["superseded", "wontfix", "duplicate"];

/// The tracker format this engine understands. See `SUPPORTED_FORMAT` in the Python
/// engine's `constants.py` for the bump policy — it is one policy, not two.
pub(crate) const SUPPORTED_FORMAT: i64 = 1;

/// Opt-in features a tracker may declare. None yet; the mechanism ships without one.
pub(crate) const KNOWN_EXTENSIONS: &[&str] = &[];

/// What `new` assigns when no priority is given: the middle one.
pub(crate) fn default_priority() -> &'static str {
    PRIORITIES[PRIORITIES.len() / 2]
}

pub(crate) fn initial_status() -> &'static str {
    BACKLOG
}

pub(crate) fn is_terminal(status: &str) -> bool {
    status == DONE
}

/// Whether `ready`/`next` may propose this as work to pick up: only `backlog`, work
/// nobody has started.
///
/// There is no assignee field, so `start` is the only claim a tracker records. Offering a
/// claimed issue to whoever asks next is exactly the collision `ready` exists to prevent —
/// which makes `ongoing` no more available than `in-review`, whose output is merely
/// pending someone else's judgement rather than someone else's keyboard.
///
/// For the fixed vocabulary this reduces to `== BACKLOG`, and is kept as a predicate
/// anyway: it is the seam a fifth status would slot into, and it names the rule at the
/// call sites instead of making each of them restate it. Its complement over the
/// unfinished statuses is [`is_in_flight`].
pub(crate) fn is_actionable(status: &str) -> bool {
    status == BACKLOG
}

/// Whether someone is holding this issue: started, not finished. There is no assignee
/// field, so `start` is the only claim a tracker records — which is what makes this the
/// set `next` names above its pick.
///
/// Spelled as the two statuses rather than "not backlog, not done" so an unrecognised
/// status from a hand-edited row is not reported as somebody's work in progress.
pub(crate) fn is_in_flight(status: &str) -> bool {
    status == ONGOING || status == IN_REVIEW
}

/// The status a parent should carry given its children's: all initial -> initial, all
/// terminal -> terminal, otherwise active.
pub(crate) fn reconcile(children: &[String]) -> &'static str {
    if children.iter().all(|s| s == BACKLOG) {
        return BACKLOG;
    }
    if children.iter().all(|s| is_terminal(s)) {
        return DONE;
    }
    ONGOING
}

pub(crate) fn resolve_alias(verb: &str) -> Option<&'static str> {
    VERB_STATUS.iter().find(|(v, _)| *v == verb).map(|(_, s)| *s)
}

// --------------------------------------------------------------------------- //
// value checks
// --------------------------------------------------------------------------- //
// One predicate per rule, shared by the command handlers (which fail on the returned
// message) and `validate` (which collects it). `None` means acceptable.

pub(crate) fn check_status(value: &str) -> Option<String> {
    if STATUSES.contains(&value) {
        return None;
    }
    Some(format!("unknown status '{value}' (expected one of: {})", STATUSES.join(", ")))
}

pub(crate) fn check_priority(value: &str) -> Option<String> {
    if PRIORITIES.contains(&value) {
        return None;
    }
    Some(format!("bad priority '{value}' (expected one of: {})", PRIORITIES.join(", ")))
}

pub(crate) fn check_resolution(value: &str) -> Option<String> {
    if RESOLUTIONS.contains(&value) {
        return None;
    }
    Some(format!("bad resolution '{value}' (expected one of: {})", RESOLUTIONS.join(", ")))
}

/// Forge-agnostic: any absolute http(s) link. The engine never talks to a forge, so the
/// only thing it can meaningfully check is the shape.
pub(crate) fn check_review_url(value: &str) -> Option<String> {
    let absolute = value.starts_with("http://") || value.starts_with("https://");
    let rest = value.split_once("//").map_or("", |x| x.1);
    if absolute && !rest.is_empty() && !value.chars().any(char::is_whitespace) {
        return None;
    }
    Some(format!("bad review_url '{value}' (must be an absolute http(s) URL)"))
}

pub(crate) fn check_points(value: i64) -> Option<String> {
    if value >= 0 {
        return None;
    }
    Some(format!("bad points {value} (must be a non-negative integer)"))
}

// --------------------------------------------------------------------------- //
// trck.json
// --------------------------------------------------------------------------- //

/// A tracker's configuration: what little of it is left.
#[derive(Debug, Clone, Default)]
pub(crate) struct Config {
    /// The declared format, or `None` when absent — which means the current shape, so
    /// every tracker written before the key existed keeps working.
    pub(crate) format: Option<i64>,
    pub(crate) update_repo: Option<String>,
    pub(crate) update_channel: Option<String>,
    /// Keys that used to define a vocabulary and no longer do, in file order.
    pub(crate) vestigial: Vec<String>,
    raw: Option<Json>,
}

pub(crate) const DEFAULT_UPDATE_REPO: &str = "leonkacowicz/trck";

/// Config keys that used to define a vocabulary. A tracker still carrying one is not
/// broken — the key is ignored — so this is a warning naming the replacement, not an
/// error that would lock the tracker out of every verb.
fn vestigial_reason(key: &str) -> Option<String> {
    let joined = |v: &[&str]| v.join(", ");
    Some(match key {
        "statuses" => format!("the vocabulary is fixed: {}", joined(STATUSES)),
        "aliases" => format!("the verbs map to fixed statuses: {}", joined(STATUSES)),
        "kinds" => "`kind` is an ordinary custom field now (`set --field kind=bug`)".into(),
        "priorities" => format!("the priorities are fixed: {}", joined(PRIORITIES)),
        "default_priority" => format!("the default is fixed: {}", default_priority()),
        "resolutions" => format!("the resolutions are fixed: {}", joined(RESOLUTIONS)),
        _ => return None,
    })
}

/// Warnings for vocabulary keys a tracker still carries.
pub(crate) fn vestigial_warnings(cfg: &Config) -> Vec<String> {
    cfg.vestigial
        .iter()
        .filter_map(|k| vestigial_reason(k).map(|why| format!("config: '{k}' is no longer configurable and is being ignored ({why})")))
        .collect()
}

impl Config {
    /// Parse `trck.json` text. `path` names the file in any error.
    ///
    /// A malformed config is fatal rather than ignored: it is the one file that says
    /// which shape everything else is in.
    pub(crate) fn parse(text: &str, path: &str) -> Result<Config, String> {
        let trimmed = text.trim();
        let raw = if trimmed.is_empty() { Json::Object(Vec::new()) } else { parse(trimmed).map_err(|e| format!("{path}: invalid JSON ({e})"))? };
        let Json::Object(pairs) = &raw else {
            return Err(format!("{path}: expected a JSON object"));
        };
        let mut cfg = Config {
            format: raw.get("format").and_then(Json::as_i64),
            vestigial: pairs.iter().map(|(k, _)| k.clone()).filter(|k| vestigial_reason(k).is_some()).collect(),
            ..Config::default()
        };
        if let Some(update) = raw.get("update") {
            cfg.update_repo = update.get("repo").and_then(Json::as_str).map(str::to_string);
            cfg.update_channel = update.get("channel").and_then(Json::as_str).map(str::to_string);
        }
        cfg.raw = Some(raw);
        Ok(cfg)
    }

    pub(crate) fn update_repo(&self) -> &str {
        self.update_repo.as_deref().unwrap_or(DEFAULT_UPDATE_REPO)
    }

    /// Whether this engine understands the tracker — `None` when it does.
    ///
    /// Refuses a *newer* format and any extension it does not know; an older format is
    /// accepted, since refusing it would mean an engine could not run the migration
    /// that fixes it.
    pub(crate) fn check_format(&self) -> Option<String> {
        let Some(raw) = &self.raw else { return None };
        match raw.get("format") {
            None | Some(Json::Null) => {},
            Some(Json::Number(n)) if n.parse::<i64>().is_ok() => {},
            Some(other) => {
                return Some(format!("bad 'format' {} in trck.json (must be an integer)", py_repr_shallow(other)));
            },
        }
        if let Some(fmt) = self.format
            && fmt > SUPPORTED_FORMAT
        {
            return Some(format!(
                "tracker format {fmt} is newer than this engine understands \
                 (format {SUPPORTED_FORMAT}) — upgrade trck"
            ));
        }
        match raw.get("extensions") {
            None | Some(Json::Null) => None,
            Some(Json::Object(pairs)) => {
                let mut unknown: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).filter(|k| !KNOWN_EXTENSIONS.contains(k)).collect();
                unknown.sort_unstable();
                if unknown.is_empty() {
                    return None;
                }
                Some(format!("tracker uses unknown extension(s): {} — upgrade trck", unknown.join(", ")))
            },
            Some(other) => Some(format!("bad 'extensions' {} in trck.json (must be an object)", py_repr_shallow(other))),
        }
    }
}

/// Python's `repr` for the shapes that reach a config diagnostic, so the two engines
/// word the same failure the same way.
fn py_repr_shallow(v: &Json) -> String {
    match v {
        Json::Null => "None".into(),
        Json::Bool(true) => "True".into(),
        Json::Bool(false) => "False".into(),
        Json::Number(raw) => raw.clone(),
        Json::String(s) => format!("'{s}'"),
        Json::Array(items) => format!("[{}]", items.iter().map(py_repr_shallow).collect::<Vec<_>>().join(", ")),
        Json::Object(pairs) => format!("{{{}}}", pairs.iter().map(|(k, v)| format!("'{k}': {}", py_repr_shallow(v))).collect::<Vec<_>>().join(", ")),
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn the_vocabulary_is_fixed() {
        assert_eq!(STATUSES, ["backlog", "ongoing", "in-review", "done"]);
        assert_eq!(PRIORITIES, ["urgent", "high", "medium", "low", "lowest"]);
        assert_eq!(RESOLUTIONS, ["superseded", "wontfix", "duplicate"]);
        assert_eq!(default_priority(), "medium");
    }

    #[test]
    fn only_backlog_offers_work_to_pick_up() {
        let got: Vec<bool> = STATUSES.iter().map(|s| is_actionable(s)).collect();
        assert_eq!(got, [true, false, false, false]);
    }

    #[test]
    fn actionable_and_in_flight_partition_the_unfinished_statuses() {
        // Every status is exactly one of: available, held, finished. A fifth status added
        // to only one of the two predicates would land in neither and go missing.
        for s in STATUSES {
            let claims = usize::from(is_actionable(s)) + usize::from(is_in_flight(s)) + usize::from(is_terminal(s));
            assert_eq!(claims, 1, "{s} should be actionable, in flight or terminal — exactly one");
        }
    }

    #[test]
    fn the_started_statuses_are_the_ones_in_flight() {
        let got: Vec<bool> = STATUSES.iter().map(|s| is_in_flight(s)).collect();
        assert_eq!(got, [false, true, true, false]);
        assert!(!is_in_flight("wat"), "a hand-edited junk status is nobody's claim");
    }

    #[test]
    fn a_parent_rolls_up_from_its_children() {
        let s = |v: &[&str]| v.iter().map(|x| (*x).to_string()).collect::<Vec<_>>();
        assert_eq!(reconcile(&s(&["backlog", "backlog"])), "backlog");
        assert_eq!(reconcile(&s(&["done", "done"])), "done");
        assert_eq!(reconcile(&s(&["backlog", "done"])), "ongoing");
        assert_eq!(reconcile(&s(&["in-review"])), "ongoing");
    }

    #[test]
    fn checks_name_the_fixed_sets() {
        assert_eq!(check_status("done"), None);
        assert!(check_status("shipped").unwrap().contains("expected one of"));
        assert_eq!(check_priority("urgent"), None);
        assert!(check_priority("p0").unwrap().contains("urgent, high"));
        assert_eq!(check_resolution("wontfix"), None);
        assert!(check_resolution("fixed").unwrap().contains("superseded"));
        assert_eq!(check_points(0), None);
        assert!(check_points(-1).is_some());
    }

    #[test]
    fn review_urls_must_be_absolute_http() {
        assert_eq!(check_review_url("https://example.test/pull/1"), None);
        assert_eq!(check_review_url("http://e.test/1"), None);
        for bad in ["", "not a url", "example.com/pr/1", "ftp://x/y", "https://has space/x"] {
            assert!(check_review_url(bad).is_some(), "should reject {bad:?}");
        }
    }

    #[test]
    fn an_empty_or_absent_config_is_valid() {
        for text in ["", "   ", "{}"] {
            let cfg = Config::parse(text, "trck.json").expect("parses");
            assert_eq!(cfg.check_format(), None);
            assert_eq!(cfg.update_repo(), DEFAULT_UPDATE_REPO);
        }
    }

    #[test]
    fn an_absent_format_means_the_current_shape() {
        // Every tracker written before the key existed. Treating absence as "ancient,
        // refuse" would lock out all of them.
        let cfg = Config::parse("{}", "trck.json").expect("parses");
        assert_eq!(cfg.format, None);
        assert_eq!(cfg.check_format(), None);
    }

    #[test]
    fn a_newer_format_is_refused_and_names_the_fix() {
        let cfg = Config::parse(r#"{"format": 99}"#, "trck.json").expect("parses");
        let msg = cfg.check_format().expect("refused");
        assert!(msg.contains("newer than this engine"), "{msg}");
        assert!(msg.contains("upgrade trck"), "{msg}");
    }

    #[test]
    fn an_older_format_is_accepted() {
        // Refusing it would mean an engine could not run the migration that fixes it.
        let cfg = Config::parse(r#"{"format": 0}"#, "trck.json").expect("parses");
        assert_eq!(cfg.check_format(), None);
    }

    #[test]
    fn a_malformed_format_is_a_clean_error() {
        for bad in [r#"{"format": "1"}"#, r#"{"format": 1.5}"#, r#"{"format": true}"#] {
            let cfg = Config::parse(bad, "trck.json").expect("parses");
            let msg = cfg.check_format().expect("refused");
            assert!(msg.contains("must be an integer"), "{bad}: {msg}");
        }
    }

    #[test]
    fn an_unknown_extension_is_refused_and_every_one_is_named() {
        let cfg = Config::parse(r#"{"extensions": {"zeta": {}, "alpha": {}}}"#, "trck.json").expect("parses");
        let msg = cfg.check_format().expect("refused");
        assert!(msg.contains("alpha"), "{msg}");
        assert!(msg.contains("zeta"), "{msg}");
    }

    #[test]
    fn a_malformed_extensions_block_is_a_clean_error() {
        let cfg = Config::parse(r#"{"extensions": ["a"]}"#, "trck.json").expect("parses");
        assert!(cfg.check_format().expect("refused").contains("must be an object"));
    }

    #[test]
    fn leftover_vocabulary_keys_warn_rather_than_break() {
        let cfg = Config::parse(r#"{"statuses": [], "priorities": [], "kinds": [], "update": {"repo": "a/b"}}"#, "trck.json").expect("parses");
        let warns = vestigial_warnings(&cfg);
        assert_eq!(warns.len(), 3);
        assert!(warns.iter().all(|w| w.contains("no longer configurable")), "{warns:?}");
        assert_eq!(cfg.check_format(), None); // ignored, not fatal
        assert_eq!(cfg.update_repo(), "a/b");
    }

    #[test]
    fn invalid_json_names_the_file() {
        let err = Config::parse("{oops", "issues/trck.json").expect_err("rejects");
        assert!(err.starts_with("issues/trck.json: invalid JSON"), "{err}");
    }

    #[test]
    fn the_verb_aliases_are_constants() {
        assert_eq!(resolve_alias("start"), Some("ongoing"));
        assert_eq!(resolve_alias("review"), Some("in-review"));
        assert_eq!(resolve_alias("done"), Some("done"));
        assert_eq!(resolve_alias("nonesuch"), None);
    }
}
