# Everything this desktop has grown, tried again

    tools/legion-check                       here, against the emulator
    tools/legion-check --list                what there is
    tools/legion-check brightness            only the checks about that
    tools/legion-check --stage device --dry  what it would do to the device
    tools/legion-check --stage device --yes  do it

A check is one file and one feature. It says what somebody did with their
thumbs and what should have happened. Running them in order walks everything
this desktop has grown, oldest first, and says which of it still works.

## One file to a feature

The number at the front is when the feature arrived, so the order they run in
is the order they were built in, and a run reads as the history of the machine.

When a feature changes, its file is edited. It does not get a second file
saying something different, because two files describing one button is how a
suite comes to disagree with itself and with the device.

A feature is split only where the parts fail separately. "The d-pad works" is
not a thing that fails: left works or right works, and a check that presses
both and asserts once tells you neither which failed nor that only one did. So
there is a check for the d-pad's left and one for its right, and brightness up
and brightness down are two files rather than one.

## The same check, in two places

    def here(pad, seen):     what can be answered with no machine at all
    def device(pad, seen):   what can only be answered on the Legion Go
    def check(pad, seen):    the same thing in both

`pad` presses. `seen` looks. A check asks `seen` for whatever it needs, and a
stage that cannot answer that does not have the method, so the check skips and
says which thing could not be seen. Nothing declares in advance what it needs:
that would be the same fact written twice, and the second copy is the one that
goes stale.

Here, what can be seen is what the daemon decided to run and what it wrote to
the pointer. On the device, what can be seen is the machine: which workspace,
which windows, how bright, whether the keyboard is up, which controller profile
is loaded, whether every service is running, what appeared in a directory.

Two checks are honest about being answerable in only one place. Nothing on the
device can see a page scroll, and InputPlumber cannot send touch at all, which
is the whole reason the daemon reads the touchpad directly.

## Pressing a button on the real device

Through InputPlumber's own `SendEvent` and `SendButtonChord`, which is how a
chord on the device already works. So a press arrives exactly as the hardware's
would, through whichever profile is loaded, and nothing is created on the
device, nothing is grabbed, and nothing is left behind if a run stops halfway.

The alternative was a uinput device pretending to be the controller, which
risks a second composite device publishing a second pad for the daemons to find
and argue over. This asks the thing that already owns the pad.

## Looking at the screen

There is a third stage, `--stage desktop`, which starts the device's own
desktop nested on this machine and answers what colour the screen is.

It exists because of a fault neither of the others can see. The wallpaper on
the device did not paint for days: hyprpaper 0.8 changed its config format, the
old lines stopped meaning anything, and it did not fail. It started, said the
monitor had no target, painted nothing, and reported success. Nothing was in a
failed state and a service check would have passed. What was on screen was the
compositor's own default, dark enough to pass for a background.

So `150-the-wallpaper` reads the colour most of the screen is and compares it
to the colour most of the carried wallpaper is, which it reads out of the
picture rather than being told. A wallpaper that changes changes the check with
it, and a check that has to be edited whenever the thing it checks changes is
one that will eventually be edited to agree with a fault.

    tools/legion-desktop shot after.png --sample most --sample 512,320

says the same thing by hand, for anything else that paints.

## It is somebody's machine

`--stage device` does nothing without `--yes`. `--dry` prints every command it
would send, and judges nothing, because on a dry run the machine answers
nothing and every assertion would be about that rather than about the desktop.

Some checks change what is on screen: they open the menu, move between
workspaces, close a window. `030-close-the-window` closes whatever is in front
of you. Read the dry run before the real one.

## They are also the fast suite

Every check that can run without a machine runs on every `make fast`. A check
nobody has run since the feature changed is a check that will fail on the
device for a reason that has nothing to do with the device.
