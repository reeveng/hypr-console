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
    was = seen.brightness()
    pad.trigger("l2", 1.0)
    pad.press("dpad-right")
    seen.settle(1.0)
    pad.trigger("l2", 0.0)
    assert seen.brightness() > was, "still at %d" % was
