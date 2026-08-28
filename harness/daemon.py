"""Running one of the desktop's daemons here, where it can be watched.

The daemons are scripts on the device with no extension and no package around
them, which is right for what they are. To test one, it is loaded from the
checkout as it is written, given a world of devices that is not this machine's
and a clock that is not this machine's either, and then run for as many turns
of its loop as the test wants.

Nothing is stubbed inside the daemon. What it decides, it decides.
"""

import importlib.machinery
import importlib.util
import sys
from pathlib import Path

from . import fake_evdev

REPO = Path(__file__).resolve().parent.parent
BIN = REPO / "files/usr/local/bin"


class Clock:
    """Time, as a number this holds.

    A daemon that turns how long a stick was held into how far a page scrolled
    is arithmetic, and arithmetic has one right answer. Reading the machine's
    clock would make it a race instead.
    """

    def __init__(self, ticks, script=None):
        self.now = 1000.0
        self.ticks = ticks
        self.tick = 0
        self.slept = 0.0
        self.naps = []
        self.script = dict(script or {})

    def monotonic(self):
        return self.now

    def time(self):
        return self.now

    def sleep(self, seconds):
        self.naps.append(seconds)
        self.slept += seconds
        self.now += seconds
        self.tick += 1
        happens = self.script.pop(self.tick, None)
        if happens is not None:
            happens()
        if self.tick >= self.ticks:
            raise fake_evdev.Stop


class Ran:
    """Every command the daemon started, instead of starting any of them."""

    DEVNULL = -3
    PIPE = -1
    STDOUT = -2

    def __init__(self):
        self.commands = []

    def Popen(self, argv, **_):        # noqa: N802 - subprocess spells it this way
        self.commands.append(list(argv))
        return self

    def run(self, argv, **_):          # in case a daemon reaches for it
        self.commands.append(list(argv))
        return self

    def wait(self):
        return 0

    @property
    def names(self):
        """Just the program of each, which is usually the whole question."""
        return [Path(argv[0]).name for argv in self.commands]

    def ran(self, name):
        return name in self.names

    def dispatched(self):
        """What was asked of the compositor, as the argument it was given."""
        return [argv[-1] for argv in self.commands
                if Path(argv[0]).name == "hyprctl" and argv[1:2] == ["dispatch"]]


class Daemon:
    """A daemon, its world, and what it did."""

    def __init__(self, name, world=None, script=None):
        self.world = world if world is not None else fake_evdev.World()
        self.path = Path(script) if script else BIN / name
        self.ran = Ran()
        self.clock = None
        was = fake_evdev.install(self.world)
        try:
            loader = importlib.machinery.SourceFileLoader(
                "legion_daemon_" + name.replace("-", "_"), str(self.path))
            spec = importlib.util.spec_from_loader(loader.name, loader)
            self.module = importlib.util.module_from_spec(spec)
            loader.exec_module(self.module)
        finally:
            if was is None:
                sys.modules.pop("evdev", None)
            else:
                sys.modules["evdev"] = was
        self.module.subprocess = self.ran

    def run(self, ticks=8, script=None):
        """Turn the daemon's loop over, and stop it after so many turns.

        `script` says what happens partway through: `{3: pad.unplug}` pulls the
        pad out between the third turn and the fourth. Anything that has to
        happen while the daemon is running rather than before it starts belongs
        here, because a daemon started twice is two daemons.
        """
        self.clock = Clock(ticks, script)
        self.module.time = self.clock
        try:
            self.module.main()
        except fake_evdev.Stop:
            pass
        return self

    @property
    def output(self):
        return self.world.output
