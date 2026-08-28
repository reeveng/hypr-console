"""Every service the desktop is made of is running."""

FEATURE = "services"
SINCE = "2026-08-24"


def device(pad, seen):
    states = seen.services()
    assert states and all(s == "active" for s in states), \
        "the desktop is missing a piece: %s" % states
