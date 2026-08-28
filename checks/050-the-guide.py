"""Legion right opens the guide to what every button does."""

FEATURE = "guide"
SINCE = "2026-08-26"


def here(pad, seen):
    pad.press("legion-right")
    seen.settle()
    assert seen.commands() == [["/usr/local/bin/legion-buttons", "--menu"]], \
        "it ran %s" % seen.commands()


def device(pad, seen):
    """The guide is a chooser, so the pad goes to the chooser profile and
    comes back. B is what closes it, which is the contract."""
    pad.press("legion-right")
    seen.settle(2.5)
    assert seen.profile() in ("Menu", "Tabs"), \
        "the guide did not take the pad; profile is %s" % seen.profile()
    assert seen.menus(), "the guide took the pad but drew nothing"
    pad.press("b")
    seen.settle(1.5)
    assert seen.profile() == "Desktop", "the pad was not handed back"
    assert not seen.menus(), \
        "the pad came back and a chooser is still on screen: %s" % seen.menus()
