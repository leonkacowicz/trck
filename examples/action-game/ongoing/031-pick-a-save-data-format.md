# Pick a save-data format

## Summary
Investigation: choose the on-disk save format. Compare JSON (human-readable, easy to debug)
vs. a compact binary format (smaller, tamper-resistant), considering versioning/migration.

## Acceptance criteria
- [ ] Decision recorded with rationale
- [ ] Migration/versioning strategy sketched
- [ ] Spike proves round-tripping a sample save

## Notes
In progress, needs-design. Blocks the actual save/load implementation #032. Leaning JSON for
v1 simplicity.
