//! One index row's key/value pairs, while it is being read.
//!
//! A thin wrapper over the pairs rather than a map, for the same reason [`crate::json`] keeps
//! objects ordered: the migration needs to *remove* a key and *add* another, and the reader
//! needs to know which keys were left over.

use super::CANON_KEYS;
use crate::json::Json;
use std::collections::BTreeMap;

/// Later duplicates collapsed the way Python's `json.loads` collapses them: the last one
/// wins, and only one copy remains.
pub(super) struct Row(Vec<(String, Json)>);

impl Row {
    pub(super) fn new(pairs: &[(String, Json)]) -> Row {
        let mut out: Vec<(String, Json)> = Vec::new();
        for (k, v) in pairs {
            if let Some(slot) = out.iter_mut().find(|(existing, _)| existing == k) {
                slot.1 = v.clone();
            } else {
                out.push((k.clone(), v.clone()));
            }
        }
        Row(out)
    }

    pub(super) fn get(&self, key: &str) -> Option<&Json> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// The value, treating an explicit `null` as absent — which is what every optional
    /// field here means by it.
    pub(super) fn present(&self, key: &str) -> Option<&Json> {
        match self.get(key) {
            Some(Json::Null) | None => None,
            v => v,
        }
    }

    /// A required field's value, or `Json::Null` when it is absent — which the coercions
    /// then reject by type. Nothing reaches here that
    /// [`super::read::require_present`] has not already accepted.
    pub(super) fn required(&self, key: &str) -> Json {
        self.get(key).cloned().unwrap_or(Json::Null)
    }

    pub(super) fn take(&mut self, key: &str) -> Option<Json> {
        self.0.iter().position(|(k, _)| k == key).map(|i| self.0.remove(i).1)
    }

    pub(super) fn set(&mut self, key: &str, value: Json) {
        self.0.push((key.to_string(), value));
    }

    /// Whatever this engine has never heard of, sorted so the canonical form is stable.
    ///
    /// This is the forward-compatibility guarantee: a key it cannot interpret is still a key
    /// it hands back, so adding a field never makes an older engine *wrong*, only ignorant.
    pub(super) fn into_extra(self) -> BTreeMap<String, Json> {
        self.0.into_iter().filter(|(k, _)| !CANON_KEYS.contains(&k.as_str())).collect()
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn row(pairs: &[(&str, Json)]) -> Row {
        Row::new(&pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect::<Vec<_>>())
    }

    /// The last duplicate wins, and only one copy survives — so `into_extra` cannot emit the
    /// same unknown key twice and make a row that no longer parses.
    #[test]
    fn a_duplicate_key_collapses_to_the_last_value() {
        let r = row(&[("a", Json::Number("1".into())), ("a", Json::Number("2".into()))]);
        assert_eq!(r.get("a"), Some(&Json::Number("2".into())));
        assert_eq!(r.into_extra().len(), 1);
    }

    #[test]
    fn present_treats_an_explicit_null_as_absent() {
        let r = row(&[("a", Json::Null), ("b", Json::Number("1".into()))]);
        assert_eq!(r.present("a"), None, "null means unset");
        assert_eq!(r.get("a"), Some(&Json::Null), "but it was there");
        assert!(r.present("b").is_some());
    }

    #[test]
    fn required_substitutes_null_for_a_missing_key() {
        assert_eq!(row(&[]).required("nope"), Json::Null);
    }

    #[test]
    fn take_removes_and_set_appends() {
        let mut r = row(&[("pr", Json::String("u".into()))]);
        assert_eq!(r.take("pr"), Some(Json::String("u".into())));
        assert_eq!(r.take("pr"), None, "removed, not just read");
        r.set("review_url", Json::String("u".into()));
        assert_eq!(r.get("review_url"), Some(&Json::String("u".into())));
    }

    /// Known keys are not extras, whatever order they arrived in.
    #[test]
    fn into_extra_keeps_only_the_unknown_keys() {
        let r = row(&[("id", Json::String("a".into())), ("zz", Json::Number("1".into())), ("aa", Json::Bool(true))]);
        let extra = r.into_extra();
        assert_eq!(extra.keys().collect::<Vec<_>>(), ["aa", "zz"], "sorted, and 'id' excluded");
    }
}
