"""L1 moves to the workspace before."""

FEATURE = "workspaces"
SINCE = "2026-08-24"


def here(pad, seen):
    pad.press("l1")
    seen.settle()
    assert seen.dispatches() == ['hl.dsp.focus({workspace = "-1"})'], \
        "L1 asked for %s" % seen.dispatches()


def device(pad, seen):
    was = seen.workspace()
    pad.press("l1")
    seen.settle()
    there = seen.workspace()
    pad.press("r1")
    seen.settle()
    assert there != was, "L1 left us on %s" % was
