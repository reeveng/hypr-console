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
    was = seen.brightness()
    pad.trigger("l2", 1.0)
    pad.press("dpad-left")
    seen.settle(1.0)
    pad.trigger("l2", 0.0)
    assert seen.brightness() < was, "still at %d" % was
