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
from harness.screen import Screen, where
from emulator import profile as profiles
from emulator import vocabulary
from harness.daemon import Daemon
from harness.fake_evdev import World

REPO = Path(__file__).resolve().parent.parent
HOST = "root@handheld"
USER = "player"

# The session's own environment, taken from something the session started.
#
# Not from the compositor. /proc/<pid>/environ is what a process was handed
# when it was executed, and Hyprland sets WAYLAND_DISPLAY and its own
# signature after that, so its environ has neither and every hyprctl asked
# through it answered with nothing. Everything it starts inherits them.
SESSION_ENV = (
    "for p in $(ps -u %s -o pid=); do "
    "if tr '\\0' '\\n' < /proc/$p/environ 2>/dev/null "
    "| grep -q '^HYPRLAND_INSTANCE_SIGNATURE='; then "
    "export $(tr '\\0' '\\n' < /proc/$p/environ "
    "| grep -E '^(HYPRLAND_INSTANCE_SIGNATURE|WAYLAND_DISPLAY|XDG_RUNTIME_DIR)=' "
    "| xargs); break; fi; done 2>/dev/null; "
    "[ -n \"$HYPRLAND_INSTANCE_SIGNATURE\" ] || "
    "{ echo 'nothing on the device is in a Hyprland session' >&2; exit 1; }"
) % USER
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

    # doing. Every one of these throws away the picture: what was on the
    # screen a moment ago is not an answer about what is on it now.

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


