# Notifications

What this desktop says to someone who is not in a terminal. A fault it met, a
wallpaper it has set going, the dictation saying it is listening: one card,
top right, under the bar.

`console-notify` draws them and `libnotify`'s `notify-send` is how everything
here speaks to it. Between the two sits `console-say`, which counts.

## The name

`org.freedesktop.Notifications` has exactly one owner on a session bus, and for
a year on this machine that owner was no one.

The only file claiming the name was `plasma-workspace`'s, which is installed
because `cachyos-handheld` wants the login manager and the login manager wants
it. Under Hyprland the service it names cannot start. So every notification on
the device -- every fault, every press of the dictation paddle -- was answered
by D-Bus starting a program that failed, fifty seconds later, with the caller
waiting the whole time.

Nothing said this. `notify-send` prints its complaint to a stderr no one was
reading, the desktop went on working, and the promise in `console-say` that a
fault reaches the screen had never once been kept.

The package cannot be removed without taking the login manager with it, so the
name is taken instead. `/usr/local/share/dbus-1/services/` is searched before
`/usr/share/`, and the file there names `console-notify.service`, so anything
that asks for the name before the desktop is up starts this desktop's own
daemon rather than KDE's dead one. With the desktop up the question never
arises: the target has already started it and it already holds the name.

## One unit may watch a bus name

Taking the name was half of it. mako's own package shipped
`/usr/lib/systemd/user/mako.service`, which declared the same `Type=dbus` and
the same `BusName`, and a user manager lets exactly one unit watch a bus name:
the second to load is refused with `EEXIST`. Units under `/usr/lib` are loaded
first, so mako's won and `console-notify.service` lost.

What that looks like is nothing. A refused unit is `LoadState=error` and simply
never runs; `console check` counted it as one line among fifty saying ok, and
notifications went on reaching the screen, because the package's unit ran the
same mako and took the same name. So the unit this repository writes was dead
from the day it was written and the desktop looked exactly as though it were
not.

What was lost is everything the unit around a notification daemon is for.
`ExecStopPost` never ran, so a daemon that died said nothing -- on the one
daemon whose whole purpose is that a thing which broke while no one was looking
is still there when someone looks. There is no `Restart=` on the package's
unit, and it is `PartOf=graphical-session.target` rather than this desktop's.

`mako.service` is masked, under `[masked]` in the manifest beside the autologin
unit, and it stays masked now that mako has left `[packages]`. An apply installs
a name and has never removed one, so every device that applied an earlier commit
is still holding the package and still has that unit under `/usr/lib` waiting to
win the same race. On a machine that never had mako the mask is a symlink to
`/dev/null` for a unit nothing was going to load anyway.

## What draws the card

mako was the last surface on this desktop drawn in someone else's colors.
Every other half of a notification was already in this tree -- `console-say`
is what raises one, `console_notifications::reading` and `rows` are what keep
and draw it afterwards, and what was asked of `makoctl` was a list, a mode and
a dismissal -- so what was left in the package was a socket, a timer and a card
on a layer surface, and a card on a layer surface is what every panel in this
repository already is.

`console-notify` is that. It answers `Notify`, `CloseNotification`,
`GetCapabilities` and `GetServerInformation`, emits `NotificationClosed` with
the reason the specification asks for, and adds `console.Notifications` beside them,
with `ClearAll` and `Quieten` -- the two presses this desktop makes on its own
daemon and the two the freedesktop interface has no word for.

The wire is `console-bus`, written here rather than taken from zbus. What a
notification daemon needs is a connection, a name and one signature --
`susssasa{sv}i` -- and zbus brings an async runtime, a proc-macro layer and a
type system for a bus this desktop speaks to no one else on. What is here is a
header, an alignment table and a walk over a signature. `tests/the_bus.rs`
takes a name on a live session bus and has `busctl` call back into it, which is
the only way to be sure of a wire format: a marshaller tested against its own
reader agrees with itself and with nothing else.

