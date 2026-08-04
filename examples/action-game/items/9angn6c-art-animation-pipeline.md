# Art & animation pipeline

## Summary
Epic for the 2D art toolchain and all sprite/tile assets: the atlas build step plus the
character, enemy, and environment art that ships in the game.

## Acceptance criteria
- [ ] Atlas tooling #adejzqs is in place and used by every asset task
- [ ] Player #fb8zz9c and enemy #fd34m5t sprite sheets are imported
- [ ] Forest #r4c8ajk and cave #sr4c37v tilesets are imported

## Notes
The atlas tool #adejzqs is the upstream dependency for every other art task — see the fan-out in
`deps --graph`.
