"""The wallpaper is painted, and it is the picture the garden drew.

A wrong colour here has two causes and they want opposite answers, so a wrong
colour says which. The garden may have been drawn wrong, or awww may be playing
the last garden's frames over this one's still, which it will do whenever a new
picture arrives at the old picture's path. Two of the probes sit in the band the
wind redraws, which is where two pictures mixed together shows first, so this
catches the cache fault already; what it used to do was report it as a colour
being wrong, and send the reader at the drawing. The drawing is the third rung
of the ladder in docs/theme.md and the cache is the second, and an afternoon
once went at the encoder for want of somebody saying so.
"""

FEATURE = "wallpaper"
SINCE = "2026-08-28"

import tomllib                                                   # noqa: E402
from pathlib import Path                                        # noqa: E402

STAMP = Path(__file__).resolve().parent.parent / "theme/garden.stamp"
PICTURE = "/usr/share/backgrounds/legion.webp"


def stamped():
    """What the garden says it drew.

    Not colours written down here. `tools/legion-garden` writes the stamp
    every time it draws, off the surface while it was still pixels, so a
    wallpaper that changes changes this with it. Reading the picture itself
    would be one step shorter and is what this used to do, but the picture is
    an animation now, and neither this nor the device will take a VP8
    bitstream apart. What proves the file on disk is still the file the garden
    drew is the manifest's own check of its contents.
    """
    return tomllib.loads(STAMP.read_text())


def near(one, other, within=4):
    """Whether two colours are the same colour after a lossy encoder.

    The garden measures the picture while it is still pixels; what reaches the
    screen has been through webp, which moves a flat colour by a unit or two.
    Anything that painted something else is out by more than a hundred.
    """
    return all(abs(int(one[at:at + 2], 16) - int(other[at:at + 2], 16)) <= within
               for at in (0, 2, 4))


def how_long(seconds):
    """A gap in the largest unit it fills, because the size is the point."""
    for size, unit in ((86400, "day"), (3600, "hour"), (60, "minute")):
        if seconds >= size:
            many = seconds // size
            return "%d %s%s" % (many, unit, "" if many == 1 else "s")
    return "%d seconds" % seconds


def or_the_cache(seen):
    """Which of the two faults a wrong colour is, where the stage can say.

    awww names a cache file after the picture's path, its size and how it was
    fitted to the screen, and nothing in that name comes from what is inside
    the file. Frames older than the picture they are frames of is the whole
    fault, and nothing about the drawing is in question.

    Newer than it does not mean the other thing, and this is the branch to be
    careful in. An mtime stands in for "these frames were decoded from these
    bytes", and the two come apart whenever a picture arrives carrying a date
    of its own: restored from a backup, copied with `cp -p`, pulled with
    `rsync -a`. New bytes under an old date, and a cache built from what was
    there before is newer than the file it no longer matches. So both branches
    end in the same move. Restart the daemon and look again, because that ends
    the question and an mtime never does, in either direction; a restart
    nobody needed costs seconds, and an afternoon at the encoder is what it
    costs to be told the cache is fine while it is the fault.

    Asked for, rather than required. A stage that cannot stat the machine it is
    looking at says nothing, and the colour still fails on its own.
    """
    ask = getattr(seen, "frame_cache", None)
    if ask is None:
        return ""
    when = ask(PICTURE)
    frames, drawn = when.get("frames"), when.get("picture")
    if frames is None or drawn is None:
        return ""
    if frames < drawn:
        return " The decoded frames under ~/.cache/awww are %s older than " \
               "the picture, so this is the cache and not the drawing: " \
               "restart legion-paper.service and look again." \
               % how_long(drawn - frames)
    return " The decoded frames under ~/.cache/awww were written %s after " \
           "the picture, which does not clear them: a picture restored or " \
           "copied with its own dates is new bytes under an old one. " \
           "Restart legion-paper.service and look again before reading " \
           "anything into the drawing." % how_long(frames - drawn)


def painted(seen):
    """The picture is on the screen, in the shape it was drawn.

    The commonest colour alone would pass on a screen nothing painted, because
    the compositor's own background is the picture's darkest colour by
    deliberate choice. The probes are places the garden measured that are not
    that colour and are spread apart, so a picture painted at the wrong size,
    the wrong shape or the wrong way up misses at least one of them.
    """
    garden = stamped()
    behind, resting = seen.background(), garden["resting"].lstrip("#")
    if not near(behind, resting):
        raise AssertionError(
            "the screen is #%s where the wallpaper is #%s, so something else "
            "painted it.%s" % (behind, resting, or_the_cache(seen)))
    for probe in garden["probe"]:
        across, down = probe["at"]
        there, expected = seen.patch(across, down), probe["colour"].lstrip("#")
        if not near(there, expected):
            raise AssertionError(
                "at %g,%g across the screen is #%s where the garden drew "
                "#%s.%s" % (across, down, there, expected, or_the_cache(seen)))


def desktop(pad, seen):
    if not seen.installed("awww-daemon"):
        raise NotImplementedError("awww is not installed on this machine")
    painted(seen)


def device(pad, seen):
    """On an empty workspace, because a maximised window is what you would be
    measuring otherwise. Every window here opens on one of its own, so there is
    always an empty one a shoulder away.

    Colour is the weaker half here: the device's bare background is the same
    colour as the resting garden, deliberately, so the daemon is asked what it
    is showing as well.
    """
    showing = seen.wallpaper()
    assert PICTURE in showing, \
        "the wallpaper daemon is showing %s" % (showing.strip() or "nothing")
    for _ in range(6):
        if seen.windows_here() == 0:
            break
        pad.press("r1")
        seen.settle(1.0)
    else:
        raise AssertionError("could not get to a workspace with nothing on it")
    painted(seen)
