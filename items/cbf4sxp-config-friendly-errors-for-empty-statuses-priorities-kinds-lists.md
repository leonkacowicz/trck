# config: friendly errors for empty statuses/priorities/kinds lists

## Summary
An empty `statuses` list causes `initial_status` to raise `IndexError`; empty `priorities`/`kinds` produce unfriendly failures. Validate config on load and `die` with clear messages.

## Acceptance criteria
- [ ] friendly `die` on empty `statuses`, `priorities`, or `kinds`

## Notes
