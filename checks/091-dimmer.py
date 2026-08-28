"""L2 and the d-pad left make it darker."""

FEATURE = "brightness"
SINCE = "2026-08-26"


def here(pad, seen):
    pad.trigger("l2", 1.0)
    pad.press("dpad-left")
    seen.settle()
    assert seen.commands() == [["/usr/local/bin/legion-brightness", "down"]], \
        "it ran %s" % seen.commands()


def device(pad, seen):
    """The floor is a clamp as much as the ceiling is.

    So room is made above before the screen is asked to fall, and given back
    afterwards, before the assertion rather than after it, so that a screen
    somebody is reading by is left alone whichever way this ends.

    Both numbers are said. "still at 64000" was the value from before the
    press, which made a clamp and a press that never arrived read alike, and
    two checks went undiagnosed on it for days.
    """
    pad.trigger("l2", 1.0)
    pad.press("dpad-right")
    seen.settle(1.0)
    was = seen.brightness()
    pad.press("dpad-left")
    seen.settle(1.0)
    now = seen.brightness()
    pad.press("dpad-right")
    seen.settle(1.0)
    pad.trigger("l2", 0.0)
    assert now < was, "it was %d and is %d" % (was, now)
