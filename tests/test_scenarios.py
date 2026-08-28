"""The written-down scenarios, played where nothing can go wrong.

A scenario is what somebody did with their thumbs, kept so it can be done
again. They are worth keeping only if they still run, and a scenario naming a
button that has since been renamed is a scenario nobody will find out about
until they reach for it.
"""

import pytest

from emulator.go import LegionGo
from emulator.script import play


def scenarios(root):
    return sorted((root / "scenarios").glob("*.txt"))


def test_there_are_some(request):
    assert scenarios(request.config.rootpath)


@pytest.mark.parametrize("number", range(3))
def test_every_scenario_plays(request, world, number):
    """Parametrised by position so a new file is picked up without being named
    here, and a file going missing is a failure rather than a silence."""
    files = scenarios(request.config.rootpath)
    if number >= len(files):
        pytest.skip("only %d scenarios" % len(files))
    go = LegionGo(devices=world, root=request.config.rootpath)
    play(go, files[number].read_text(), sleep=lambda _: None)
    assert world.log, "%s pressed nothing" % files[number].name
