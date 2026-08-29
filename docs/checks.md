# Checks

    make checks                             here, against the emulator
    console-check --list                    what there is
    console-check brightness                only the checks about that
    console-check --stage device --dry      what it would do to the device
    console-check --stage device --yes      do it

`console-check` is `cargo run --bin console-check`, and the checks themselves
are `crates/console-checks`, one module per feature. The number in front of a
check's name is the order they run in. When a feature changes, edit its check
rather than adding a second one.

A check is written for the stages that can answer it. `Body::Here` is what needs
no machine, `Body::Device` is what only the Legion Go can answer, and
`Body::Desktop` is what wants a screen to look at: `--stage desktop` runs the
device's desktop nested on this machine and can say what colour it is. A stage
nothing is written for skips and says so. So does a stage that is handed
something it cannot do, which is how `120` and `130` say the device cannot see a
page scroll or send a touch.

Every check that runs without a machine also runs in `cargo test`, so a check
nobody has run since the feature changed cannot survive to fail on the device
for a reason that has nothing to do with the device.

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

`140` was the same shape for longer. It asks whether every service is `active`,
and every one of them restarts itself, so a daemon dying every few minutes is
`active` at almost any moment somebody asks. The wallpaper daemon core-dumped
eight times in a day underneath a green check. `210` is the question that was
missing: how many times has anything had to be started again.

## Wait for the thing, not for a number of seconds

How long a chooser takes to draw is how busy the machine is. A check that sleeps
for a fixed guess passes on a quiet device and fails on the same device behind a
screenshot another check is taking, which is exactly how `180` failed inside the
tier while passing three times out of three on its own. Four checks had the
fault before it was found.

`drawn()` waits for a chooser to arrive and `gone()` waits for every chooser to
leave. Both answer whether it happened rather than failing, so the check says
what it was waiting for in its own words. For anything else there is
`until(what)`, which is what those two are built from. A `settle` with a number
in it is a guess, and a guess in a check is a check that will one day be red for
a reason that is not the feature.

## The first minute after a deploy is a lie

`010` and `011` read the workspace out of the compositor, and for about a minute
after a pacman transaction they read it wrong and then settle on their own. The
tier is most often run straight after a deploy, which is exactly when they lie.
A red `010` on the first run after an install is the machine being honest about
being busy; run it again before believing it.

## It is somebody's machine

`--stage device` does nothing without `--yes`, and `--dry` prints what it would
send. Some checks open menus, move between workspaces and close windows. Read a
dry run first.

Ask the person holding the device before running anything on it. A clean tree is
not permission. Another session saying it is finished is not permission. Another
user clearing a deploy is not permission for the run after it. Whoever is
holding the device decides what happens on it.