SCREEN = Screen()


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

    def fresh(self):
        """Forget the picture and what was asked for; another check is next."""
        self.open_these = ()
        self.taken = None

    def open(self, *commands):
        """Have these running when the picture is taken.

        One picture answers every question asked of it, so this has to be said
        before anything is looked at. Said afterwards it would quietly be a
        statement about a screen that never had them on it, so it raises.
        """
        if self.taken is not None:
            raise AssertionError(
                "the picture has already been taken; open before looking")
        self.open_these += tuple(commands)
        return self

    def _picture(self):
        """One session, one picture, and every question asked of that."""
        if self.taken is None:
            shot = Path(self.here.name) / "screen.png"
            argv = [sys.executable, str(REPO / "tools/legion-desktop"),
                    "shot", str(shot)]
            for command in self.open_these:
                argv += ["--open", command]
            said = subprocess.run(argv, capture_output=True, text=True,
                                  timeout=180)
            if not shot.exists():
                raise AssertionError("the nested desktop took no picture: %s"
                                     % said.stderr.strip().splitlines()[-1:])
            self.taken = Picture(shot)
        return self.taken

    def installed(self, program):
        return subprocess.run(["sh", "-c", "command -v " + shlex.quote(program)],
                              capture_output=True).returncode == 0

    def colour(self, across, down):
        return where(self._picture(), across, down, SCREEN)

    def patch(self, across, down, size=0.02):
        return self._picture().average(across, down, size)

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
        self.taken = None

    def ssh(self, command):
        self.done.append(command)
        if self.dry:
            return ""
        done = subprocess.run(["ssh", "-o", "BatchMode=yes", self.host, command],
                              capture_output=True, text=True, timeout=60)
        return done.stdout.strip()

    def user(self, command):
        """As the person whose session the desktop is."""
        return self.ssh("machinectl shell --uid=%s .host /bin/sh -c %s"
                        % (USER, shlex.quote(command)))

    def hypr(self, command):
        """hyprctl needs the session's own environment to find its socket."""
        return self.user("%s && hyprctl %s" % (SESSION_ENV, command))

    # doing

    def _send(self, capability, value):
        self.taken = None
        self.ssh("busctl --system call %s %s %s SendEvent sv %s b %s"
                 % (BUS[0], BUS[1], BUS[2], shlex.quote(capability),
                    "true" if value else "false"))

    # What a target event is called when it is sent rather than received.
    SPOKEN_AS = {"key": "Keyboard:%s",
                 "mouse-button": "Mouse:Button:%s",
                 "gamepad-button": "Gamepad:Button:%s"}

    def _profile(self):
        """The profile the device has loaded, read as what it maps."""
        loaded = self.profile()
        for known in profiles.load_all(REPO).values():
            if known.name.lower() == (loaded or "").lower():
                return known
        return None

    def _capability(self, button):
        """What this button becomes under the profile the device has loaded.

        InputPlumber sends what it is handed. A capability given to SendEvent
        arrives at the targets as itself, with the loaded profile's mapping
        not applied on the way, which is the opposite of what a physical press
        does. So a paddle sent as a paddle arrives as a paddle, where a thumb
        on the same paddle arrives as a function key, and the daemon that acts
        on function keys sees nothing at all. Four checks failed on that and
        read as four faults on the device.

        A button the profile says nothing about is sent as itself, because
        that is what the device does with it too. A button the profile names
        and sends nowhere is not sent at all: under a chooser that is a
        deliberate silence, and sending the button itself would put it on the
        pad, which is the accident the naming exists to prevent.
        """
        here = self._profile()
        if here is None:
            return "Gamepad:Button:" + vocabulary.button_name(button)
        named = here.for_button(button)
        for target in (t for m in named for t in m.targets):
            if target.kind in self.SPOKEN_AS:
                return self.SPOKEN_AS[target.kind] % target.name
        if named:
            return None
        return "Gamepad:Button:" + vocabulary.button_name(button)

    def press(self, button):
        self.taken = None
        capability = self._capability(button)
        if capability is None:
            return          # named here, and sent nowhere, on purpose
        self.ssh("busctl --system call %s %s %s SendButtonChord as 1 %s"
                 % (BUS[0], BUS[1], BUS[2], shlex.quote(capability)))

    def hold(self, button):
        capability = self._capability(button)
        if capability is not None:
            self._send(capability, True)

    def release(self, button=None):
        capability = self._capability(button) if button else None
        if capability is not None:
            self._send(capability, False)

    def trigger(self, which, amount):
        """Not from here. InputPlumber sends events, and a held axis is state.

        An injected value is overwritten by the pad's own reading of a trigger
        nobody is pulling, a few hundred times a second, so it lasts about as
        long as it takes the hardware to report again. Measured on the device:
        the step by hand, 64000 to 58000. The trigger injected forty times
        around the press over one connection, 58000 to 58000. Sent instead as
        Gamepad:Button:LeftTrigger, which the composite device does publish,
        64000 to 64000. SendEvent and SendButtonChord are the whole surface and
        neither of them holds anything.

        So the screen, legion-brightness and the daemon are all in the clear,
        and a check whose subject is a held trigger wants a thumb, the way the
        touchpad does. Saying so makes those checks skip and give the reason.
        Not saying it made 020 and 021 pass because nothing had arrived, which
        is worse than either of them failing.
        """
        raise NotImplementedError(
            "an axis cannot be held from here; L2 wants a thumb")

    def stick(self, which, x, y):
        raise NotImplementedError("a stick is two axes in one event; not yet")

    def tap(self, x=512, y=512):
        raise NotImplementedError("the touchpad is not InputPlumber's to send")

    def drag(self, start, end, steps=8):
        raise NotImplementedError("the touchpad is not InputPlumber's to send")

    def load_profile(self, name):
        self.user("controller-profile %s" % shlex.quote(name))

    def exec_cmd(self, command):
        """Ask the compositor to start something.

        Quoted as one argument, because the shell on the far end would
        otherwise eat the quotes around the Lua and hand hyprctl a bare word
        where a string was meant. It answers ok either way.
        """
        return self.hypr("dispatch " + shlex.quote(
            'hl.dsp.exec_cmd("%s")' % command))

    def open(self, command="alacritty", seconds=12.0):
        """Start something on the device, and wait until it is really there.

        A check that needs a window has to be able to make one. The device is
        usually sitting on an empty desktop, and a check that refuses unless
        somebody happened to leave something open is a check that never runs.
        """
        if self.dry:
            self.exec_cmd(command)
            return True
        was = {client["address"] for client in self._clients()}
        self.exec_cmd(command)
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            self.taken = None
            new = [client for client in self._clients()
                   if client["address"] not in was]
            if new:
                # And look at it. Every window here opens on a workspace of its
                # own, so one that has just opened is not the one being looked
                # at, and a button aimed at the active window would find none.
                self.hypr("dispatch " + shlex.quote(
                    'hl.dsp.focus({workspace = "%s"})'
                    % new[0]["workspace"]["name"]))
                time.sleep(0.6)      # drawn, not only mapped
                return True
            time.sleep(0.4)
        return False

    def settle(self, seconds=0.6):
        if not self.dry:
            time.sleep(seconds)

    # seeing

    def workspace(self):
        out = self.hypr("activeworkspace -j")
        return json.loads(out)["name"] if out else None

    def _clients(self):
        out = self.hypr("clients -j")
        return json.loads(out) if out else []

    def windows(self):
        return sorted(client["class"] for client in self._clients())

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

    def fresh(self):
        """Forget the picture, and put the desk back where a check expects it.

        A chooser some earlier check left drawn is not scenery. `_capability`
        resolves a button against the profile that is loaded, and a button a
        chooser's profile names and sends nowhere is not sent at all, so the
        next check's presses quietly come to mean something else. 060 read as
        "R1 did not move" because 050 had failed with the guide still up and
        the pad in Menu, which maps no right bumper; 080 passed on a chooser it
        had never opened. Neither was a fact about the machine.

        Best effort, and it never raises. `checking.run` calls this outside the
        try that turns a check's own trouble into a result, so anything thrown
        here would end the tier rather than fail one check. A reset that could
        not run is not a fact about the check that follows, and that check will
        still say what it finds.
        """
        self.taken = None
        if self.dry:
            return
        try:
            for _ in range(3):
                if not self.menus():
                    break
                self.press("b")
                time.sleep(0.8)
            if self.profile() != "Desktop":
                self.load_profile("desktop")
                time.sleep(0.5)
        except Exception:              # noqa: BLE001 - a reset is not a result
            pass

    def _picture(self):
        """The device's screen, taken there and fetched, kept until it moves.

        Every question about colour is asked of one picture, because each one
        costs a screenshot, a copy over the network and a second and a half of
        waiting. Anything pressed throws it away again.
        """
        if self.taken is not None:
            return self.taken
        self.exec_cmd("grim /tmp/legion-check.png")
        time.sleep(1.5)
        here = tempfile.TemporaryDirectory(prefix="legion-shot-")
        shot = Path(here.name) / "screen.png"
        subprocess.run(["scp", "-q", "%s:/tmp/legion-check.png" % self.host,
                        str(shot)], check=False, timeout=90)
        self.ssh("rm -f /tmp/legion-check.png")
        if not shot.exists():
            raise AssertionError("could not fetch a picture of the screen")
        self.taken = Picture(shot)
        here.cleanup()
        return self.taken

    def background(self):
        """What colour most of the device's screen is."""
        if self.dry:
            self.user("grim /tmp/legion-check.png")
            return ""
        return self._picture().commonest()

    def colour(self, across, down):
        """The colour of one place in the desktop's layout, on the device."""
        if self.dry:
            self.user("grim /tmp/legion-check.png")
            return ""
        return where(self._picture(), across, down, SCREEN)

    def patch(self, across, down, size=0.02):
        """The average colour of a small patch, placed by fraction."""
        if self.dry:
            self.user("grim /tmp/legion-check.png")
            return ""
        return self._picture().average(across, down, size)

    def frame_cache(self, picture):
        """When the decoded frames were written, and when the picture was.

        awww names a cache file after the picture's path, its size and how it
        was fitted to the screen. Nothing in that name comes from what is
        inside the file, so a redrawn garden installed at the same path is
        played as the old picture's frames over the new picture's still. The
        second rung of the ladder in docs/theme.md is these two numbers, and
        it is where the answer usually is.

        Both are seconds since the epoch, or None where there is nothing to
        stat. No cache at all is a clean read and not a fault.
        """
        said = self.user(
            "find ~/.cache/awww -type f -exec stat -c %Y {} + 2>/dev/null "
            "| sort -n | tail -1; echo --; stat -c %Y "
            + shlex.quote(picture) + " 2>/dev/null")
        halves = said.split("--")
        seen = [[int(w) for w in half.split() if w.isdigit()]
                for half in (halves + ["", ""])[:2]]
        return {"frames": seen[0][-1] if seen[0] else None,
                "picture": seen[1][-1] if seen[1] else None}

    def wallpaper(self):
        """What the wallpaper daemon says it is showing, per screen.

        Colour alone cannot answer this here. The bare background is the
        palette's darkest colour on purpose, so that a wallpaper arriving after
        the compositor does not announce itself, and the resting garden is that
        same colour; a screen nothing painted and a screen the garden painted
        read alike. The daemon knows which it is, so it is asked.
        """
        return self.user("%s && awww query" % SESSION_ENV)

    # The layers that are always there: the bar, the wallpaper, the keyboard.
    # Anything else drawn over the desktop is something a person opened.
    FURNITURE = ("waybar", "awww-daemon", "hyprpaper", "wvkbd")

    def menus(self):
        """The choosers on screen, by name.

        A chooser is a layer and not a window, so nothing that counts windows
        can see one. Asking the profile instead is not the same question: a
        chooser hands the desktop's buttons back as it closes, so with two of
        them open the pad comes back while one is still drawn, and a check
        that asks only about the profile passes with a menu on the screen.
        """
        out = self.hypr("layers -j")
        found = []
        for screen in (json.loads(out).values() if out else ()):
            for level in screen["levels"].values():
                found += [layer["namespace"] for layer in level
                          if layer["namespace"] not in self.FURNITURE]
        return sorted(found)

    def journal(self, unit="legion-controller", lines=20):
        return self.user("journalctl --user -u %s -n %d --no-pager"
                         % (unit, lines))

    def close(self):
        pass
