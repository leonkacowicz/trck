# validate: report one error per parent cycle, not one per node

## Summary
`validate` emits an "in a parent cycle" error for every node in a cycle; collapse to one error per distinct cycle for clearer output.

## Acceptance criteria
- [ ] one error per distinct parent cycle

## Notes
Inherited from the original `track` tool's design.
