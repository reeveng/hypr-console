"""Somewhere a check can be run, and what can be seen from there.

A check says what somebody did and what should have happened. Where it is run
decides how the doing is done and how much of the happening can be seen at all.

    here      emulated devices, the daemon in this process, no machine
              involved. What can be seen is what the daemon decided to run.

    device    the Legion Go itself, over ssh. The pressing goes through
              InputPlumber's own SendEvent, so a button arrives exactly as the
              hardware's would, through the loaded profile. What can be seen is
              the machine: which workspace, which windows, how bright, whether
              the keyboard is up, which profile is loaded.

The same check file runs in both. It cannot assert the same things in both, so
it says what it needs to be able to see, and a stage that cannot see that skips
it and says so rather than passing quietly.
"""

import json
import shlex
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from emulator.go import LegionGo
from harness.picture import Picture
from emulator import vocabulary
from harness.daemon import Daemon
from harness.fake_evdev import World

REPO = Path(__file__).resolve().parent.parent
HOST = "root@handheld"
BUS = ("org.shadowblip.InputPlumber",
       "/org/shadowblip/InputPlumber/CompositeDevice0",
       "org.shadowblip.Input.CompositeDevice")


class Here:
    """The emulator, and the daemon running against it in this process."""

    name = "here"
    offers = {"commands", "dispatches", "profile", "wrote"}

    def __init__(self, root=None):
        self.world = World()
        self.go = LegionGo(devices=self.world, root=root) if root \
            else LegionGo(devices=self.world)
        self.daemon = Daemon("stick-scroll", self.world)
        self.turns = 0

    # doing

    def press(self, button):
        self.go.press(button)

    def hold(self, button):
        self.go.hold(button)

    def release(self, button=None):
        self.go.release(button) if button else self.go.release_all()

    def stick(self, which, x, y):
        self.go.stick(which, x, y)

    def trigger(self, which, amount):
        self.go.trigger(which, amount)

    def tap(self, x=512, y=512):
        self.go.tap(x, y)

    def drag(self, start, end, steps=8):
        self.go.drag(start, end, steps=steps)

    def load_profile(self, name):
        self.go.load_profile(name)

    def settle(self, seconds=None, turns=3):
        """Let the daemon read what was sent.

        Time here is turns of the daemon's own loop, not seconds. Anything
        that depends on how long a stick was held says how many turns it
        wants; a button needs a couple.
        """
        self.turns += 1
        self.daemon.run(ticks=turns)

    # seeing

    def commands(self):
        return [list(argv) for argv in self.daemon.ran.commands]

    def dispatches(self):
        return self.daemon.ran.dispatched()

    def profile(self):
        return self.go.profile_name

    def wrote(self, kind, code):
        """How much of something the daemon sent to the pointer.

        Summed over every device it has made. The daemon is started afresh for
        each settle, and starting makes a device, so the last one is not the
        whole story.
        """
        return sum(out.total(kind, code) for out in self.world.outputs)

    def sent(self, kind, code, value):
        """Whether it ever sent exactly that."""
        return any((kind, code, value) in out.written
                   for out in self.world.outputs)

    def close(self):
        self.world.close()


class Desktop:
    """The device's own desktop, nested on this machine, and looked at.

    What this can answer that nothing else can is what colour the screen is.
    A service being active proves nothing about whether it is doing its job:
    the wallpaper on the device did not paint for days because hyprpaper read a
    config format it no longer understood, painted nothing, and reported
    success. Nothing was in a failed state. The screen was the wrong colour.

    It cannot press anything. That needs an input device, which needs
    /dev/uinput, which is the other tier.
    """

    name = "desktop"

    def __init__(self, open_these=()):
        self.open_these = tuple(open_these)
        self.here = tempfile.TemporaryDirectory(prefix="legion-desktop-")
        self.taken = None

    def _picture(self):
        """One session, one picture, and every question asked of that."""
        if self.taken is None:
            shot = Path(self.here.name) / "screen.png"
            argv = [sys.executable, str(REPO / "tools/legion-desktop"),
                    "shot", str(shot)]
            for command in self.open_these:
                argv += ["--open", command]
            subprocess.run(argv, capture_output=True, text=True, timeout=180)
            if not shot.exists():
                raise AssertionError("the nested desktop took no picture")
            self.taken = Picture(shot)
        return self.taken

    def installed(self, program):
        return subprocess.run(["sh", "-c", "command -v " + shlex.quote(program)],
                              capture_output=True).returncode == 0

    def colour(self, x, y):
        return self._picture().at(x, y)

    def background(self):
        return self._picture().commonest()

    def close(self):
        self.here.cleanup()


