//! Turning a title into a filename component.

/// A filesystem-safe slug from a title: lowercase, runs of non-alphanumerics collapsed
/// to a single dash, trimmed.
pub(crate) fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c.to_ascii_lowercase());
        } else {
            // Everything else becomes a dash — including non-ASCII alphanumerics, which are
            // not filesystem-safe across platforms and which Python's slugify drops too.
            pending_dash = true;
        }
    }
    out
}

/// Whether a slug is usable as a filename component.
pub(crate) fn check_slug(slug: &str) -> bool {
    let mut chars = slug.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn slugify_matches_the_python_rule() {
        assert_eq!(slugify("Fix the parser"), "fix-the-parser");
        assert_eq!(slugify("  Leading & trailing!  "), "leading-trailing");
        assert_eq!(slugify("CamelCase123"), "camelcase123");
        assert_eq!(slugify("---"), "");
        assert_eq!(slugify("café ✓"), "caf");
    }

    /// A trailing run of separators leaves no trailing dash, and a leading one no leading
    /// dash — either would make a filename that `check` then refuses as a bad slug.
    #[test]
    fn no_slug_starts_or_ends_with_a_dash() {
        for title in ["!leading", "trailing!", "!both!", "a - b", "--x--"] {
            let s = slugify(title);
            assert!(!s.starts_with('-') && !s.ends_with('-'), "{title:?} -> {s:?}");
            assert!(s.is_empty() || check_slug(&s), "{title:?} -> {s:?} is not a usable slug");
        }
    }

    /// Whatever `slugify` produces must be something `check_slug` accepts, or `new` would
    /// write a file its own `check` rejects.
    #[test]
    fn slugify_output_always_passes_check_slug() {
        for title in ["Fix the parser", "CamelCase123", "a1", "Ünïcödé wörds", "123 numbers"] {
            let s = slugify(title);
            assert!(s.is_empty() || check_slug(&s), "{title:?} -> {s:?}");
        }
    }

    #[test]
    fn check_slug_rejects_what_a_filename_cannot_carry() {
        assert!(check_slug("fix-the-parser"));
        assert!(check_slug("a1"));
        assert!(!check_slug(""));
        assert!(!check_slug("-leading"));
        assert!(!check_slug("Upper"));
        assert!(!check_slug("has space"));
        assert!(!check_slug("under_score"));
    }
}
