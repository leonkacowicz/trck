//! An index built to be hostile, and the summary it renders to.
//!
//! Both live here rather than in either test module because they are one fixture: the summary
//! golden is only meaningful as the rendering of *this* index, and a copy of the index in the
//! other module is a copy that drifts.
//!
//! # Why this and not the real tracker
//!
//! Two tests used to read this repository's own committed `index.jsonl` and require the bytes
//! back. The appeal was breadth — hundreds of real rows carrying shapes nobody would sit down
//! and invent. Counting them says otherwise. Across 291 rows of it: **zero** titles containing
//! `"`, **zero** containing `\`, **zero** containing a newline, **zero** astral-plane
//! characters, one containing `[`/`]` and one containing `|`, both by accident, and 26
//! non-ASCII titles that are all em dashes.
//!
//! The two characters a JSON serialiser can actually break on appeared *never*. Real data is
//! arbitrary, not adversarial: it holds what someone happened to type, which is a different set
//! from what pins a serialiser. It was also live and mutable — a fixture that changed whenever
//! anybody filed an issue — and a failure would have meant "run `repo normalize`" rather than
//! "there is a bug".
//!
//! So the breadth is reproduced deliberately below, in a fixture that cannot change underfoot
//! and that fails only for a reason worth acting on. What it does *not* replace is the question
//! of whether the committed tracker is canonical; that is a check on data, it belongs in the
//! tracker branch's own workflow, and it is `#fgsetxs`.
//!
//! # Reading it
//!
//! Every line is already in canonical form — known keys in `CANON_KEYS` order, defaults
//! omitted, extras last and sorted — because the claim under test is that rendering what was
//! parsed reproduces the input byte for byte. A line that is merely *valid* would round-trip to
//! something else and prove nothing. Rows are in id order for the same reason:
//! [`crate::index::render_index`] sorts, so an unsorted fixture would fail on the ordering
//! before it reached anything interesting.
//!
//! Written as Rust raw strings, so what appears here is what the file would contain: `\"` is a
//! backslash followed by a quote, and JSON's escapes stay legible instead of being doubled.

/// The fixture, one row per shape worth pinning.
///
/// - `aaaaaaa` — the escapes, and only this row would notice if the encoder regressed. A title
///   carrying `"`, `\`, a newline, a tab, and `\u0007` for a C0 control with no short form —
///   which is the whole of what must be escaped. `/` sits beside them **unescaped**: escaping
///   it is legal JSON and a byte-level mismatch, so its presence is an assertion too.
/// - `bbbbbbb` — markdown's structural characters in a title: `[`, `]`, `|`, backtick,
///   underscore, asterisk. `SUMMARY.md` interpolates titles into link text and table cells
///   without escaping them, so this is the row that would show it if that ever mattered.
/// - `ccccccc` — text beyond the BMP (an Old Italic letter and an emoji, a surrogate pair each
///   in UTF-16 terms) and a wide CJK character, so a length or slicing bug in either renderer
///   has something to trip on.
/// - `ddddddd` — the epic. `eeeeeee` sits under it and `fffffff` under that, so the rollup
///   recurses rather than counting one level.
/// - `eeeeeee` — every optional field at once: a non-default `points`, `parent`, `labels`,
///   `depends_on`, `spec`, `review_url`, all four timestamps, `resolution`, `manual_status`,
///   and two extras whose keys sort after the known ones. If canonical *order* ever changed,
///   this is the row that would say so. It is also `done` while its child is open, which only
///   `manual_status` makes legal — and which the rollup has to render as it stands rather than
///   as derivation would have it.
/// - `fffffff` — the minimum: identity fields and a parent, nothing else. What proves the
///   defaults are omitted on the way out rather than merely absent on the way in. An empty
///   `labels` list is *not* representable — canonical form strips one — so absent is the only
///   spelling there is, and a fixture claiming otherwise would never round-trip.
/// - `ggggggg` — `points` of 0, kept because the default is 1 and the test is
///   equals-the-default rather than falsiness.
pub(crate) const HOSTILE_INDEX: &str = concat!(
    r#"{"id": "aaaaaaa", "slug": "escapes", "title": "quote \" backslash \\ newline \n tab \t control \u0007 slash /", "status": "backlog", "priority": "urgent"}"#,
    "\n",
    r#"{"id": "bbbbbbb", "slug": "markdown", "title": "link [text](url) | pipe `code` _under_ *star*", "status": "in-progress", "priority": "high"}"#,
    "\n",
    r#"{"id": "ccccccc", "slug": "astral", "title": "astral 𐌀 emoji 🧪 and a wide 漢字", "status": "in-review", "priority": "medium", "review_url": "https://example.invalid/pr/1"}"#,
    "\n",
    r#"{"id": "ddddddd", "slug": "epic", "title": "The epic — em dash and all", "status": "in-progress", "priority": "high", "spec": "docs/specs/thing.md"}"#,
    "\n",
    r#"{"id": "eeeeeee", "slug": "every-field", "title": "Every optional field at once", "status": "done", "priority": "low", "points": 0, "parent": "ddddddd", "labels": ["one", "two-hyphen"], "depends_on": ["aaaaaaa", "bbbbbbb"], "spec": "docs/specs/other.md", "review_url": "https://example.invalid/pr/2", "created": "2026-01-01T00:00:00Z", "started": "2026-01-02T09:30:00Z", "closed": "2026-01-03T17:45:00Z", "resolution": "superseded", "manual_status": true, "assignee": "someone", "z-sorts-last": "after assignee"}"#,
    "\n",
    r#"{"id": "fffffff", "slug": "minimal", "title": "Nothing but the identity fields and a parent", "status": "backlog", "priority": "lowest", "parent": "eeeeeee"}"#,
    "\n",
    r#"{"id": "ggggggg", "slug": "zero-points", "title": "Weightless on purpose", "status": "backlog", "priority": "medium", "points": 0}"#,
    "\n",
);

/// What [`HOSTILE_INDEX`] renders to, as a file rather than a string literal.
///
/// A golden is read far more often than it is written, and the thing under review is a
/// *document*: section order, blank lines, table alignment, and what an unescaped title does to
/// a markdown link. As a `concat!` of escaped lines none of that is visible and a diff of it is
/// unreadable. `include_str!` keeps the review honest at the cost of one file.
///
/// The file is confronting in places, deliberately. `#aaaaaaa`'s title carries a literal
/// newline and tab, so the list item it lands in spans two lines and contains a control
/// character; `#bbbbbbb`'s carries `[`, `]` and `|`, so its link text is malformed markdown.
/// That is what the renderer does today — it interpolates titles without escaping them — and
/// freezing it is the point. If it should change, this file is where the decision becomes
/// visible, which is where it belongs.
pub(crate) const HOSTILE_SUMMARY: &str = include_str!("test_index_summary.md");
