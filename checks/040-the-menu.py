"""The top left paddle opens the menu."""

FEATURE = "launcher"
SINCE = "2026-08-24"


def here(pad, seen):
    pad.press("left-paddle-top")
    seen.settle()
    assert [c[0] for c in seen.commands()] == ["launcher"], \
        "it ran %s" % seen.commands()


def device(pad, seen):
    pad.press("left-paddle-top")
    seen.settle(2.0)
    assert seen.profile() in ("Menu", "Tabs"), \
        "the chooser did not take the pad; profile is %s" % seen.profile()
    pad.press("b")
    seen.settle(1.5)
    assert seen.profile() == "Desktop", \
        "the pad was not handed back; profile is %s" % seen.profile()
