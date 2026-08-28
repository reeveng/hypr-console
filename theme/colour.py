"""Colours, and how far apart two of them are.

Every colour on this desktop is declared as a hue and how much of it, and the
lightness is worked out here rather than chosen: a colour is told what it has
to be readable against and comes back as the softest shade that clears it.
That is the whole reason this file exists. Picking pastels by eye and then
measuring them afterwards gets a palette that passed once; asking for the
palest colour that still clears 7:1 gets one that goes on clearing it when the
ground behind it changes.

Oklch in, sRGB out. Lightness in oklch is close to lightness as an eye reads
it, so a binary search on it converges on the answer from either side and the
hue does not drift while it happens.
"""

import math

# Oklab, from Björn Ottosson's derivation. The two matrices are the transform
# through the cone responses and back; the cube and cube root either side of
# them are what make the space perceptual.
_LMS = (
    (0.4122214708, 0.5363325363, 0.0514459929),
    (0.2119034982, 0.6806995451, 0.1073969566),
    (0.0883024619, 0.2817188376, 0.6299787005),
)
_FROM_LMS = (
    (4.0767416621, -3.3077115913, 0.2309699292),
    (-1.2684380046, 2.6097574011, -0.3413193965),
    (-0.0041960863, -0.7034186147, 1.7076147010),
)


def _to_srgb(channel):
    """A linear channel as sRGB writes it."""
    if channel <= 0.0031308:
        return 12.92 * channel
    return 1.055 * channel ** (1 / 2.4) - 0.055


def _to_linear(channel):
    if channel <= 0.04045:
        return channel / 12.92
    return ((channel + 0.055) / 1.055) ** 2.4


def oklch_to_rgb(lightness, chroma, hue):
    """(r, g, b) as floats, which are outside 0..1 when the colour is not real."""
    radians = math.radians(hue)
    a, b = chroma * math.cos(radians), chroma * math.sin(radians)
    cones = [lightness + row[1] * a + row[2] * b
             for row in ((0, 0.3963377774, 0.2158037573),
                         (0, -0.1055613458, -0.0638541728),
                         (0, -0.0894841775, -1.2914855480))]
    cubed = [value ** 3 for value in cones]
    return tuple(_to_srgb(sum(row[i] * cubed[i] for i in range(3)))
                 for row in _FROM_LMS)


def in_gamut(lightness, chroma, hue):
    """Whether a screen can actually show it."""
    return all(-0.0001 <= channel <= 1.0001
               for channel in oklch_to_rgb(lightness, chroma, hue))


def fit(lightness, chroma, hue):
    """The same colour with just enough chroma taken out of it to be real.

    Lightness is what the contrast was worked out from, so it is held and the
    saturation gives way. A pastel that has lost a little chroma is still the
    colour it was meant to be; one that has lost lightness is a different one.
    """
    if in_gamut(lightness, chroma, hue):
        return chroma
    low, high = 0.0, chroma
    for _ in range(40):
        middle = (low + high) / 2
        if in_gamut(lightness, middle, hue):
            low = middle
        else:
            high = middle
    return low


def hexcode(lightness, chroma, hue):
    """A colour as six hex digits, fitted into the gamut on the way."""
    rgb = oklch_to_rgb(lightness, fit(lightness, chroma, hue), hue)
    return "".join(f"{round(min(1.0, max(0.0, channel)) * 255):02x}"
                   for channel in rgb)


def luminance(code):
    """Relative luminance, as WCAG defines it, from six hex digits."""
    code = code.lstrip("#")
    channels = [int(code[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    red, green, blue = (_to_linear(channel) for channel in channels)
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue


def contrast(one, other):
    """How far apart two colours are, from 1:1 to 21:1."""
    first, second = luminance(one), luminance(other)
    lighter, darker = max(first, second), min(first, second)
    return (lighter + 0.05) / (darker + 0.05)


def over(top, bottom, alpha):
    """`top` laid on `bottom` at `alpha`, as the screen would blend them.

    Anything painted with transparency is a colour in its own right once it is
    on screen, and it is that colour the text on top of it has to clear.
    """
    top, bottom = top.lstrip("#"), bottom.lstrip("#")
    mixed = []
    for i in (0, 2, 4):
        front, back = int(top[i:i + 2], 16), int(bottom[i:i + 2], 16)
        mixed.append(round(front * alpha + back * (1 - alpha)))
    return "".join(f"{channel:02x}" for channel in mixed)


def lightest_clearing(chroma, hue, grounds, ratio, floor=0.0):
    """The darkest lightness at which a hue clears `ratio` against every ground.

    Darkest, because a pastel that is lighter than it needs to be is a pastel
    on its way to white, and ten of those are one colour. Contrast against a
    dark ground climbs with lightness and never falls, so the answer is found
    by halving.
    """
    if not grounds:
        return floor
    def clears(lightness):
        code = hexcode(lightness, chroma, hue)
        return all(contrast(code, ground) >= ratio for ground in grounds)

    if clears(floor):
        return floor
    low, high = floor, 1.0
    if not clears(high):
        raise ValueError(f"nothing at hue {hue} clears {ratio}:1 against {grounds}")
    for _ in range(48):
        middle = (low + high) / 2
        if clears(middle):
            high = middle
        else:
            low = middle
    return high


def darkest_clearing(chroma, hue, ceilings, ratio):
    """The lightest lightness at which a hue clears `ratio` under every ceiling.

    The mirror of the one above, for ink that is painted on top of a fill: the
    fill is already decided and the ink has to be dark enough against it.
    """
    def clears(lightness):
        code = hexcode(lightness, chroma, hue)
        return all(contrast(code, ceiling) >= ratio for ceiling in ceilings)

    if not ceilings or clears(1.0):
        return 1.0
    low, high = 0.0, 1.0
    if not clears(low):
        raise ValueError(f"nothing at hue {hue} clears {ratio}:1 under {ceilings}")
    for _ in range(48):
        middle = (low + high) / 2
        if clears(middle):
            low = middle
        else:
            high = middle
    return low