class Device:
    """The machine itself, pressed through InputPlumber and looked at over ssh.

    Nothing here makes an input device. InputPlumber is asked to emit the event
    it would have read from the hardware, through the profile that is loaded,
    which is its own supported way of doing this and is what a chord on the
    device already uses. So there is no second pad for the daemons to find and
    nothing to clean up if a check stops halfway.
    """

    name = "device"
    offers = {"workspace", "windows", "keyboard", "profile", "brightness",
              "services", "journal", "files"}

    def __init__(self, host=HOST, dry=False):
        self.host = host
        self.dry = dry
        self.done = []

    def ssh(self, command):
        self.done.append(command)
        if self.dry:
            return ""
        done = subprocess.run(["ssh", "-o", "BatchMode=yes", self.host, command],
                              capture_output=True, text=True, timeout=60)
        return done.stdout.strip()

    def user(self, command):
        """As player, whose session the desktop is."""
        return self.ssh("machinectl shell --uid=player .host /bin/sh -c %s"
                        % shlex.quote(command))

    def hypr(self, command):
        """hyprctl needs the session's own environment to find its socket."""
        return self.user(
            "export $(cat /proc/$(pgrep -u player -x Hyprland | head -1)/environ "
            "| tr '\\0' '\\n' | grep -E '^(HYPRLAND_INSTANCE_SIGNATURE|"
            "XDG_RUNTIME_DIR|WAYLAND_DISPLAY)=' | xargs) && hyprctl %s" % command)

    # doing

    def _send(self, capability, value):
        self.ssh("busctl --system call %s %s %s SendEvent sv %s b %s"
                 % (BUS[0], BUS[1], BUS[2], shlex.quote(capability),
                    "true" if value else "false"))

    def _capability(self, button):
        return "Gamepad:Button:" + vocabulary.button_name(button)

    def press(self, button):
        self.ssh("busctl --system call %s %s %s SendButtonChord as 1 %s"
                 % (BUS[0], BUS[1], BUS[2],
                    shlex.quote(self._capability(button))))

    def hold(self, button):
        self._send(self._capability(button), True)

    def release(self, button=None):
        if button:
            self._send(self._capability(button), False)

    def trigger(self, which, amount):
        capability = "Gamepad:Trigger:" + vocabulary.TRIGGERS[which]
        self.ssh("busctl --system call %s %s %s SendEvent sv %s d %s"
                 % (BUS[0], BUS[1], BUS[2], shlex.quote(capability), amount))

    def stick(self, which, x, y):
        raise NotImplementedError("a stick is two axes in one event; not yet")

    def tap(self, x=512, y=512):
        raise NotImplementedError("the touchpad is not InputPlumber's to send")

    def drag(self, start, end, steps=8):
        raise NotImplementedError("the touchpad is not InputPlumber's to send")

    def load_profile(self, name):
        self.user("controller-profile %s" % shlex.quote(name))

    def settle(self, seconds=0.6):
        if not self.dry:
            time.sleep(seconds)

    # seeing

    def workspace(self):
        out = self.hypr("activeworkspace -j")
        return json.loads(out)["name"] if out else None

    def windows(self):
        out = self.hypr("clients -j")
        return sorted(c["class"] for c in json.loads(out)) if out else []

    def windows_here(self):
        """How many are on the workspace being looked at, which is the only
        number that says whether anything is covering the wallpaper."""
        out = self.hypr("activeworkspace -j")
        return json.loads(out)["windows"] if out else 0

    def keyboard(self):
        """Whether the on-screen keyboard is on screen, not merely running."""
        out = self.hypr("layers -j")
        return "wvkbd" in out if out else False

    def profile(self):
        return self.ssh("busctl --system get-property %s %s %s ProfileName"
                        % BUS).split('"')[-2] if not self.dry else ""

    def brightness(self):
        out = self.ssh("cat /sys/class/backlight/*/brightness")
        return int(out.splitlines()[0]) if out else 0

    def services(self):
        out = self.user("systemctl --user is-active legion-controller "
                        "legion-keyboard legion-bar legion-session legion-paper")
        return out.split()

    def files(self, where):
        """What is in a directory, for the things that leave one behind."""
        out = self.user("ls -1 %s 2>/dev/null" % shlex.quote(where))
        return sorted(out.split("\n")) if out else []

    def background(self):
        """What colour most of the device's screen is, right now.

        The picture is taken there and fetched, because the question is about
        that screen. Nothing is left behind on the machine.
        """
        if self.dry:
            self.user("grim /tmp/legion-check.png")
            return ""
        self.hypr("dispatch hl.dsp.exec_cmd(\"grim /tmp/legion-check.png\")")
        time.sleep(1.5)
        here = tempfile.TemporaryDirectory(prefix="legion-shot-")
        shot = Path(here.name) / "screen.png"
        subprocess.run(["scp", "-q", "%s:/tmp/legion-check.png" % self.host,
                        str(shot)], check=False, timeout=90)
        self.ssh("rm -f /tmp/legion-check.png")
        if not shot.exists():
            raise AssertionError("could not fetch a picture of the screen")
        colour = Picture(shot).commonest()
        here.cleanup()
        return colour

    def journal(self, unit="legion-controller", lines=20):
        return self.user("journalctl --user -u %s -n %d --no-pager"
                         % (unit, lines))

    def close(self):
        pass
