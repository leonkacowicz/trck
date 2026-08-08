#!/usr/bin/env python3
"""Decide whether a change has to run the engine's suites.

    git diff --no-renames --name-only "origin/$BASE...HEAD" | python3 scripts/ci_changed.py
    # -> "true" or "false" on stdout, nothing else

A pull request that only edits tracker data or prose cannot affect the engine, so CI
skips the build/test matrix, the conformance suite, the quality ratchet and the helper
scripts for it. This script is the whole of that decision, kept out of the workflow file
because a path glob buried in YAML is unreviewable and untestable — and the way this
fails is silent. A rule that wrongly says "skippable" does not turn a check red; it makes
the checks green by never running them.

Two rules follow from that.

**It is an allowlist.** Only `issues/`, `docs/` and repository-root markdown are
skippable; everything else is code. A denylist keyed on `.md` would be wrong in this
repository, where markdown is also a compiled-in asset (`assets/`), a conformance fixture
(`conformance/`), an example tracker (`examples/`) and the shipped skill (`skills/`) —
all of which the engine or its specification reads.

**Every uncertainty resolves to code.** An empty diff, a diff of nothing but blank lines,
a path shape not accounted for: all of them run everything. The cost of a needless CI run
is minutes; the cost of a needless skip is a merge nobody built.

Standard library only, and it imports nothing from the engine — it runs on a checkout
where the engine does not build.
"""
import sys

# Directories whose entire contents are inert to the engine. Trailing slash is load-bearing:
# it is what stops `issues-archive/` from matching `issues/`.
SKIPPABLE_DIRS = ("issues/", "docs/")


def is_skippable(path: str) -> bool:
    """Whether this one path is inert to the engine.

    Repository-root markdown (README, CONTRIBUTING, CLAUDE) is prose about the project.
    Markdown deeper in the tree is not automatically prose, so only the root is granted.
    """
    if path.startswith(SKIPPABLE_DIRS):
        return True
    return "/" not in path and path.endswith(".md")


def needs_full_ci(paths) -> bool:
    """Whether this changeset must run the engine's suites.

    True unless every changed path is skippable — and true for a changeset with no paths
    at all, which means the diff could not be read rather than that nothing changed.
    """
    changed = [p.strip() for p in paths]
    changed = [p for p in changed if p]
    if not changed:
        return True
    return any(not is_skippable(p) for p in changed)


def main() -> int:
    print("true" if needs_full_ci(sys.stdin.read().splitlines()) else "false")
    return 0


if __name__ == "__main__":
    sys.exit(main())