What decides is `serving`, and it has no socket and no screen in it. A message
goes in; a reply, the signals to emit, what changed and what to arm a timer for
come out. That is what makes the awkward half of a notification daemon
answerable without one -- an id can be raised again while its own card is still
up, and a count carried through the timer is what stops the first card's expiry
taking down the second. Nothing about that is visible on a screen until the day
it goes wrong.

What this must not become is a general notification server. No actions, no icon
data, no fd passing, no hints beyond urgency and progress, because everything
that raises a notification here is named in one module. The day a program
no one wrote raises one is the day that question is worth answering, and
the backlog says what it would take.

## Five seconds, or until it is seen

Everything is drawn the same, out of `theme/palette.toml`, on the panel color
every other card in front of the wallpaper is drawn on. Which used to mean a
second stylesheet: `console-palette` wrote mako's colors into
`~/.config/mako/config` in mako's own spelling, and a daemon reads its config
once, when it starts. So `just theme` on a running desktop changed every surface
except the one that arrives uninvited, and it went on being yesterday's palette
until something restarted it. The card loads `console_panel::style::sheet()`
now, which is the sheet every panel loads.

The border is what says which kind it is, because it is the only part that
nothing has to stay readable against: soft for low, coral for critical, the
ordinary edge for the rest.

A notification goes after five seconds. Critical ones do not go at all, and
everything `console-say` raises is critical, because the whole point of it is
that a thing which broke while no one was looking is still there when someone
looks.

## What a card says

A name and a sentence. The summary names the thing a person already knows -- the
words on the unit, the words on the tab -- and says what happened to it: *Status
bar restarted*, *Battery low*, *Update stopped halfway*. The body is one sentence
under it, and it is allowed to be missing.

What was there before was a paragraph. A card that explains why the thing is
worth knowing, what the machine is going to do about it and what to type is four
sentences of argument on a surface that is gone in five seconds, and none of it
is read: someone who has just been handed a card is deciding whether to stop
what they are doing, and that is one word of work. The argument belongs in the
file that decided it and the detail belongs in the journal, which is where the
reason a service fell over is now written rather than on the card.

The names are the same half of it. A unit's `Description=` is what a fall puts at
the top of the card, so it is a name for the thing and not a summary of the crate
behind it: *Desktop events*, not *one subscription per source, handed to whoever
asked for it*. Nothing on the screen is the place to say how something works.

## The one reading that raises its own card

The screen and the volume both say where they got to, and both are raised by
the press that caused them. The battery has the same shape of reading and no
press: it moves while no one is doing anything, so something has to be watching
and whatever watches has to decide when a crossing happened rather than when a
number was read.

Three crossings, and each is a number a person sets on the Battery tab. Getting
low is a card that goes by itself; getting really low is a card that stays,
because it is asking for a cable; and the third stops the machine before the
battery does. The first two sit where the icon on the bar already changes
color, so the card and the icon say the same thing at the same moment. Any of
them can be walked down to *never*.

`console-bar` is what watches -- `dwindling` is that half of it -- and it is
the only thing on the machine reading the battery at all: it takes a reading for
the icon it draws, when udev says a supply changed and on its own tick under
that, and a second program on a second clock would be two opinions about when
one battery crossed something. What a crossing *is* -- the `console-battery`
crate -- is a function of the reading, the levels and what has already been
said, so it can be asked without a battery. What is done about one is the
`console-battery` program in `console-settings`, a program of its own, because the
third of them waits a quarter of a minute under a card and the bar is not a
thing that should be holding still for that.

Which couples the battery to the bar, and that is worth saying out loud: if the
bar is not running, nothing is reading the battery and none of the three
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
the runtime directory rather than in the process, because the bar is restarted
-- by an apply, by a change of screen size -- and what it has already said has
to outlive that.

### What "stop" means depends on the machine

