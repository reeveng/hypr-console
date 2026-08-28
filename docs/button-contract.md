# What the buttons promise

A person holding this thing learns a few buttons once and then stops thinking
about them. That only holds if the answer is the same in every program, and
what a button means is decided in four separate files, so it is written here
and checked in `tests/test_button_contract.py` rather than remembered.

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
| **Menu**, **Legion left** | The bar's icons, each opening the panel at its own tab |

`tests/test_manifest.py` holds these to it: the bar has to carry a door for the
menu and one for the keyboard, a panel has to draw a way out, and a level has
to draw its two ends.

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

`tests/test_button_contract.py` reads the daemon's own tables of what it acts
on, so a button given a job there and forgotten in a chooser profile is a
failure rather than something found later with a thumb.

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
the first section and no more. "Held with L2", "Keyboard up" and "In a menu" are
written by hand in `legion-buttons`, and so are the rows in the first section
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
