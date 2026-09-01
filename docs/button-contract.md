# What the buttons promise

This is the buttons. [`docs/panels.md`](panels.md) is how a surface is built so
that what is written here can be kept.

A person holding this thing learns a few buttons once and then stops thinking
about them. That only holds if the answer is the same in every program. What a
button means used to be decided in four separate files; it is decided in one
now. `crates/console-controller/src/means.rs` is a table of everything this
desktop does, when each of them applies, and what it is bound to, and the
daemon carries it out, the setup screen writes to it and the guide reads it out
loud. What is written below is what that table says, and
`crates/console-controller/tests/what_reaches_the_desktop.rs` is where it is
held to it rather than remembered.

| | |
| --- | --- |
| **D-pad** | Moves between things: options in a list, windows, the pages of a chooser. It never does anything, it only goes somewhere |
| **A** | Accepts. Whatever is highlighted, that one. On the desktop, where nothing is highlighted, accepting is clicking what the pointer is on |
| **B** | Goes back. Cancels a chooser, closes what is open, and deletes in the keyboard |
| **X** | Shows the keyboard, and puts it away again, wherever you are |
| **Y** | Is not spoken for by the desktop, which makes it the one that can be lent. A panel lends it *what else can be done with this row*; a page lends it a label over everything on it that can be pressed. Nothing may quietly give it a job one of the others already owns |

X is the one of those that is not the daemon's on both ends, and the round trip
is worth following once. Under the profile the desktop wears, X arrives as a
key the on-screen keyboard's own fork cannot see, and the daemon raises the
keyboard. While the keyboard is up the pad wears `keyboard.yaml`, which
translates nothing and passes everything through, and the daemon acts on
nothing at all -- so the second press reaches the fork as the pad button it
always was, and the keyboard puts itself away. One button, one promise, and two
programs that never both act on it.

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

## And the other way round

Anything a finger can press, a thumb has to be able to reach. The table above
is read from the left, and it is as true read from the right: a mark drawn on
the screen that no button walks onto is a thing that can be looked at and not
pressed, which is the same fault wearing the other face.

The **×** was that. B closed the panel all along, so nobody was shut in, but a
thumb walking the top of a panel came to the last tab and stopped in front of a
mark that plainly meant something. It is a place along the top now, one press
of a shoulder past the last tab, and A on it closes the card. The **‹** and
**›** are not: they are the shoulders drawn for a finger, so a shoulder that
walked onto one would be a button pressing a picture of itself.

## The layers, and what belongs on them

L2 held is a second meaning for a button that already has one. R2 held is a
third, and both together a fourth, and none of the three is a thing a profile
could say: a mapping is one source event to one target, with nothing in it
about what another control was doing at the time. The daemon reads both
triggers as the axes they are and asks which of the four layers is being held,
in the same table that says what the button does. A layer is a column there
rather than a feature of the machine.

Only L2 carries anything. R2 and the two together are empty on purpose, and
they are the room this leaves: a job can be moved onto a chord nothing else is
on without taking a button off anything, and the guide draws a section for a
layer only once there is something in it, so a machine nobody has changed does
not show two headings with nothing under them.

The brightness has always been on L2. The volume and the screenshot are there
now.

The d-pad carries both levels, in the shape the hand already expects: across
for the one you look at, up and down for the one you hear. The rocker on the
top edge is still the volume and nothing here takes it away -- it is a keyboard
of its own and never comes through a profile, which is why it is not in the
daemon's table and cannot be. So this is not two buttons doing one job, which
the table below forbids. It is one job reachable without moving the hand that
is holding the machine up, which is the whole difference between a level you
can set and a level you can set while you are doing something else.

Both of them say where they got to. A press on this layer happens with a game
or a page in front of it, so the only place the reading can go is a
notification: a card that fills to the level it reached, replaced by the next
press rather than stacked under it. The screen needs that more than the volume
does: a volume that moved is a volume you can hear move, while brightness is
the one setting whose effect cannot be judged by looking at it, because looking
at it is what the eyes have just adapted to.

