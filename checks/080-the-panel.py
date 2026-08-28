"""Legion right opens the settings panel."""

FEATURE = "panel"
SINCE = "2026-08-28"


def here(pad, seen):
    pad.press("legion-right")
    seen.settle()
    assert [c[0] for c in seen.commands()] == ["settings-panel"], \
        "it ran %s" % seen.commands()


def device(pad, seen):
    pad.press("legion-right")
    seen.settle(2.0)
    assert seen.profile() in ("Menu", "Tabs"), \
        "the panel did not take the pad; profile is %s" % seen.profile()
    pad.press("b")
    seen.settle(1.5)
    assert seen.profile() == "Desktop", "the pad was not handed back"
