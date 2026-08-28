"""Reading a pixel out of a screenshot.

A screenshot nobody looks at agrees with anything. The wallpaper on the device
had not been painting for days: hyprpaper 0.8 changed its config format, the
old lines stopped meaning anything, and it did not fail. It started, said the
monitor had no target, painted nothing and reported success. What was on screen
was the compositor's own default, near enough to a plain dark background that
nobody went looking. A service being active proves nothing about whether it is
doing its job, and the only thing that would have caught it is looking at the
colour of the screen.

So this reads a colour out of a PNG, and it does it here rather than through a
library, because the whole of what is needed is one pixel out of what grim
writes: eight bits a channel, not interlaced, with the five filters PNG has.
"""

import struct
import zlib

PAETH, AVERAGE, UP, SUB, NONE = 4, 3, 2, 1, 0


def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


class Picture:
    """A PNG, far enough decoded to be asked the colour of somewhere."""

    def __init__(self, path):
        data = open(path, "rb").read()
        if data[:8] != b"\x89PNG\r\n\x1a\n":
            raise ValueError("%s is not a PNG" % path)

        chunks, at = {}, 8
        pixels = b""
        while at < len(data):
            length, kind = struct.unpack(">I4s", data[at:at + 8])
            body = data[at + 8:at + 8 + length]
            if kind == b"IDAT":
                pixels += body
            else:
                chunks[kind] = body
            at += 12 + length

        (self.width, self.height, depth, colour,
         compression, filtering, interlace) = struct.unpack(">IIBBBBB",
                                                            chunks[b"IHDR"])
        if depth != 8 or interlace or colour not in (2, 6):
            raise ValueError("only the plain kind of PNG is read here: "
                             "depth %d, colour %d, interlace %d"
                             % (depth, colour, interlace))
        self.channels = 3 if colour == 2 else 4
        self.rows = self._unfilter(zlib.decompress(pixels))

    def _unfilter(self, raw):
        stride = self.width * self.channels
        step = self.channels
        rows, previous, at = [], bytearray(stride), 0
        for _ in range(self.height):
            how = raw[at]
            line = bytearray(raw[at + 1:at + 1 + stride])
            at += 1 + stride
            for i in range(stride):
                left = line[i - step] if i >= step else 0
                above = previous[i]
                corner = previous[i - step] if i >= step else 0
                if how == SUB:
                    line[i] = (line[i] + left) & 0xFF
                elif how == UP:
                    line[i] = (line[i] + above) & 0xFF
                elif how == AVERAGE:
                    line[i] = (line[i] + (left + above) // 2) & 0xFF
                elif how == PAETH:
                    line[i] = (line[i] + _paeth(left, above, corner)) & 0xFF
                elif how != NONE:
                    raise ValueError("no such filter as %d" % how)
            rows.append(line)
            previous = line
        return rows

    def at(self, x, y):
        """The colour of one place, as it would be written in a stylesheet."""
        x, y = int(x), int(y)
        if not (0 <= x < self.width and 0 <= y < self.height):
            raise ValueError("%d,%d is off the edge of a %dx%d picture"
                             % (x, y, self.width, self.height))
        start = x * self.channels
        row = self.rows[y]
        return "%02x%02x%02x" % (row[start], row[start + 1], row[start + 2])

    def average(self, across, down, size=0.02):
        """The average colour of a small patch, placed by fraction not pixel.

        A fraction because the thing being compared was measured on a picture
        of another size, and an average because a petal that strayed into the
        patch should move the answer by less than the encoder does.
        """
        wide = max(1, int(self.width * size))
        left = min(max(0, int(self.width * across) - wide // 2),
                   self.width - wide)
        top = min(max(0, int(self.height * down) - wide // 2),
                  self.height - wide)
        totals, seen = [0, 0, 0], 0
        for y in range(top, min(top + wide, self.height)):
            row = self.rows[y]
            for x in range(left, min(left + wide, self.width)):
                start = x * self.channels
                for band in range(3):
                    totals[band] += row[start + band]
                seen += 1
        return "%02x%02x%02x" % tuple(total // seen for total in totals)

    def commonest(self):
        """The colour most of the screen is, which is usually the background."""
        seen = {}
        for y in range(0, self.height, max(1, self.height // 64)):
            row = self.rows[y]
            for x in range(0, self.width, max(1, self.width // 64)):
                start = x * self.channels
                colour = "%02x%02x%02x" % (row[start], row[start + 1],
                                           row[start + 2])
                seen[colour] = seen.get(colour, 0) + 1
        return max(seen, key=seen.get)
