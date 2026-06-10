# Damage & health system

## Summary
Central damage and health model: hit points, damage application, invulnerability windows,
and death. Both the player and enemies route through it.

## Acceptance criteria
- [ ] Damage events reduce HP and trigger hit-stun
- [ ] Death fires an event other systems can subscribe to
- [ ] Supports per-entity max-HP and resistances

## Notes
In progress. Shared dependency for checkpoints #024 and the HUD #029.
