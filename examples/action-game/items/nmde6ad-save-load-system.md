# Save / load system

## Summary
Save / load system: serialise progress (current level, checkpoint, unlocks, settings) and
restore it on launch.

## Acceptance criteria
- [ ] Saves and restores level, checkpoint, and unlocks
- [ ] Handles a missing/corrupt save gracefully
- [ ] Uses the format chosen in #h9chxqu

## Notes
Depends on the format decision #h9chxqu and on checkpoints #xzyw9wm (the main thing worth saving).
