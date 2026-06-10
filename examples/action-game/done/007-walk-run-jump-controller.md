# Walk / run / jump controller

## Summary
Ground locomotion: walking, running (with acceleration), and a variable-height jump with
coyote time and a short jump buffer. This is the base every other movement and combat task
builds on.

## Acceptance criteria
- [x] Walk and run have distinct top speeds with smooth acceleration
- [x] Jump height scales with how long the button is held
- [x] Coyote time (~80ms) and jump buffering (~100ms) are tuned

## Notes
Shipped. Replaces the throwaway prototype #033. Everything in #001/#002 depends on this
controller.
