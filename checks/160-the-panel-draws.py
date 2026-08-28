"""The settings panel opens, and draws itself.

This is the check that was missing when the panel raised before drawing
anything and the whole suite stayed green. Nothing else builds a panel:
building one wants a compositor, so a file that could not survive its own
first screenful passed everything there was to pass.
"""

FEATURE = "panel"
SINCE = "2026-08-28"

from harness.palette import palette                            # noqa: E402

# Down the left of the panel, past the tab strip and through the rows, in the
# coordinates the compositor lays out in. A band rather than a point: the
# question is whether the panel is there at all, and a point is a question
# about where a row happens to be.
ACROSS = 200
DOWN = range(150, 520, 6)


def desktop(pad, seen):
    wanted = palette()
    seen.open("settings-panel Sound")
    down = {str(seen.colour(ACROSS, y)).lower().lstrip("#") for y in DOWN}

    assert wanted["panel"] in down or wanted["ground"] in down, \
        "nothing of the panel is on the screen where it should be: %s" \
        % sorted(down)
    assert wanted["pink"] in down, \
        "the panel drew but nothing on it is highlighted: %s" % sorted(down)
