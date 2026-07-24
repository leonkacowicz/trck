from __future__ import annotations
import sys
from .graph import Graph, normalize_points, normalize_statuses
from .index import Ctx, Issue, save_index
from .scan import validate
from .summary import write_summary

# --------------------------------------------------------------------------- #
# finalize: persist + regenerate + validate after every mutation
# --------------------------------------------------------------------------- #
def finalize(ctx: Ctx, rows: list[Issue]) -> None:
    g = Graph(ctx.cfg, rows)
    normalize_points(g)
    normalize_statuses(ctx, g)
    save_index(ctx, rows)
    write_summary(ctx)
    errors, warnings = validate(ctx, rows)  # reuse the rows we just wrote
    for w in warnings:
        print(f"warning: {w}", file=sys.stderr)
    if errors:
        print("\nINCONSISTENCIES after this operation:", file=sys.stderr)
        for e in errors:
            print(f"  error: {e}", file=sys.stderr)
        print("the tracker is now inconsistent — fix before committing.", file=sys.stderr)


