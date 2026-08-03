# vocabulary: fix priorities at five canonical levels

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

## Outcome

`PRIORITIES` and `DEFAULT_PRIORITY` are constants; `priorities` and `default_priority` are gone
from `trck.json`, which now holds only `update` and `resolutions`.

**The display-aliases criterion is struck.** It was written before the statuses collapsed, and it
is exactly what that collapse deleted: two words for one concept, in every message, doc and
conversation. `P0`-`P4` is admittedly more common house vocabulary than renaming `done` is, so
this is the strongest case for an exception — and it still is not strong enough to be
inconsistent over. With it struck, "store canonical, show alias" collapses to "store the value",
and the CLI has nothing extra to accept.

No rows were rewritten. The five canonical names are what every tracker already stores, here and
in the bundled example.

`priority_rank` keeps its trailing bucket for an unrecognised value, and `demand_vector` keeps the
matching slot. Validation rejects a bad priority on the way in, so the only route left is a hand
edit — which should sink to the bottom of the ranking, not throw.

Landed with the statuses work in the same sweep: `86e5a80` demoted `kind`, and this is the last
of the three vocabularies.
