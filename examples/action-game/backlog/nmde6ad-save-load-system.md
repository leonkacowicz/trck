# Save / load system

## Summary
Save / load system: serialise progress (current level, checkpoint, unlocks, settings) and
restore it on launch.

## Acceptance criteria
- [ ] Saves and restores level, checkpoint, and unlocks
- [ ] Handles a missing/corrupt save gracefully
- [ ] Uses the format chosen in #031

## Notes
Depends on the format decision #031 and on checkpoints #024 (the main thing worth saving).
