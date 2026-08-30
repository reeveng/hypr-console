# What the buttons promise

This is the buttons. [`docs/panels.md`](panels.md) is how a surface is built so
that what is written here can be kept.

A person holding this thing learns a few buttons once and then stops thinking
about them. That only holds if the answer is the same in every program, and
what a button means is decided in four separate files, so it is written here
and checked in `crates/console-pad/tests/the_button_contract.rs` rather than
remembered.

| | |
| --- | --- |
| **D-pad** | Moves between things: options in a list, windows, the pages of a chooser. It never does anything, it only goes somewhere |
| **A** | Accepts. Whatever is highlighted, that one. On the desktop, where nothing is highlighted, accepting is clicking what the pointer is on |
| **B** | Goes back. Cancels a chooser, closes what is open, and deletes in the keyboard |
| **X** | Shows the keyboard, and puts it away again, wherever you are |
| **Y** | Is not spoken for. It may be lent to something, and nothing may quietly give it a job that one of the others already owns |

The keyboard profile keeps none of these itself. While the on-screen keyboard
is up it reads the pad directly, so that profile translates nothing and passes
everything through, and X closing the keyboard that X opened is the same press
arriving at the same place.

## The same promise, without the controller

The device is a touchscreen and it is put down as often as it is held, so every
one of those has to have an answer for a hand holding nothing. They are not
extra features. A button with no answer here is a thing that can be looked at
and not changed, which is worse than a thing that is missing: the volume was
readable on a panel a finger could open, on a row a finger could only silence.

| Button | The finger's answer |
| --- | --- |
| **D-pad** | Touching a row moves the highlight to it, so where you are and where your finger is are one answer |
| **A** | Tap the row |
| **B** | The **×** at the end of the tab strip, and for the menu, the bar icon that opened it |
| **X** | The keyboard icon on the bar |
| Left and right on a level | The **−** and **+** on the row that carries it |
| **L1** and **R1** | The **‹** and **›** either side of the tab strip, which appear when there are tabs it has no room for |
| **Legion right**, **left paddle, top** | The bar's icons: the settings, each at its own tab, and the menu |
| **Legion left** | The **Game Mode** row on the panel's System tab. Coming back the other way is the button held, and a finger has Steam's own **Switch to Desktop** |
| **Left paddle, bottom** | Dictation, in the menu, which starts and stops the same way the paddle does |
| **View** | The browser, in the menu |

`crates/console-manifest/tests/the_tree.rs` holds these to it: the bar has to
carry a door for the menu and one for the keyboard, a panel has to draw a way
out, a level has to draw its two ends, and a strip that hides a tab has to draw
the way to it.

## The rule that is not about a person

An event only reaches a device the profile lists in `target_devices`.
InputPlumber builds the targets a profile names and destroys the rest, so a
mapping that sends a pad button from a profile with no pad in it sends it
nowhere, and sends it nowhere silently. Every profile publishing the same three
devices also keeps a profile switch from destroying one and building it again,
which is worth avoiding on its own: the compositor does not deliver anything
from a keyboard that appeared after it started.

The two chooser profiles publish a pad for a second reason: the on-screen
keyboard reads one directly, so without a pad X was dead for as long as a
chooser was open, and the label promising otherwise could not be kept. The same
disappearance is what crashed the controller daemon, which read from a pad that
had been destroyed under it.

Publishing a pad means every button reaches one whether or not the profile has
anything to say about it. So the buttons that would otherwise act behind an
open chooser are named and given `target_events: []`, which means the same
thing whether an unmapped button is passed through or dropped. Nothing here
rests on knowing which of those InputPlumber does, and no test had to guess it.

`crates/console-pad/tests/the_button_contract.rs` reads the daemon's own tables
of what it acts on, so a button given a job there and forgotten in a chooser
profile is a failure rather than something found later with a thumb.

The front of the machine is not silenced with them. The settings button opens
the settings and the menu button opens the guide, with a chooser already up as
much as without one, because a button on the front means one thing wherever it
is pressed. The settings button used to close what was up instead, which cost
two presses to reach the settings from the menu and made the first of them look
like a machine ignoring a thumb. Nothing was lost by taking closing off it: B
backs out and the right paddle closes. Leaving for Game Mode is the one
exception and stays quiet, because that is not a thing to do by brushing a
button with a menu open.

## The one button that means something on both sides

