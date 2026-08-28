"""B closes a panel with the keyboard over it, and leaves the pad usable."""

FEATURE = "panel"
SINCE = "2026-08-28"


def device(pad, seen):
    """The keyboard is over the panel and B still means back.

    Nothing translates it: while wvkbd is up the keyboard profile maps
    nothing, so B arrives as the keyboard's backspace, and the panel, which
    holds the keyboard focus, reads backspace as back. The thumb's habit
    works without anybody being told.

    Two faults hid behind each other here. The panel used to be stopped for
    as long as the keyboard was up, since the signal that takes the pad from
    the daemon went to everything in its control group, so the press was
    answered a minute later when the keyboard came down. And a panel that
    closes under the keyboard puts the desktop back as it goes, which the
    hook then covered with the panel's own profile: the pad answered to a
    panel that was not there and the menu button drew nothing. So this
    presses the way out and then asks whether the machine still works.
    """
    pad.press("menu")
    seen.settle(2.0)
    assert seen.menus(), "the panel did not draw"

    pad.press("x")
    seen.settle(1.5)
    assert seen.keyboard(), "the keyboard did not come up over the panel"
    assert seen.menus(), "the keyboard came up and the panel went"

    pad.press("b")
    seen.settle(1.5)
    assert not seen.menus(), "B did not close the panel"

    pad.press("x")
    seen.settle(1.5)
    assert not seen.keyboard(), "the keyboard would not go away"
    assert seen.profile() == "Desktop", \
        "the pad still answers to the panel that closed; profile is %s" % seen.profile()

    pad.press("menu")
    seen.settle(2.0)
    assert seen.menus(), "the menu button stopped drawing anything"
    pad.press("b")
    seen.settle(1.5)
