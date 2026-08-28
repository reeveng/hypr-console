"""The on-screen keyboard comes up, and every key has a key under it.

The keyboard is the piece this desktop has broken most often, and the last way
it broke was not that it failed to appear: the slab behind the keys and a key
that is not a letter had been given the same colour, so Esc, Tab, the arrows
and Enter had nothing underneath them. They read as letters lying on the
desktop, and the whole keyboard read as something you could see through.

So this asks for three colours and not one. Two of them being the same is the
fault, and a check that only asked whether the keyboard was there would have
had nothing to say about it.
"""

FEATURE = "keyboard"
SINCE = "2026-08-28"

from harness.palette import palette                            # noqa: E402

# The keyboard is 260 tall along the bottom, in the coordinates the compositor
# lays out in. Swept rather than sampled: which key sits where is the layout's
# business and none of this check's.
ACROSS = range(20, 1010, 14)
DOWN = range(390, 636, 12)


def desktop(pad, seen):
    wanted = palette()
    seen.open("osk")

    there = {str(seen.colour(x, y)).lower().lstrip("#")
             for x in ACROSS for y in DOWN}

    missing = [name for name in ("night", "ground", "panel")
               if wanted[name] not in there]
    assert not missing, \
        "the keyboard is not three shades; nothing is %s. The slab, a letter " \
        "key and a key that is not a letter have to differ or some of the " \
        "keys have nothing under them." % " or ".join(missing)
