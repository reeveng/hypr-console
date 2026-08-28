"""L2 and the d-pad right make the screen brighter."""

FEATURE = "brightness"
SINCE = "2026-08-26"


def here(pad, seen):
    pad.trigger("l2", 1.0)
    pad.press("dpad-right")
    seen.settle()
    assert seen.commands() == [["/usr/local/bin/legion-brightness", "up"]], \
        "it ran %s" % seen.commands()


def device(pad, seen):
    """Brightness has a ceiling and the screen usually sits on it.

    `legion-brightness` clamps, so on a screen already at the top there is no
    higher number to arrive at, and asserting that one does fails on a machine
    doing exactly what it should. Room is made first, by one step down, and the
    step up gives it back: the screen ends where it was found.

    What full is stays the script's to know. A number here would be a second
    opinion about this panel, and two numbers about one screen part company the
    day either of them moves.
    """
    pad.trigger("l2", 1.0)
    pad.press("dpad-left")
    seen.settle(1.0)
    was = seen.brightness()
    pad.press("dpad-right")
    seen.settle(1.0)
    pad.trigger("l2", 0.0)
    now = seen.brightness()
    assert now > was, "it was %d and is %d" % (was, now)
