//! Issue ids: the alphabet, the length, validation, and generation.
//!
//! Ids are opaque strings, seven characters from a base32 alphabet with the
//! look-alikes (`0`, `1`, `o`, `l`, `i`) removed. Random rather than sequential
//! because two branches both running `trck new` used to mint the same number and
//! conflict.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

/// Base32 minus the characters that get misread. Lowercase, for typeability.
pub(crate) const ALPHABET: &str = "23456789abcdefghjkmnpqrstuvwxyz";
/// 31^7 ≈ 2.75e10 ids.
pub(crate) const LEN: usize = 7;

/// Whether a hand-supplied id is well formed — `None` when it is, else a message.
///
/// Both halves are load-bearing. The alphabet excludes characters that are misread, so
/// an id containing one is a typo waiting to be pasted wrong; and the length is fixed
/// because the CLI resolves ids by unique prefix, which a short id would make ambiguous
/// against every longer one sharing its start.
pub(crate) fn check(value: &str) -> Option<String> {
    if value.is_empty() || !value.chars().all(|c| ALPHABET.contains(c)) {
        return Some(format!("bad id '{value}' (must use the alphabet {ALPHABET})"));
    }
    if value.chars().count() != LEN {
        return Some(format!("bad id '{value}' (must be exactly {LEN} characters)"));
    }
    None
}

/// A fresh random id, avoiding anything in `taken`.
///
/// The randomness comes from `RandomState`, which std seeds from the operating system.
/// That is a deliberate choice over `/dev/urandom` or a platform crate: the engine takes
/// no dependencies and should carry no `#[cfg(unix)]` fork for something this small. Each
/// call builds a fresh state and hashes a counter through it, so consecutive ids do not
/// share structure.
pub(crate) fn generate(taken: &dyn Fn(&str) -> bool) -> String {
    let alphabet: Vec<char> = ALPHABET.chars().collect();
    let mut counter: u64 = 0;
    loop {
        let mut bits = {
            let mut h = RandomState::new().build_hasher();
            h.write_u64(counter);
            h.finish()
        };
        counter = counter.wrapping_add(1);
        let mut id = String::with_capacity(LEN);
        for _ in 0..LEN {
            // 31 is not a power of two, so the low bits are very slightly biased. The
            // id space is ~2.75e10 and the only thing riding on it is collision
            // avoidance, which the `taken` check backstops.
            let idx = usize::try_from(bits % alphabet.len() as u64).unwrap_or(0);
            id.push(alphabet[idx]);
            bits /= alphabet.len() as u64;
        }
        if !taken(&id) {
            return id;
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use std::collections::HashSet;

    #[test]
    fn accepts_a_well_formed_id() {
        assert_eq!(check("k3m9x2a"), None);
        assert_eq!(check("2345678"), None); // all-digit ids are ordinary
    }

    #[test]
    fn rejects_look_alikes_wrong_length_and_case() {
        for bad in ["short", "waytoolongid", "aaaaaa0", "aaaaaa1", "aaaaaao", "AAAAAAA", ""] {
            assert!(check(bad).is_some(), "should reject {bad:?}");
        }
    }

    #[test]
    fn generated_ids_match_the_alphabet_and_length() {
        for _ in 0..200 {
            let id = generate(&|_| false);
            assert_eq!(check(&id), None, "generated {id:?}");
        }
    }

    #[test]
    fn generated_ids_are_distinct() {
        let ids: HashSet<String> = (0..500).map(|_| generate(&|_| false)).collect();
        // A handful of collisions in 500 draws from 2.75e10 would mean the generator is
        // not actually varying between calls, which is the failure mode worth catching.
        assert_eq!(ids.len(), 500);
    }

    #[test]
    fn generation_avoids_taken_ids() {
        let mut seen: HashSet<String> = HashSet::new();
        for _ in 0..50 {
            let id = generate(&|c| seen.contains(c));
            assert!(seen.insert(id));
        }
    }
}
