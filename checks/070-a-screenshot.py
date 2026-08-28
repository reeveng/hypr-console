"""The bottom right paddle takes a screenshot."""

FEATURE = "screenshot"
SINCE = "2026-08-26"


def here(pad, seen):
    pad.press("right-paddle-bottom")
    seen.settle()
    assert seen.commands() == [["/usr/local/bin/legion-screenshot"]], \
        "it ran %s" % seen.commands()


SHOTS = "/home/player/Pictures"


def device(pad, seen):
    before = seen.files(SHOTS)
    pad.press("right-paddle-bottom")
    seen.settle(2.5)
    after = seen.files(SHOTS)
    assert len(after) > len(before), "no picture appeared in %s" % SHOTS
