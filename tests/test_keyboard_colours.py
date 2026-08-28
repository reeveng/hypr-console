"""No two things on the keyboard may be the same colour, and none is a wash.

This fault has happened three times. First the slab behind the keys and a key that
is not a letter were both `ground`, so Esc and Tab and the arrows had nothing
under them. Then the slab and a key being pressed were both `night`, so a key
vanished at the moment it was pressed. Both times the keyboard looked
see-through, and both times nothing said anything. The third time was the key
the stick is sitting on, drawn in the swipe colour, and a swipe's colour is a
quarter of a colour by design: it is a wash laid over a key to show where a
finger went. The wallpaper came through the letter.

The colours are read out of the command `osk-start` builds, so this asks about
what the keyboard is actually given rather than about what anybody meant.
"""

import re

import pytest

from harness.palette import palette

# What each background is called, and the ink written on it. The slab has no
# ink of its own: nothing is written on the space between keys.
INK = {"bg": None, "fg": "text", "fg-sp": "text-sp",
       "press": "text-press", "press-sp": "text-press-sp",
       "sel": "text-sel", "sel-sp": "text-sel-sp",
       "swipe": "text-swipe", "swipe-sp": "text-swipe-sp"}

BACKGROUNDS = ("bg", "fg", "fg-sp", "press", "sel")

# Everything else on that command line is a size or a font.
COLOURS = set(INK) | {ink for ink in INK.values() if ink}


@pytest.fixture(scope="module")
def spends(request):
    """Which colour `osk-start` hands to each of wvkbd's colour options."""
    text = (request.config.rootpath
            / "files/usr/local/bin/osk-start").read_text()
    found = dict(re.findall(r'--([a-z-]+) "\$(\w+)"', text))
    named = {option: colour for option, colour in found.items()
             if option in COLOURS}
    assert named, "no colours are passed to wvkbd"
    return named


@pytest.fixture(scope="module")
def raw(request):
    """The same options, as they are written rather than as they resolve."""
    text = (request.config.rootpath
            / "files/usr/local/bin/osk-start").read_text()
    return {option: value for option, value
            in re.findall(r'--([a-z-]+) "([^"]*)"', text)
            if option in COLOURS}


def test_every_colour_it_spends_is_in_the_palette(spends):
    known = palette()
    for option, colour in spends.items():
        assert colour in known, \
            "--%s is $%s, which the palette does not have" % (option, colour)


def test_no_two_backgrounds_are_the_same_colour(spends):
    """A key the colour of the slab is a key with nothing under it."""
    known = palette()
    seen = {}
    for option in BACKGROUNDS:
        if option not in spends:
            continue
        colour = known[spends[option]]
        assert colour not in seen, \
            "--%s and --%s are both #%s, so one of them is invisible " \
            "against the other" % (option, seen[colour], colour)
        seen[colour] = option


def test_nothing_is_written_in_the_colour_it_is_written_on(spends):
    known = palette()
    for background, ink in INK.items():
        if ink is None or background not in spends or ink not in spends:
            continue
        assert known[spends[background]] != known[spends[ink]], \
            "--%s and --%s are both #%s, so the writing is invisible" \
            % (background, ink, known[spends[ink]])


def test_every_background_is_named_at_all(raw):
    """An option not passed leaves wvkbd on its own colour for it, which is
    somebody else's palette and, for the selected key, a wash."""
    for option in BACKGROUNDS:
        assert option in raw, \
            "the keyboard is never told what colour --%s is, so it keeps " \
            "the one it was compiled with" % option


def test_no_colour_is_written_as_anything_but_a_palette_name(raw):
    """A colour here is named, and nothing is added to it.

    wvkbd reads a colour as `rrggbb` or as `rrggbbaa`, and six digits leaves
    the alpha wherever its own defaults put it. Every default is opaque except
    the swipe trail, which is a wash on purpose. So the two ways to end up with
    a see-through key are to write the digits out with an alpha on the end, and
    to hand a key the trail's colour. This is the first; the second is
    `test_no_two_backgrounds_are_the_same_colour` above.
    """
    for option in raw:
        assert re.fullmatch(r"\$\w+", raw[option]), \
            "--%s is given %s. A colour here is a palette name and nothing " \
            "else: the keyboard is read against the wallpaper, and anything " \
            "after the six digits is an alpha." % (option, raw[option])
