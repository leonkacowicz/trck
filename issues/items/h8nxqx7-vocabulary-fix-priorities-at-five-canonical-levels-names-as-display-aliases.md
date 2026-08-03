# vocabulary: fix priorities at five canonical levels, names as display aliases

## Summary
Five priorities cover the cases that matter, and fixing the count turns the demand vector from a
config-length structure into a fixed-width one. But "five priorities" bundles two decisions, and
only one should be fixed.

**Fix the count and the ordering. Allow the names.** Teams say P0–P4, or S1–S4, or
critical/major/minor. That is pure display — no branching, no validation surface, no semantic
variation — and it is the most common reason someone bounces off a tool with opinions.

The argument for keeping names configurable is the same one that motivates fixing everything
else: store the canonical value, show the alias. Two trackers with free-form priorities cannot
be compared at all; canonical storage plus display aliases is what makes cross-tracker tooling
possible.

## Acceptance criteria
- [ ] Five canonical levels with fixed ordering, defined in code rather than config.
- [ ] `trck.json` may map each to a display name; absent means the canonical name.
- [ ] `index.jsonl` stores the canonical value only. Aliases never reach disk.
- [ ] The CLI accepts either the canonical name or the configured alias wherever a priority is
      taken, and prints the alias.
- [ ] Demand ranking is unchanged in behaviour, but keyed to fixed slots.
- [ ] Migration is covered by `rbast9r`; this issue only lands the model.

## Notes
Must land before the conformance goldens are frozen, or every priority-bearing fixture churns.
