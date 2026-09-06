# Checks

    just checks                             here, against the emulator
    console-check --list                    what there is
    console-check brightness                only the checks about that
    console-check --stage device --dry      what it would do to the device
    console-check --stage device --yes      do it
    console-check --stage device --yes --all   every check written for it

`console-check` is `cargo run --bin console-check`, and the checks themselves
are `crates/console-feature-checks`, one module per feature. The number in front of a
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

## The person holding it is told what is happening

A device run is minutes of somebody's handheld opening menus by itself, and
everything the run says it says on a terminal in another room. So it says two
things on the device as well, and no more than two.

The strip under the bar fills as it goes. That row of pixels already exists --
`console apply` fills it, `console_notifications::updating` is the file both
ends of it agree on -- and a check run writes the same file the same way, from
the laptop end of the ssh. Nothing new is drawn: a panel put up to report the
run would be a layer over the desktop that the checks then have to press
through, and several of them ask what is on the screen and what colour it is.
The surface reporting the run would be the run's own worst interference.

A card is raised when it starts, saying how many checks and about how long, and
replaced by one at the end saying how it went. The strip has its tooltip turned
off and is two pixels tall, so it can say how much is left and nothing else; the
card is where the words go. The one at the end stays on the screen when
something failed, because a run that ends badly while somebody is making tea is
the whole reason to say it twice.

## Longest first, and how it knows

On the device the checks run longest first. Everywhere else they run in the
order they grew, which walks the desktop the way it was built; on the device
that order says nothing anybody watching needs, and it makes the strip crawl
and jump by turns. Longest first puts the wait at the front, so the strip slows
early and runs at the end -- which is not a trick played on the reader, it is
what the run does. `going` weights an apply the same way and for the same
reason.

How long each check takes is measured rather than declared. The `lasting`
module of `console-test-stages` times every check as it runs and hands the
table back to the device, under `~/.local/state/console/checked`, where the
next run reads it. An apply has few enough stretches to carry estimates in
their source and be corrected by hand; the checks do not, arriving one or two
at a time as they do, and a number nobody updates is a bar that lies about a
run somebody is watching.

A check nothing has ever timed is given the middle of what is known -- not a
claim about its length, a claim that it is no more surprising than the rest. A
machine nothing has ever timed gets a strip that counts checks and a card that
promises no length at all, because a run divided into equal checks tells
somebody the ten-second one and the two-minute one are the same wait. One run
fixes that, on that machine, for good.

It costs the order they grew, which is a thing to know before reading a run: a
check that leaned on the one before it would break. None does. Every check has
always had to survive being named on its own.

## The machine is asked only what nothing else can

The device tier is minutes of somebody's handheld, and most of what is written
for it is a second question about a feature the emulator answered here in a
third of a second, before the deploy went out. So `--stage device` asked for
nothing in particular runs only the checks nothing else can answer, and prints
the rest as skipped with where they were answered instead. `--all` is the whole
tier, for a run that is about the hardware rather than the desktop.

Which those are is read off the check rather than listed anywhere: a check with
a `Body::Device` and no other body is the machine's business, and one that grows
an emulator body stops being it the same moment. There is no list to keep in
step, and nothing to forget.

Naming a check is asking for it. `console-check --stage device --yes brightness`
runs brightness on the machine whatever the emulator thinks, which is how a
feature the emulator says is fine and the device disagrees about gets looked at.

The end of a deploy still touches the real hardware: `110` presses X through
InputPlumber and waits for the keyboard, so the whole chain from a button to a
window is walked once even on the short tier. What the short tier gives up is
the second opinion on everything the emulator already answered, and `--all` is
where to get it when a deploy changed something about the machine rather than
about the desktop.

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

Both are answered here now, so only `--all` asks them of the machine at all.
That is most of why the short tier is steadier straight after a deploy, and it
is worth knowing before reading an `--all` run taken in the same minute.

## One panel, on its own

    cargo test -p console-viewer --test a_finger
    cargo test -p console-panel --test every_panel_answers_a_finger the_files

A tier below the three above, and the one a change to a panel is tried in while
it is being written. It opens one panel in the nested desktop, with no bar and
no desktop around it, and asks it what it put on the screen.

A panel says so when `CONSOLE_PANEL_TELLS` names a file: one line per draw, and
in the line every part of itself a hand is offered and the rectangle it occupies
in the room the compositor granted. `console_panel::telling` reads it back and
`console_test_stages::panels` holds it against what the rows were built to
offer. What that can answer is the question none of the other tiers could: not
whether the panel is drawn, but whether a hand could use what is drawn.

The rules are the ones in `button-contract.md`. A row that offers something
behind Y draws a mark a finger can reach, unless it has said out loud that it
wears none. A card with one subject draws one mark for it. Everything drawn to
be pressed is inside the room. There is always a way out. Every panel here broke
the first of those, and no check could see it, because nothing had ever asked a
panel what it had drawn -- only whether it had drawn.

One test per panel, so a name after the test binary runs one of them: about
nine seconds for one panel, most of which is a compositor starting.

Most of the contract does not need any of that, and should not be asked for
here. Whether a row that offers something wears a mark for it is decided from
the row before a widget exists, in one place that every panel's rows pass
through, so `wears` in `console_panel::panel` answers it for all ten panels in a
unit test that costs microseconds. That is where a rule about what a panel
*decides* belongs. What is left for this tier is what only a real screen can
say: where the thing it decided to draw actually landed, measured by GTK with
the real stylesheet at the real size. Every fault this tier has found was of
that kind -- a button whose padding made the measurement lie, a margin that made
the surface wider than the output, a mark right-aligned off the glass -- and not
one of them was visible to any amount of reasoning about rows.

The session it opens is `--bare`: no ground painted, because nothing here looks
at the picture. What is read back is what the panel wrote down. Painting a
wallpaper was the longest thing a run of this did and it was spent asking a
daemon that nothing had started.

It does not open in front of you. Hyprland has no headless-only backend, so a
nested session is a window on whoever's screen started it, and it used to open
on their current workspace in the magenta this stage paints and sit there for
the length of the run. `console-desktop` now asks the running compositor, before
starting the nested one, to put anything of class `aquamarine` on a special
workspace of its own, silently. Nothing is written to anybody's config and the
rule is gone at the next reload.

    hyprctl dispatch togglespecialworkspace console-desktop

is how to watch one while it runs.

The panels take turns. `console_panel::chooser` allows one chooser on a session
at a time and enforces it by asking whoever holds the screen to leave, which is
right on the device and wrong here: the lock belongs to the login session and
the nested sessions are inside it, so two of these running at once are two
panels asking each other to go. The stage takes an exclusive lock for the length
of a run rather than asking anybody to remember a flag.

## It is somebody's machine

`--stage device` does nothing without `--yes`, and `--dry` prints what it would
send. Some checks open menus, move between workspaces and close windows. Read a
dry run first.

Ask the person holding the device before running anything on it. A clean tree is
not permission. Another session saying it is finished is not permission. Another
user clearing a deploy is not permission for the run after it. Whoever is
holding the device decides what happens on it.
