"""The wallpaper is painted, and it is the picture the manifest carries."""

FEATURE = "wallpaper"
SINCE = "2026-08-28"

from pathlib import Path                                        # noqa: E402

from harness.picture import Picture                             # noqa: E402

WALLPAPER = Path(__file__).resolve().parent.parent \
    / "files/usr/share/backgrounds/legion.png"


def wanted():
    """The colour most of the picture is, read out of the picture.

    Not a colour written down here. A wallpaper that changes changes this with
    it, and a check that has to be edited whenever the thing it checks changes
    is a check that will one day be edited to agree with a fault.
    """
    return Picture(WALLPAPER).commonest()


def desktop(pad, seen):
    if not seen.installed("hyprpaper"):
        raise NotImplementedError("hyprpaper is not installed on this machine")
    behind, expected = seen.background(), wanted()
    assert behind == expected, \
        "the screen is #%s where the wallpaper is #%s, so something else " \
        "painted it" % (behind, expected)


def device(pad, seen):
    """On an empty workspace, because a maximised window is what you would be
    measuring otherwise. Every window here opens on one of its own, so there is
    always an empty one a shoulder away."""
    for _ in range(6):
        if seen.windows_here() == 0:
            break
        pad.press("r1")
        seen.settle(1.0)
    else:
        raise AssertionError("could not get to a workspace with nothing on it")
    behind, expected = seen.background(), wanted()
    assert behind == expected, \
        "the screen is #%s where the wallpaper is #%s, so something else " \
        "painted it" % (behind, expected)
