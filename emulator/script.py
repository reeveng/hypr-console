"""A scenario: what somebody did with their thumbs, written down.

    profile desktop
    press left-paddle-top
    wait 0.3
    press dpad-down
    press a

The same lines drive the emulator whether it is making real devices for the
desktop in front of you or fake ones inside a test, which is the point of
having them: what was tried by hand is what gets kept as a test.
"""

import time


def _number(word):
    try:
        return float(word)
    except ValueError:
        raise ValueError("%r is not a number" % word) from None


def step(go, line, sleep=time.sleep):
    """One line, done. Returns what it was, for anything that wants to say."""
    words = line.split("#", 1)[0].split()
    if not words:
        return None
    verb, rest = words[0], words[1:]

    if verb == "profile":
        go.load_profile(rest[0])
    elif verb == "press":
        for button in rest:
            go.press(button)
    elif verb == "hold":
        for button in rest:
            go.hold(button)
    elif verb == "release":
        if rest:
            for button in rest:
                go.release(button)
        else:
            go.release_all()
    elif verb == "stick":
        go.stick(rest[0] if rest[0].endswith("-stick") else rest[0] + "-stick",
                 _number(rest[1]), _number(rest[2]))
    elif verb == "centre":
        go.centre(rest[0] if rest[0].endswith("-stick") else rest[0] + "-stick")
    elif verb == "trigger":
        go.trigger(rest[0], _number(rest[1]))
    elif verb == "tap":
        go.tap(*(int(_number(w)) for w in rest[:2]) if rest else ())
    elif verb == "drag":
        x0, y0, x1, y1 = (int(_number(w)) for w in rest[:4])
        go.drag((x0, y0), (x1, y1), seconds=_number(rest[4]) if rest[4:] else 0.0)
    elif verb == "click":
        go.touch_click(1 if rest[0] in ("down", "1") else 0)
    elif verb == "wait":
        sleep(_number(rest[0]))
    else:
        raise ValueError("no such thing as %r" % verb)
    return (verb, rest)


def play(go, text, sleep=time.sleep):
    """Every line of a scenario, in order."""
    done = []
    for number, line in enumerate(text.splitlines(), start=1):
        try:
            was = step(go, line, sleep=sleep)
        except (ValueError, KeyError, IndexError) as exc:
            raise ValueError("line %d: %s" % (number, exc)) from None
        if was is not None:
            done.append(was)
    return done


VERBS = """  profile <name>            which profile the presses go through
  press <button>...         press and let go
  hold <button>...          press and keep pressing
  release [<button>...]     let go, of everything if nothing is named
  stick left|right <x> <y>  push a stick, each axis from -1 to 1
  centre left|right         let it go back
  trigger l2|r2 <amount>    pull a trigger, from 0 to 1
  tap [<x> <y>]             a quick touch on the touchpad
  drag <x> <y> <x> <y> [s]  a finger from one place to another
  click down|up             press the touchpad in, and let it out
  wait <seconds>            do nothing for a moment"""
