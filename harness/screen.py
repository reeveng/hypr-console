"""The device's screen, read out of the compositor's own file.

The panel is mounted portrait and turned a quarter, and the desktop is laid
out at two and a half times the density everything is drawn at. Three numbers,
and they were written down in four places: the tool that nests the desktop,
the one that draws the wallpaper, a test, and a comment. This is the one
place, and it reads them from the file the device itself reads, so a screen
that changes changes them all.

A test environment that is not the shape, the size or the density of the
thing it stands in for is a test environment that agrees with you. The
wallpaper was drawn portrait into a landscape screen for its whole life and
nothing said so.
"""

import re
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CONFIG = REPO / "files/home/player/.config/hypr/hyprland.lua"


class Screen:
    """What the compositor is told the screen is."""

    def __init__(self, text=None):
        lua = CONFIG.read_text() if text is None else text
        said = re.search(r"hl\.monitor\s*\(\s*\{(.+?)\}\s*\)", lua, re.S)
        if said is None:
            raise AssertionError("%s declares no screen" % CONFIG)
        block = said.group(1)
        self.mode = tuple(int(n) for n in self._must(
            block, r'mode\s*=\s*"(\d+)x(\d+)').groups())
        self.refresh = int(self._must(block, r'mode\s*=\s*"\d+x\d+@(\d+)').group(1))
        self.scale = float(self._must(block, r"scale\s*=\s*([\d.]+)").group(1))
        self.transform = int(self._must(block, r"transform\s*=\s*(\d+)").group(1))

    @staticmethod
    def _must(block, pattern):
        found = re.search(pattern, block)
        if found is None:
            raise AssertionError("the screen says nothing about %s" % pattern)
        return found

    @property
    def turned(self):
        """Whether the compositor rotates it a quarter or three quarters."""
        return self.transform % 2 == 1

    @property
    def pixels(self):
        """The size of a picture of it, which is the mode after the turn."""
        wide, tall = self.mode
        return (tall, wide) if self.turned else (wide, tall)

    @property
    def logical(self):
        """The size the desktop is laid out in, which is what a window sees."""
        wide, tall = self.pixels
        return (round(wide / self.scale), round(tall / self.scale))

    def cut_to(self, room):
        """The same screen, made small enough to look at on one this size.

        Only the density is given up, and only as far as it has to be: the
        desktop is still laid out in the same logical size, so everything is
        still where it is on the device and only the pixels are fewer. Nothing
        is given up at all when it already fits.
        """
        wide, tall = self.pixels
        room_wide, room_tall = room
        fits = min(room_wide / wide, room_tall / tall, 1.0)
        return self.scale * fits


def where(picture, across, down, screen=None):
    """A place in the desktop's own layout, found on a picture of its pixels.

    A check says where something is the way the compositor says it: in the
    size the desktop is laid out in. A picture is the screen's own pixels,
    because that is what the device draws and what a fault in drawing shows up
    in. This is the one place that knows the difference between the two.
    """
    screen = screen or Screen()
    return picture.at(across * picture.width / screen.logical[0],
                      down * picture.width / screen.logical[0])
