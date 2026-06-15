# validate: suppress redundant bad-slug error when filename-slug mismatch already reported

## Summary
A malformed slug currently produces both a filename-slug-mismatch error and a bad-slug error for the same issue. Report just one.

## Acceptance criteria
- [ ] one error per malformed slug

## Notes