What puts a job on the layer is not how rarely the job is wanted, it is where
the button is. The four paddles are behind the machine, under the fingers that
hold it up rather than under a thumb reaching for something, so a job on a
bare paddle is a job that happens while nobody is doing anything at all. The
screenshot was exactly that: ninety-six pictures in two days, not one of them
asked for, every one of them left in the folder somebody then has to go through.
The menu, closing and dictation stay bare, because each of those announces
itself the instant it happens and is undone by pressing something else. A
picture is filed in silence and is only ever found afterwards.

The button still sends what it always sends, whatever is held with it: the
layer is not a mode the pad is put into, it is a question asked about the same
press. Both triggers reach the desktop as triggers all the while, because they
are two of the three things the profile passes through untouched, and a trigger
is an analogue control before it is a layer.

## The rule that is not about a person

An event only reaches a device the profile lists in `target_devices`.
InputPlumber builds the targets a profile names and destroys the rest, so a
mapping that sends a pad button from a profile with no pad in it sends it
nowhere, and sends it nowhere silently. Every profile publishing the same three
devices also keeps a profile switch from destroying one and building it again,
which is worth avoiding on its own: the compositor does not deliver anything
from a keyboard that appeared after it started.

There used to be two profiles that swapped on the way in and out of every menu,
which is that fault happening several times a minute. Each swap destroyed the
pad and built another: the on-screen keyboard read one directly, so X was dead
for as long as a chooser was open, and the controller daemon read from a pad
that had been taken out from under it and fell over. That is gone. The desktop
and a chooser are one profile now and the difference between them is a column
in the daemon's table, so opening a menu changes nothing about what the machine
is wearing. `opening_a_menu_does_not_change_the_profile` is the test that keeps
it that way.

Every button is routed and none is silenced, which is the other half of the
same change. A profile that gave a button `target_events: []` was a profile
deciding that button meant nothing here, and where that decision belongs is the
one table. `QuickAccess2` and `RightPaddle3` are the case that proves it:
nobody who wrote these files knows where on the machine they are, so they are
routed to a key of their own like every other button, they have no job on them,
and the setup screen can move one onto them the moment somebody presses one and
finds out what it is.

`crates/console-pad/tests/the_button_contract.rs` keeps what is left of this
that is genuinely about the files: every word the switcher takes names a
profile that exists, and every profile publishes all three devices.

The front of the machine keeps working with a chooser up. The settings button
opens the settings and the menu button opens the guide, with a menu already up
as much as without one, because a button on the front means one thing wherever
it is pressed -- which is a row in the table saying `Anywhere` rather than two
files agreeing. The settings button used to close what was up instead, which cost
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
and builds another is the thing the section above is about, so every word
`controller-profile` takes now names a file this desktop wrote. Two are in this
repository -- `game.yaml` and `keyboard.yaml`, which are the two that translate
nothing. The other two are written by `console apply` out of what the device
itself says it can send: `router`, which the desktop wears from login to
shutdown, and `asking`, for the reason the last section is about. Neither could
be kept in the tree, because what each holds is one machine's buttons and the
tree is what every machine has in common.

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

## Only one thing acts on the pad

The on-screen keyboard reads the pad itself, and so does the controller daemon.
Both acting on it, the right stick navigates and scrolls at once.

`Mode::acts()` is the whole of what keeps them off each other, and it is a
question about what is on the screen rather than a note one program leaves
another. The daemon acts everywhere except `Mode::Keyboard` and `Mode::Asking`,
and both of those are read off the compositor's own list of layers: the
keyboard is up, or the card that asks which button you just pressed is up. The
daemon goes on reading either way, so nothing queues behind it.