Legion left leaves this desktop for Game Mode, and Legion left held is what
comes back. One button for the door, whichever side of it you are on, because
somebody who learned the press should not have to learn a second answer for the
other direction.

A hold there and a press here, because on that side the button is Steam's.
Taken outright it would cost Game Mode its own menu, which is where the
library, the power and the way out of a game are, and a machine that cannot
quit a game is worse off than one that takes a second to leave. So the press
arrives at Steam untouched and opens what it always opened. Only keeping it
down for a second means anything to us. It also has to be held alone: Steam's
own shortcuts are that button and another one together, and holding Steam and B
until a game gives up is somebody staying in Game Mode rather than leaving it.

Nothing translates it, so there is no mapping to read this off. `game.yaml`
publishes the same three devices as every other profile and maps nothing at
all, and `game-return` reads the pad the way the desktop's own daemon does. It
is a program of its own because the desktop's daemon is not there: Game Mode
stops `console.target` behind it. `console-return.service` is started by the
Game Mode session itself, through the drop-in at
`/etc/systemd/user/gamescope-session.service.d/console.conf`, rather than
enabled for the user, which would leave it reading the pad on the desktop too,
behind the daemon that already reads it.

Game Mode kept the shipped Default profile until this was written, and what
that publishes was nobody's decision. A profile switch that destroys a target
and builds another is the thing the section above is about, so the four words
`controller-profile` takes now all name a file this repository holds.

Reading a pad somebody else is also reading is the ordinary state of things
here, and the section below is the other half of it: on the desktop the
on-screen keyboard and the controller daemon hold that node at once. Steam does
not take it either, which had to be read off the machine rather than reasoned
about: a grab is in neither the node's permissions nor `fuser`, and a second
reader that has lost is a second reader receiving nothing. Asked with an
`EVIOCGRAB` taken and given back while Game Mode had the screen, the published
pad came back free, against the real hardware as a control, which InputPlumber
does grab. So the pad is read here the same way the desktop reads it, and Game
Mode's profile stays the one that translates nothing.

`crates/console-controller/tests/the_controller.rs` presses the button on both
sides: the press that is Steam's, the hold that comes back, the chord that does
not, and every other button reaching the pad as itself.

## Only one thing holds the pad

The on-screen keyboard reads the pad itself, and so does the controller daemon.
Both reading it, the right stick navigates and scrolls at once, so while the
keyboard is up the daemon is stopped and started again after.

`osk-hook` names `--kill-whom=main` when it does that. Without it the signal
goes to every process in the unit's control group, and the menu, the panel and
anything opened from the menu are all in that group: a control group is
inherited by every child, and nothing a program can do to itself leaves one.
Starting them in a session of their own was tried and fixed nothing. Named
wrongly, raising the keyboard over a panel stopped the panel, which stayed on
screen reading nothing until the keyboard went away.

The keyboard profile maps nothing, so the keyboard gets the buttons rather than
a translation of them. While it is up, B is the keyboard's backspace. It still
closes a panel, because the panel holds the keyboard focus and reads backspace
as back, so the thumb's habit works and nothing has to be learnt. While a
question is being typed it deletes a letter instead, which is also what the
thumb expects.

## Where a change goes

In the profile, never in a compositor binding. Binding buttons to key
combinations and letting Hyprland match them was tried and does not work here:
InputPlumber emits the modifier and the key in one frame, so the key is often
acted on alone and lands in whatever window has focus. That is how pressing X
typed a k into a terminal.

Each mapping is named "Button - what it does", the guide on the device prints
those names, and the tests read them. Renaming a mapping renames it everywhere
it is shown.

That holds for the part of the guide that is read out of the profiles, which is
the first section and no more. "L2", "Keyboard", "Menus" and "Files" are
written by hand in `console-buttons`, and so are the rows in the first section
about hardware no profile mentions: the volume rocker, the touchpad, the screen
and the bar. Nothing keeps those honest. Change what the right paddle does in
`tabs.yaml` and the guide will still say it closes the menu.

There is no check for it and the reason is worth knowing before somebody writes
one. The hand-written rows are phrases a person reads, "D-pad left / right",
"Right paddle, top", "L1 / R1", and the profiles hold button names. Telling a
stale row from a true one means a table from one to the other, which is the
profile written down a second time in the file that was supposed to be checked
against it, and the second copy is the one that goes stale. So the rule is the
narrow one: a row about a mapping belongs in the profile where the guide will
read it, and a row written by hand is a promise somebody has to keep by hand.
