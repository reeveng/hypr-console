# Checks

    tools/legion-check                       here, against the emulator
    tools/legion-check --list                what there is
    tools/legion-check brightness            only the checks about that
    tools/legion-check --stage device --dry  what it would do to the device
    tools/legion-check --stage device --yes  do it

One file per feature. The number in front is the order they run in. When a
feature changes, edit its file rather than adding a second one.

A check has up to three functions. `here(pad, seen)` is what needs no machine,
`device(pad, seen)` is what only the Legion Go can answer, `check(pad, seen)` is
both. `pad` presses, `seen` looks. If a stage cannot answer something the check
skips and says which part it could not see.

There is a third stage, `--stage desktop`, which runs the device's desktop
nested on this machine and can say what colour the screen is.

Every check that runs without a machine also runs in `make fast`.

## Assert what a person would see

A check has to assert the thing somebody would notice, not the mechanism behind
it.

Three checks pressed B on an open menu and then asked which controller profile
was loaded. The answer was right and all three passed, with a menu still on the
screen that would not close. The question to ask is whether the menu is gone.

Asking the mechanism is still worth doing as a tiebreak. `150-the-wallpaper`
reads five places on the screen first and then asks the wallpaper daemon which
file it is showing, because an empty screen is the same colour as the picture.

## A green check can be a lie

`020` says a held trigger carries the window to the next workspace. It asserted
that the workspace had changed and that the machine still had as many windows as
before. Both are true whether the window came along or stayed behind, so it
passed from the day it was written with the trigger doing nothing at all.

Ask what a check would say if the feature were broken. If the answer is the
same, it is not a check. A green one nobody doubts is worse than a red one,
because it is the thing that was supposed to tell you.

## It is somebody's machine

`--stage device` does nothing without `--yes`, and `--dry` prints what it would
send. Some checks open menus, move between workspaces and close windows. Read a
dry run first.

Ask the person holding the device before running anything on it. A clean tree is
not permission. Another session saying it is finished is not permission. Another
user clearing a deploy is not permission for the run after it. Whoever is
holding the device decides what happens on it.