It used to be a `SIGSTOP` sent by `osk-hook`, and the fault that came of it is
worth keeping. Stopped is not deaf: the devices stayed open, the kernel went on
queueing, and the whole backlog arrived in one instant when the keyboard went
away -- every button pressed while typing, in order, against a desktop that had
moved on. That is how the machine once left for Game Mode on its own. The
signal also had to name `--kill-whom=main`, because a control group is
inherited by every child and the menu, the panel and anything opened from the
menu were all in it: named wrongly, raising the keyboard over a panel stopped
the panel, which stayed on screen reading nothing.

The last program still sending it was `console-buttons --identify`, which had
all three faults at once and one more of its own: its `SIGCONT` came after a
loop the program only leaves by being killed, so on the way out it documents --
Ctrl-C -- the daemon was always left stopped, and the backlog stopped is not
deaf about was never delivered at all. A program that has to have the pad to
itself takes it now, with `EVIOCGRAB`. That is the argument for a grab over any
signal, and it is a property a signal cannot be given: the kernel holds it, and
the kernel lets go when the process goes, however it goes. There is nothing to
undo, so there is no path on which the undoing is missed.

`no_program_here_stops_a_unit_with_a_signal` keeps it that way, and how it
came to be written is the point of it. The guard before it walked `files/`,
which held the scripts that used to do this, so when the scripts were deleted
the test went on passing and never looked at a crate. This one walks every
`src` under `crates/`, which is what ships.

The keyboard profile maps nothing, so the keyboard gets the buttons rather than
a translation of them. While it is up, B is the keyboard's backspace. It still
closes a panel, because the panel holds the keyboard focus and reads backspace
as back, so the thumb's habit works and nothing has to be learnt. While a
question is being typed it deletes a letter instead, which is also what the
thumb expects.

## The device this was written for, and every other one

Every profile here is written in a Legion Go's words. Twenty-three buttons are
routed, and five of them -- `LeftPaddle1`, `LeftPaddle2`, `RightPaddle1`,
`RightPaddle2` and `QuickAccess` -- exist because `50-legion_go.yaml` matched
this machine's DMI and read them off hidraw. On an ordinary pad those five name
nothing, and the menu, closing, dictation, the screenshot and the settings are
all on buttons nobody can press. Four of the five are also the ones the
finger's table above answers for, so a device with no touchscreen loses them
twice.

The profile itself is made out of what the machine answered, so on such a
device those five are not in it and nothing is bound to a button that is not
there. The question `console check` asks is the one that follows from that: of
everything this desktop does, what is on a button nothing on this machine can
press? It answers in the words the rows are written in -- "the settings, on
legion-right" -- because a report that named `QuickAccess` would be telling
somebody about a capability rather than about their machine.

None of that stops an install, and it is deliberate that it does not. A desktop
that refuses to install on a device missing one paddle is worse than one that
installs and says which promise it cannot keep, so `console check` grows a
`buttons` section and an apply raises a notice, and neither is counted as
drift: an apply cannot grow a paddle, and a report ending "3 differences,
`console apply` settles them" while one of the three is a button that does not
exist is the engine promising what it cannot do.

Only InputPlumber can answer the question. Half of what this device sends never
appears in `/dev/input` at all, so enumerating input devices would report a
Legion Go with no paddles. The composite device is asked instead, over the
system bus, and it answers in the same words the profiles are written in:
`Gamepad:Button:LeftPaddle1`. A machine that does not answer is reported as not
asked, which is the honest third state and the usual one for the minute after a
boot, while InputPlumber waits for udev.

### Where an answer is kept

In `~/.config/console/buttons.toml`, and only what somebody moved. A job that
is not in the file is a job where this desktop put it, so a machine nobody has
touched has an empty file and the whole of its answer in `means.rs`. It is not
in the manifest and never travels in this repository: it is the one file that
is true of one person's machine and wrong for every other.

Nothing is applied. The daemon reads the file, watches for it changing and
takes the new table up on the next turn of its loop; the setup screen writes
it, and the guide reads it. That is the whole of what a move is. It used to be
a rewrite of a profile's own text, applied on the way to the machine so
`console check` could compare what an apply would write, with a `sudo` and a
reload behind it -- which is what a move had to be while a profile was the
thing that said what a button meant. None of that is needed to change a row in
a table three programs read for themselves, and a button moved is now a button
moved before the thumb is off it.

