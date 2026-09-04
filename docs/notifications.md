# Notifications

What this desktop says to somebody who is not in a terminal. A fault it met, a
wallpaper it has set going, the dictation saying it is listening: one card,
top right, under the bar.

`mako` draws them and `libnotify`'s `notify-send` is how everything here
speaks to it. Between the two sits `console-say`, which counts.

## The name

`org.freedesktop.Notifications` has exactly one owner on a session bus, and for
a year on this machine that owner was nobody.

The only file claiming the name was `plasma-workspace`'s, which is installed
because `cachyos-handheld` wants the login manager and the login manager wants
it. Under Hyprland the service it names cannot start. So every notification on
the device -- every fault, every press of the dictation paddle -- was answered
by D-Bus starting a program that failed, fifty seconds later, with the caller
waiting the whole time.

Nothing said this. `notify-send` prints its complaint to a stderr nobody was
reading, the desktop went on working, and the promise in `console-say` that a
fault reaches the screen had never once been kept.

The package cannot be removed without taking the login manager with it, so the
name is taken instead. `/usr/local/share/dbus-1/services/` is searched before
`/usr/share/`, and the file there names `console-notify.service`, so anything
that asks for the name before the desktop is up starts our mako rather than
KDE's dead one. With the desktop up the question never arises: the target has
already started it and it already holds the name.

## One unit may watch a bus name

Taking the name was half of it. mako's own package ships
`/usr/lib/systemd/user/mako.service`, which declares the same `Type=dbus` and
the same `BusName`, and a user manager lets exactly one unit watch a bus name:
the second to load is refused with `EEXIST`. Units under `/usr/lib` are loaded
first, so mako's won and `console-notify.service` lost.

What that looks like is nothing. A refused unit is `LoadState=error` and simply
never runs; `console check` counted it as one line among fifty saying ok, and
notifications went on reaching the screen, because the package's unit ran the
same mako and took the same name. So the unit this repository writes was dead
from the day it was written and the desktop looked exactly as though it were
not.

What was lost is everything the unit around mako is for. `ExecStopPost` never
ran, so a mako that died said nothing -- on the one daemon whose whole purpose
is that a thing which broke while nobody was looking is still there when
somebody looks. There is no `Restart=` on the package's unit, and it is
`PartOf=graphical-session.target` rather than this desktop's.

So `mako.service` is masked, under `[masked]` in the manifest beside the
autologin unit. Masked rather than removed, for the same reason the name is
taken rather than the package: the unit belongs to mako, and mako is wanted
here.

## Five seconds, or until it is seen

Everything is drawn the same, out of `theme/palette.toml`, on the panel colour
every other card in front of the wallpaper is drawn on. The border is what says
which kind it is, because it is the only part that nothing has to stay readable
against: soft for low, coral for critical, the ordinary edge for the rest.

A notification goes after five seconds. Critical ones do not go at all, and
everything `console-say` raises is critical, because the whole point of it is
that a thing which broke while nobody was looking is still there when somebody
looks.

## The one reading that raises its own card

The screen and the volume both say where they got to, and both are raised by
the press that caused them. The battery has the same shape of reading and no
press: it moves while nobody is doing anything, so something has to be watching
and whatever watches has to decide when a crossing happened rather than when a
number was read.

Three crossings, and each is a number a person sets on the Battery tab. Getting
low is a card that goes by itself; getting really low is a card that stays,
because it is asking for a cable; and the third stops the machine before the
battery does. The first two sit where the icon on the bar already changes
colour, so the card and the icon say the same thing at the same moment. Any of
them can be walked down to *never*.

`bar-say battery` is what watches, and it is the only thing on the machine
reading the battery at all: it takes a reading every thirty seconds for the
icon it draws, and a second program on a second clock would be two opinions
about when one battery crossed something. What a crossing *is* --
`console_defaults::battery` -- is a function of the reading, the levels and
what has already been said, so it can be asked without a battery. What is done
about one is `console-battery`, a program of its own, because the third of them
waits a quarter of a minute under a card and a bar module is not a thing that
should hold still for that.

