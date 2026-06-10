# Dash & wall-jump

## Summary
Air-dash (8-direction) and wall-jump. Dash has a fixed distance and a short cooldown; wall-
jump kicks off vertical surfaces and briefly locks horizontal input so it reads cleanly.

## Acceptance criteria
- [ ] Dash travels a fixed distance and respects cooldown
- [ ] Wall-slide slows descent; wall-jump launches away from the wall
- [ ] Dash grants i-frames (tracked separately in #014)

## Notes
Depends on the base controller #007. The i-frame behaviour is buggy — see #014. Wall
collision handling here absorbed the old slope-jitter report #034.
