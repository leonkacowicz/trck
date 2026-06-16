# Dash i-frames not applied

## Summary
Bug: the invulnerability frames that a dash is supposed to grant are not being applied, so
players take damage mid-dash. Reproduces 100% when dashing through a projectile.

## Acceptance criteria
- [ ] Dash grants i-frames for its full active duration
- [ ] Damage taken during i-frames is ignored
- [ ] Add a regression test for dash-through-projectile

## Notes
Urgent. Root cause looks like the i-frame flag being cleared a frame too early in #008's
state machine. Depends on dash #008.
