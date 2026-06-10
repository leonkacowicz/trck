# Ranged / projectile attacks

## Summary
Ranged attack: a chargeable projectile with travel time, a max range, and a small
ammo/energy cost.

## Acceptance criteria
- [ ] Tap fires a quick shot; hold charges a stronger one
- [ ] Projectiles despawn at max range or on hit
- [ ] Ammo/energy is consumed and regenerates

## Notes
Depends on melee #010 so both share the same attack-state machine. Labelled needs-design —
charge curve and ammo economy are unresolved.
