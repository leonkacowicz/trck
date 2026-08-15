# serve: poll and fast-forward the tracker ref on a timer

## Summary
A served page must not go stale. A background loop fetches the tracker ref, applies the
ahead / equal / behind / diverged rule from #abynj5c, and fast-forwards the local ref when it is
behind.

This is a **deliberate exception** to #sqzr7nk's rule that reads must not auto-fetch. That rule
is about verbs in a pipeline, where a network round trip on every read is unacceptable. `serve`
is a long-lived process with a timer, and a page left open on a week-old ref is the time-travel
bug in a new costume.

## Acceptance criteria
- [ ] A timer fetches the tracker ref on a configurable interval with a sane default.
- [ ] Behind → fast-forward the local ref and re-read. Ahead or equal → do nothing. Diverged →
      surface it; never resolve silently.
- [ ] No parsed index is held across requests, or it is cached keyed on the ref SHA. Another
      `trck` in another terminal moving the local ref is normal for this process and impossible
      for every other verb — a stale in-memory index is the bug this criterion exists to prevent.
- [ ] A fetch failure (offline, no remote) degrades to serving the local ref and says so. It does
      not kill the process.
- [ ] The auto-fetch exception is written down where the no-auto-fetch rule is stated, not only
      in this issue.

## Notes
Pushing the refresh to open pages is the next child. This one only has to make the state correct
and observable.