Which couples the battery to the bar, and that is worth saying out loud: if
waybar is not running, nothing is reading the battery and none of the three
happens. It is the right trade all the same. A watcher of its own would be a
second program reading the same two files on a second clock, which is the thing
this desktop keeps arriving at as the mistake, and a machine with no bar on it
is a machine with rather more wrong than an unwatched battery.

What is said once is not said again while the charge stays under it, so a
machine sitting at nineteen per cent says so once; a charge that climbs three
points clear arms it again, which is more than a reading wobbles and less than
a step of the level; a reading that falls through two steps owes the deeper
card, because being told the machine is stopping and then that it is getting
low is a machine reading its own list out backwards; and on the mains nothing
is said at all, since a machine that stopped itself while it was filling would
be doing the one thing this is here to prevent. What has been said is kept in
the runtime directory rather than in the process, because waybar starts a
module again the moment it exits.

### What "stop" means depends on the machine

Hibernating is the answer everybody wants: the session goes to disk, the
machine goes off, and plugging in puts it all back. This handheld cannot. Its
only swap is zram, which is memory, and nothing on the kernel command line
names a device to come back from -- `/sys/power/resume` reads `0:0` -- so
logind answers `na` when it is asked, and it is right to.

So `console_settings::stopping` asks the kernel what this machine can do and
the card says which of the two it will be. A device with a real swap partition
hibernates and is told everything will be where it left it. This one shuts
down, and is told plainly that what is open will not be saved, because a card
that promised otherwise would be a lie written where somebody goes to trust it.

Shutting down is still the right answer of the three available. Sleeping keeps
the session in the memory the failing battery is what powers, so a suspend at
five per cent is the session lost in an hour and a hard cut when the cell
empties -- and `hypridle.conf` already refuses to sleep this machine
unattended, for the separate reason that nothing here has ever proved it wakes.
Doing nothing is the same loss with a dirty filesystem and a cell taken to
nought, which is the one thing that damages a battery rather than merely
emptying it.

Fifteen seconds sit between the card and the stopping, and the card says how
many. At five per cent there are minutes left rather than seconds, so the wait
costs nothing; the battery is looked at once a second through it, and a cable
going in replaces the card with one saying nothing was stopped. The journal is
told either way, because a machine found off in the morning is a question.

## The bell

`bar-notice` counts what mako is holding and the bar draws it on the right,
beside the tray. Lit with a number when something is waiting, soft and empty
when nothing is -- the same soft the bar wears for bluetooth that is off and
music that is not playing. A tap opens the panel, and a second tap puts it
away, which is how every icon along that edge works.

It is nearly always a fault, for the reason above: everything else has taken
itself down by the time it could be counted.

The count is not polled. `busctl --user monitor` watches the one name, which
catches both halves -- the call that raises a notification and the signal that
says one has closed, whether a thumb took it down or it ran out of seconds --
and a ten second tick sits under that as the net. The compositor is watched
beside it, because the bell lights while its own panel is in front and nothing
else says when that changed.

## The strip

Four pixels under the bar, the width of the screen, the colour of the bar. It
fills from the left while `console apply` runs and is invisible the rest of the
time.

It exists because a card cannot answer the one question an apply raises. An
apply is minutes -- pacman, a release build of every program on the machine,
sixty files, two profiles, a dozen services -- and the card that goes up says
one is running and then says the same sentence for the whole of it. Somebody
standing over the device is not asking what it is doing; the lines already say
that. They are asking whether to keep standing there.

The fill is weighted rather than counted. `going.rs` holds what share of an
apply each of the thirteen stretches usually is, and the build is 62 of the
hundred on its own. A strip that moved a thirteenth per stretch would sit near
the left through the minutes of the build and then jump to the end, which is a
strip that lies twice. Weighted, it crawls at the start, where the time is, and
runs at the finish, where there is nothing left to wait for. `CONSOLE_TIMINGS=1
console apply` prints what each stretch actually took, which is how the numbers
are corrected.

