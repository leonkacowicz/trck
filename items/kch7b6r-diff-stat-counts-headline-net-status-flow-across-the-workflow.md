# diff --stat: counts headline (net status flow across the workflow)

## Summary
One line summarising net movement across the configured statuses, plus creations and deletions:

```
backlog 24 → 21   ongoing 3 → 2   done 88 → 94      +2 new  −0 gone
```

Cheap to read, cheap to compute, and the right shape for a commit hook, a CI comment, or a PR
description. It says nothing about *which* issues moved — that is what the other layers are for.

Printed as the headline of every `trck diff` invocation; `--stat` suppresses the detail body and
prints only this.

## Acceptance criteria
- [ ] Columns come from the configured status list in order, not a hard-coded set; statuses with
      no change on either side are omitted so the line stays short.
- [ ] Counts reconcile: `Σnew − Σold == added − removed`.
- [ ] Backward movement is visible rather than netted away (at minimum a `↩N reopened` tail when
      any transition is backward).
- [ ] `--stat` exits after the headline; the headline also appears above the default layout.

## Notes
- Depends on the change model (direction classification) from the foundation issue.
- Worth also considering a points/progress tail (`points 42 → 51 done`) once the rollup layer lands
  — same rollup helper, one more number. Keep it out of scope here unless it falls out for free.
