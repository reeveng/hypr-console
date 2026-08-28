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

## Where a change goes

In the profile, never in a compositor binding. Binding buttons to key
combinations and letting Hyprland match them was tried and does not work here:
InputPlumber emits the modifier and the key in one frame, so the key is often
acted on alone and lands in whatever window has focus. That is how pressing X
typed a k into a terminal.

Each mapping is named "Button - what it does", the guide on the device prints
those names, and the tests read them. Renaming a mapping renames it everywhere
it is shown.