Nothing polls it. The engine writes `/run/console/updating` and signals waybar
once a stretch -- thirteen wake-ups across a whole apply, and none at all on a
desktop where nothing is being applied. The engine is root's and the bar is
hers, so a file under `/run` and a real-time signal are the only things that
cross between them.

It is a second waybar bar rather than a module on the first, because it runs
the width of the screen and nothing on a bar does that. waybar has no progress
widget of any kind: a custom module hands over text, a tooltip and a class, and
the class is all the stylesheet gets. So `bar-updating` sends `at-0` through
`at-100` in fives and `style.css` has a rule for each, filling a gradient to
that mark. Two lists in two languages, held together by
`every_step_the_bar_can_send_is_one_the_style_paints`.

The four pixels are bought rather than taken: they are reserved whether an
apply is running or not. A strip that appeared only during one would shove
every window on the screen down and back again, and one that reserved nothing
would sit on top of the bar instead of under it. Idle, it is the same colour as
the bar above it, so what it reads as is the bar being four pixels taller.

## The panel

`notices-panel`, and it is the same card as the settings and the files:
[`docs/panels.md`](panels.md) is how it is built and what its buttons promise.
Two tabs, because a notification is in one of two states.

**Waiting** is what is on the screen now, a row each. A row opens onto the
whole of what it said -- who said it, the summary, and the body under them --
and that page is the only place the body can be read. A card is 320 by 140 and
the body is the half that does not fit: the summary says a thing broke and the
body says which service it was. Under the notifications is **Clear them all**,
which is what the bell's tap used to do on its own.

The body needs mako 1.11, which is where `makoctl list -j` arrived, and the
device has it. On 1.10 the flag is ignored rather than refused and the printed
form comes back instead, which carries no body at all: the panel would list
what is waiting and open onto a name and a summary with nothing under them.

`console_notices::reading` reads both shapes and lets the answer say which mako
it is, so nothing asks and nothing has to be right about the version. The
printed form is kept for the bell as much as for the panel. It is the shape the
count was built on and the only one this device has ever been seen to print,
and a bell that goes permanently empty is worse than no bell: it is a reading,
and it is wrong.

Nothing is asked before clearing. What is cleared is in Earlier a moment
later, so it is a press that moves things rather than one that throws them
away, and a question about a press that can be walked back is a question
somebody learns to answer without reading it.

**Earlier** is mako's history buffer and nothing else. It keeps the last twenty,
dismissed and expired alike, and it is read rather than chosen: there is
nothing to do to a notification that has already gone. Both halves are drawn at
once, in the two columns a row that is only read is given, so the tab can be
gone down without opening anything.

There was no panel and no history for a long time, on the argument that what is
on the screen is what there is and the journal has the rest. The journal is not
a place anybody holding a handheld stands, which is the same argument the top
of this page makes about a fault that reached a stderr nobody was reading.

## Quiet, without going deaf

The last row of Waiting keeps cards off the screen. It is mako's
`do-not-disturb` mode, added and removed with `makoctl mode -t`, and
`~/.config/mako/config` is where it is given its meaning: one criteria, setting
`invisible`.

That is the whole of what it does. What was sent is still held, the bell still
counts it, and the bell still turns coral for a fault -- it only wears a
struck-through glyph to say the card is not coming. A handheld is held in front
of a game as often as it is worked on, and the thing worth stopping is the
interruption rather than the news. A mode that threw notifications away would
be a desktop that had quietly stopped saying what broke, which is this page's
own fault arrived at from the other end.

So the bell is the one thing that says the desktop has been quietened. The
cards are gone by definition, and nothing else on the screen would tell you.

    journalctl --user -t console        every fault console-say has counted
    makoctl list -j                     what is waiting right now
    makoctl history -j                  what the Earlier tab is
    makoctl mode                        whether they are being held back
