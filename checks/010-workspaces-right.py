"""R1 moves to the next workspace."""

FEATURE = "workspaces"
SINCE = "2026-08-24"


def here(pad, seen):
    pad.press("r1")
    seen.settle()
    assert seen.dispatches() == ['hl.dsp.focus({workspace = "+1"})'], \
        "R1 asked for %s" % seen.dispatches()


def device(pad, seen):
    was = seen.workspace()
    pad.press("r1")
    seen.settle()
    assert seen.workspace() != was, "still on workspace %s" % was
