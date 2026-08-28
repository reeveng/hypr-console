"""View goes back to the workspace you were on."""

FEATURE = "previous"
SINCE = "2026-08-25"


def here(pad, seen):
    pad.press("view")
    seen.settle()
    assert seen.dispatches() == ['hl.dsp.focus({workspace = "previous"})'], \
        "it asked for %s" % seen.dispatches()


def device(pad, seen):
    here = seen.workspace()
    pad.press("r1")
    seen.settle(1.0)
    away = seen.workspace()
    assert away != here, "R1 did not move, so there is nothing to go back from"
    pad.press("view")
    seen.settle(1.0)
    assert seen.workspace() == here, \
        "View left us on %s rather than %s" % (seen.workspace(), here)
