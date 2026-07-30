# Combat system

## Summary
Sub-epic (nested under #001) covering the offensive/defensive combat loop: melee, ranged,
the damage/health model, and enemy reactions to being hit.

## Acceptance criteria
- [ ] Melee #010, ranged #011, and health #012 are shipped
- [ ] Enemy AI #013 reacts to all attack types
- [ ] No open combat bugs (#014, #015 resolved)

## Notes
Demonstrates a three-level hierarchy: #001 → #002 → leaves. Health system #012 is the shared
dependency for HUD #029 and checkpoints #024.
