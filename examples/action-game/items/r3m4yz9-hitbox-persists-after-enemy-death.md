# Hitbox persists after enemy death

## Summary
Bug: an enemy's hitbox lingers for a few frames after death, so a corpse can still deal
contact damage.

## Acceptance criteria
- [ ] Hitbox is disabled on the death event from #012
- [ ] No contact damage after an enemy's death animation starts
- [ ] Covered by a test

## Notes
Tagged tech-debt — the death path in the AI #013 never disabled the collider. Depends on
enemy AI #013.
