# Enemy AI: patrol & aggro

## Summary
Basic enemy AI: patrol a path, detect the player within a vision cone, switch to
aggro/chase, and return to patrol when the player escapes.

## Acceptance criteria
- [ ] Enemies patrol between waypoints
- [ ] Vision cone triggers aggro; line-of-sight is required
- [ ] Enemies disengage and reset after losing the player

## Notes
Depends on melee #010 (needs something to react to). Blocks the death-hitbox bug #015.
