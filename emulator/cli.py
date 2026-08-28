"""Press the buttons of a Legion Go on a machine that is not one.

    legion-emulate                  make the devices and take commands
    legion-emulate press a b        press those and stop
    legion-emulate run scenario     play a file of the same commands
    legion-emulate what x           what that button does, in every profile
    legion-emulate devices          what the emulator publishes

The devices exist for as long as the command runs and are gone when it stops.
While they exist they are real input devices, which means the desktop in front
of you is reading them: `press a` clicks whatever the pointer is on.
"""

import argparse
import sys
from pathlib import Path

from . import script
from .go import REPO, LegionGo
from .profile import load_all
from .targets import Devices, descriptors


def interactive(go):
    print("Devices are up. %s\nOne command a line, or 'help'. Control-D stops."
          % ", ".join("%s at %s" % (r, p) for r, p in sorted(go.devices.paths().items())))
    for line in sys.stdin:
        line = line.strip()
        if line in ("help", "?"):
            print(script.VERBS)
            continue
        if line in ("quit", "exit"):
            break
        try:
            script.step(go, line)
        except (ValueError, KeyError, IndexError) as exc:
            print("%s" % exc, file=sys.stderr)


def what(names, root):
    profiles = load_all(root)
    for spoken in names:
        print(spoken)
        for name, profile in sorted(profiles.items()):
            mappings = profile.for_button(spoken)
            if not mappings:
                does = "passed through untouched" if not profile.mappings \
                       else "nothing"
                print("  %-9s %s" % (name, does))
                continue
            for mapping in mappings:
                print("  %-9s %s  %s" % (name, mapping.does or "nothing yet",
                                         "".join("[%s %s]" % (t.kind, t.name)
                                                 for t in mapping.targets)))
        print()


def show_devices():
    for role, descriptor in sorted(descriptors().items()):
        kinds = ", ".join("%s×%d" % (k, len(v))
                          for k, v in sorted(descriptor["capabilities"].items()))
        print("%-9s %-34s %s" % (role, descriptor["name"], kinds))


def main(argv=None):
    parser = argparse.ArgumentParser(
        prog="legion-emulate", description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--profile", default="desktop",
                        help="which profile the presses go through")
    parser.add_argument("--root", default=REPO, type=Path,
                        help="the checkout the profiles are read from")
    sub = parser.add_subparsers(dest="command")
    pressing = sub.add_parser("press", help="press buttons and stop")
    pressing.add_argument("buttons", nargs="+")
    running = sub.add_parser("run", help="play a scenario")
    running.add_argument("scenario", type=Path)
    asking = sub.add_parser("what", help="what a button does")
    asking.add_argument("buttons", nargs="+")
    sub.add_parser("devices", help="what the emulator publishes")
    args = parser.parse_args(argv)

    if args.command == "what":
        what(args.buttons, args.root)
        return 0
    if args.command == "devices":
        show_devices()
        return 0

    try:
        devices = Devices()
    except PermissionError:
        print("legion-emulate: no way in to /dev/uinput. Tests that need no "
              "devices at all are `make test`; see docs/emulator.md for the "
              "one rule that grants this.", file=sys.stderr)
        return 1

    with LegionGo(devices=devices, root=args.root, profile=args.profile) as go:
        if args.command == "press":
            for button in args.buttons:
                go.press(button)
        elif args.command == "run":
            script.play(go, args.scenario.read_text())
        else:
            interactive(go)
    return 0


if __name__ == "__main__":
    sys.exit(main())