A binding is a button and whatever is held with it, written the way it is said:
`screenshot = "l2 + right-paddle-bottom"`. A job two buttons do is a list. A
job somebody has taken the button off is `""`, said rather than left out,
because a row that is not in the file is a row that never moved.

What a job is stays this desktop's. Which thumb does it is all the file may
say.

### Asking is a mode

Moving a part onto a button is pressing the button you want. Nobody holding a
handheld knows which paddle `RightPaddle3` is, and a list of names is the worse
screen for the same question.

The card that asks is `console-asking`, and it is a program of its own because
its name is the mechanism: the compositor lists a layer under whatever drew it,
and that layer being up is `Mode::Asking`, which loads the generated `asking`
profile. Under it every button on the device sends a key nothing is listening
for, so a press says which button it was and does nothing else on the way past.
Without that, binding Legion left would leave for Game Mode, X would raise the
keyboard over the question, and the shoulders would carry the window away.

Two parts cannot share a place. A place is a button and whatever is held with
it, so X and L2 + X are two of them and nothing is being shared -- but one
press doing two things is a press whose second job is the one nobody meant, so
pressing a button another part is already on gives it to the part being moved,
and the part that had it is left playing nothing and says **no button** on its
own row, where a press gives it another.

It used to refuse instead, and on the machine these profiles were written for
that made the screen unusable: nineteen parts, twenty-three buttons a thumb can
press, and every button worth pressing already playing something. Four were
free. Almost every press was answered by a refusal, in the profile's own words
-- pressing Y to move the menu said `West is already West`, which names neither
the button nor the job.

The card says both lines now, in the words the rows are written in: what this
part is on, and, underneath, which part has just been left without a button.
Taking one away is the thing here that nobody asked for by name, and it is not
allowed to happen quietly.

Putting it all back is the first row on the page rather than the last, and it
asks before it does anything. Two presses can leave the menu with no button,
and the row that undoes that must not be at the bottom of a list somebody now
has to walk without it.

Twenty-eight jobs and twenty-three buttons is still more jobs than buttons, and
the layers are what keeps that from being a crowd: a button held with a trigger
is a place of its own, so there are four times as many places as there are
buttons and all but a handful of them are empty. Somebody putting a job
somewhere of their own does not have to take it off anything.

A device with more buttons than there are spare keys to lend them is a device
where the last few cannot be asked about. There are twenty-five keys and this
machine has twenty-three buttons a thumb can press.

## Where a change goes

In the daemon's table, never in a compositor binding. Binding buttons to key
combinations and letting Hyprland match them was tried and does not work here:
InputPlumber emits the modifier and the key in one frame, so the key is often
acted on alone and lands in whatever window has focus. That is how pressing X
typed a k into a terminal.

Each job is named in the table in the words a person would use for it -- "a
screenshot", "put away whatever is up" -- and the guide, the setup screen and
the notice `console check` raises all print that one name. Renaming a job
renames it everywhere it is shown, and moving one moves it everywhere it is
shown, which is what a table read by three programs buys.

What is left written by hand is what no table mentions: the volume rocker, the
touchpad, the screen and the bar, which are not buttons at all; the keys inside
the on-screen keyboard, which are wvkbd's; and what a button does inside a
page, which is the browser add-on's. Nothing keeps those honest. Change what
the add-on does with Y and the guide will still say it labels the page.

There is no check for it, and the reason is worth knowing before somebody
writes one. Those rows are about things the table has no row for at all -- a
rocker, a touchscreen, a fork of somebody else's keyboard, an add-on inside a
browser -- so there is nothing to check them against without writing down a
second copy of what those programs do, in the file that was supposed to be
checked against them, and the second copy is the one that goes stale. So the
rule is the narrow one: anything a button on this machine does belongs in the
table, where the guide will read it, and a row written by hand is a promise
somebody has to keep by hand.
