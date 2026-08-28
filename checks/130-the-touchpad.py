"""A finger on the pad moves the pointer, and a quick touch is a click."""

FEATURE = "touchpad"
SINCE = "2026-08-27"


def here(pad, seen):
    from evdev import ecodes as e
    pad.drag((200, 300), (500, 300))
    seen.settle()
    assert seen.wrote(e.EV_REL, e.REL_X) > 0, "the pointer did not move"
    assert seen.wrote(e.EV_REL, e.REL_Y) == 0, "it moved the other way too"

    pad.tap(400, 400)
    seen.settle()
    assert seen.sent(e.EV_KEY, e.BTN_LEFT, 1), "a tap did not click"
    assert seen.sent(e.EV_KEY, e.BTN_LEFT, 0), "the click was never let go"


def device(pad, seen):
    """InputPlumber cannot send touch: asked to translate it, it answers
    "Translation not implemented" and drops the event, which is the whole
    reason the daemon reads the pad directly. So there is no way to press this
    one from here, and the pointer is where the emulator has to be believed."""
    raise NotImplementedError("touch is not InputPlumber's to send")