Hibernating is the answer everyone wants: the session goes to disk, the
machine goes off, and plugging in puts it all back. This handheld cannot. Its
only swap is zram, which is memory, and nothing on the kernel command line
names a device to come back from -- `/sys/power/resume` reads `0:0` -- so
logind answers `na` when it is asked, and it is right to.

So `console_settings::stopping` asks the kernel what this machine can do and
the card says which of the two it will be. A device with a real swap partition
hibernates and is told everything will be where it left it. This one shuts
down, and is told plainly that what is open will not be saved, because a card
that promised otherwise would be a lie written where someone goes to trust it.

Shutting down is still the right answer of the three available. Sleeping keeps
the session in the memory the failing battery is what powers, so a suspend at
five per cent is the session lost in an hour and a hard cut when the cell
empties -- and `hypridle.conf` already refuses to sleep this machine
unattended, for the separate reason that nothing here has ever proved it wakes.
Doing nothing is the same loss with a dirty filesystem and a cell taken to
zero, which is the one thing that damages a battery rather than merely
emptying it.

Fifteen seconds sit between the card and the stopping, and the card says how
many. At five per cent there are minutes left rather than seconds, so the wait
costs nothing; the battery is looked at once a second through it, and a cable
going in replaces the card with one saying nothing was stopped. The journal is
told either way, because a machine found off in the morning is a question.

## The bell

The bar counts what `console-notify` is holding and draws it on the right,
beside the tray. Lit with a number when something is waiting, soft and empty
when nothing is -- the same soft the bar wears for bluetooth that is off and
music that is not playing. A tap opens the panel, and a second tap puts it
away, which is how every icon along that edge works.

It is nearly always a fault, for the reason above: everything else has taken
itself down by the time it could be counted.

The count is not polled. `busctl --user monitor` watches the two interfaces the
daemon answers, which catches every half -- the call that raises a notification,
the signal that says one has closed whether a thumb took it down or it ran out
of seconds, and the press that quietens the lot -- and a ten second tick sits
under that as the net. The compositor is watched
beside it, because the bell lights while its own panel is in front and nothing
else says when that changed.

## The strip

Four pixels under the bar, the width of the screen, the color of the bar. It
fills from the left while `console apply` runs and is invisible the rest of the
time.

It exists because a card cannot answer the one question an apply raises. An
apply is minutes -- pacman, a release build of every program on the machine,
sixty files, two profiles, a dozen services -- and the card that goes up says
one is running and then says the same sentence for the whole of it. Someone
standing over the device is not asking what it is doing; the lines already say
that. They are asking whether to keep standing there.

The fill is weighted rather than counted. `going.rs` holds what share of an
apply each stretch usually is, and the build is most of the thousand on its own.
A strip that moved an equal step per stretch would sit near the left through the
minutes of the build and then jump to the end, which is a strip that lies twice.
Weighted, it crawls at the start, where the time is, and runs at the finish,
where there is nothing left to wait for. `CONSOLE_TIMINGS=1 console apply` prints
what each stretch actually took, which is how the numbers are corrected.

It fills inside a stretch as well as between them, because the stretches that
matter are the ones long enough to doubt. Cargo names each crate as it starts
one, pacman names each package as it fetches and writes it, and the files and
the services are lists whose length is known before the loop begins -- so each
of those carries the fill a share of its own stretch and the strip moves while
the longest thing an apply does is happening, rather than at the end of it.

Nothing polls it. The engine writes `/run/console/updating` and signals the bar
when the number changes and only then -- so a stretch that is over in a
millisecond costs one wake-up, a build costs one per crate, and a desktop where
nothing is being applied costs none at all. The engine is root's and the bar is
hers, so a file under `/run` and a real-time signal are the only things that
cross between them.

It is the last rows of the bar's own surface, and it took two windows to draw
before that. waybar has no progress widget of any kind -- a custom module hands
over text, a tooltip and a class, and the class is all the stylesheet gets --
so the strip was a second bar the width of the screen, `bar-updating` sent
`at-0` through `at-100`, and `style.css` had a rule for each filling a gradient
to that mark. Two lists in two languages, and the shorter of them was the
ceiling on how many places the strip could be in: one rule per whole per cent,
on a screen where a per cent is ten points.

