import sys
from pathlib import Path

import pytest

# The daemons are loaded from the tree they are installed from, and a stray
# __pycache__ beside them is a file the manifest does not carry.
sys.dont_write_bytecode = True

REPO = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO))

from emulator.go import LegionGo            # noqa: E402
from harness.daemon import Daemon           # noqa: E402
from harness.fake_evdev import World        # noqa: E402


@pytest.fixture
def repo():
    return REPO


@pytest.fixture
def world():
    """Four devices, and no others. Nothing on the machine takes part."""
    made = World()
    yield made
    made.close()


@pytest.fixture
def go(world):
    """The front of a Legion Go, wired to that world."""
    return LegionGo(devices=world, root=REPO)


@pytest.fixture
def controller(world):
    """The daemon that reads the pad, loaded but not yet running."""
    return Daemon("stick-scroll", world)
