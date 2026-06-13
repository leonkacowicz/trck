# Art & animation pipeline

## Summary
Epic for the 2D art toolchain and all sprite/tile assets: the atlas build step plus the
character, enemy, and environment art that ships in the game.

## Acceptance criteria
- [ ] Atlas tooling #016 is in place and used by every asset task
- [ ] Player #017 and enemy #018 sprite sheets are imported
- [ ] Forest #019 and cave #020 tilesets are imported

## Notes
The atlas tool #016 is the upstream dependency for every other art task — see the fan-out in
`deps --graph`.