None of that survives the bar drawing itself. The room comes back from the
compositor, the fill is a panel that many points across, and the number is the
number: it is written in thousandths, which is about a point of fill and as
fine as the drawing can be. What the strip cannot do anything about is how
often it is told -- an apply reports once per crate it builds, so the fill
still steps where the report does.

The row is bought rather than taken: it is reserved whether an apply is running
or not. A strip that appeared only during one would shove every window on the
screen down and back again, and one that reserved nothing would sit on top of
the bar instead of under it. Idle, it is the same color as the bar above it, so
what it reads as is the bar being a row taller.

None of that is believed from the arithmetic. `440-the-strip-under-the-bar-fills`
puts a number in the file the strip reads, brings the nested desktop up with it
already there, and reads the row back off the screen: the fill color on the
left of where the number says, the bar's own ground on the right of it. Back
when the bar was waybar, a stylesheet naming a color no one defined did not
fail -- GTK drops the declaration and carries on -- so the file parsed, the
widget laid out, the bar exited 0 and the journal was empty while the strip
filled to nothing. That is how it shipped once, and asking the machine three
ways got three answers about the plumbing. The path is `CONSOLE_UPDATING_PATH` when something says so, which is
how a staged session is filled without writing into the laptop's own `/run`;
the engine runs as root outside anyone's session and is never told.

## The panel

`notifications-panel`, and it is the same card as the settings and the files:
[`docs/panels.md`](panels.md) is how it is built and what its buttons promise.
Two tabs, because a notification is in one of two states.

**Waiting** is what is on the screen now, a row each. A row opens onto the
whole of what it said -- who said it, the summary, and the body under them --
and that page is the only place the body can be read. A card is 320 by 140 and
the body is the half that does not fit: the summary names what broke and the
body says what happened to it. Under the notifications is **Clear all**,
which is what the bell's tap used to do on its own.

Both tabs are one reading, and so is the bell and so is the settings page.
`console-notify` writes what it is holding and what it has finished holding
into one file under the runtime directory, and everything that wants to know
reads that. So what the bar counts and what the panel lists cannot disagree,
and none of them forks a program to find out -- `makoctl list -j` was a process
per draw and another per tick, and `busctl monitor` showed the bell's own asking
as traffic to be woken by.

The file goes with the thing that wrote it. `ExecStopPost` takes it away when
the daemon stops, because a file no one is writing any more still reads as a
count, and a bell lit over a daemon that is gone is a reading and it is wrong.

Nothing is asked before clearing. What is cleared is in Earlier a moment
later, so it is a press that moves things rather than one that throws them
away, and a question about a press that can be walked back is a question
someone learns to answer without reading it.

**Earlier** is what the daemon has finished holding, and nothing else. It keeps
the last twenty,
dismissed and expired alike, and it is read rather than chosen: there is
nothing to do to a notification that has already gone. Both halves are drawn at
once, in the two columns a row that is only read is given, so the tab can be
gone down without opening anything.

There was no panel and no history for a long time, on the argument that what is
on the screen is what there is and the journal has the rest. The journal is not
a place anyone holding a handheld stands, which is the same argument the top
of this page makes about a fault that reached a stderr no one was reading.

## Quiet, without going deaf

The last row of Waiting keeps cards off the screen. It is `Quieten` on
`console.Notifications`, which flips the daemon and answers with the state it is now
in, so the row that pressed it is not left guessing at what it did.

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
    cat "$XDG_RUNTIME_DIR"/console/notifications.json
                                        what is waiting, what the Earlier tab
                                        is, and whether cards are held back
    busctl --user introspect org.freedesktop.Notifications \
        /org/freedesktop/Notifications  every call the daemon answers
