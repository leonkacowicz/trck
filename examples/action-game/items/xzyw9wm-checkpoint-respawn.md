# Checkpoint & respawn

## Summary
Checkpoint flags and respawn-on-death: returning the player to the last checkpoint with
health restored.

## Acceptance criteria
- [ ] Touching a checkpoint records a respawn point
- [ ] Death respawns at the last checkpoint via #7qax2vu's death event
- [ ] Checkpoints persist within a play session

## Notes
Depends on the health/death system #7qax2vu. Feeds the save system #nmde6ad.
