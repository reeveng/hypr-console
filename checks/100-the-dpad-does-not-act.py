"""The d-pad on its own moves between things and does nothing else."""

FEATURE = "dpad"
SINCE = "2026-08-26"


def here(pad, seen):
    for button in ("dpad-up", "dpad-down", "dpad-left", "dpad-right"):
        pad.press(button)
    seen.settle()
    assert seen.commands() == [], "the d-pad ran %s" % seen.commands()


def device(pad, seen):
    """On the desktop the d-pad is the arrow keys, which move a selection
    inside whatever has focus. What it must not do is move the desktop."""
    here, windows = seen.workspace(), seen.windows()
    for button in ("dpad-up", "dpad-down", "dpad-left", "dpad-right"):
        pad.press(button)
    seen.settle(1.2)
    assert seen.workspace() == here, "the d-pad moved the desktop to %s" \
        % seen.workspace()
    assert seen.windows() == windows, "the d-pad opened or closed something"
