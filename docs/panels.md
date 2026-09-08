# How a panel is built

Everything that comes up over this desktop is the same thing: a card, driven by
the front of the machine, that goes away again. The menu, the settings, the
guide and the files are all one card, and a person who has learnt one of them
has learnt all of them.

[`docs/console-ui.md`](console-ui.md) is what any surface here owes before it
is a panel at all. [`docs/button-contract.md`](button-contract.md) says what each
button promises.
This says how a surface is built so those promises can be kept, which is the
half that was only ever in the code.

The device is held like a handheld console and it is driven like one. The d-pad
picks, A accepts, B backs out, the shoulders move between places, and nothing
needs a pointer. That is the whole shape, and everything below is what it costs
to keep.

## The buttons, and what makes them true

| | |
| --- | --- |
| **D-pad** | moves the highlight, and does nothing else |
| **A** | takes the highlighted row |
| **B** | one step back out of wherever you are |
| **X** | the keyboard, up and down, everywhere |
| **Y** | what else can be done with the row you are standing on |
| **L1** and **R1** | the place before and the place after |
| **Right paddle, top** | closes whatever is up |

A means the highlighted row only for as long as a chooser is up. On the desktop
A is a mouse click where the pointer is, because there is no highlight out there
to confirm. That difference is a column in the controller daemon's table --
`When::WithAChooserUp` against `When::OnTheDesktop` -- and the daemon knows
which it is by asking the compositor whether a chooser is on the screen. A panel
does not ask for anything and cannot get it wrong; it draws, and it is seen.

It used to be two InputPlumber profiles swapped on the way in and out of every
menu, and every swap destroyed the pad and built another. What the swap bought
is what the column buys now: a list on this device can be walked with a thumb.
What it cost was the on-screen keyboard's device and the daemon's, several times
a minute.

A window that cannot be recognised as a chooser cannot be driven this way, which
is the whole of why the files are ours and not Dolphin's.

## Row nought is the way back

Wherever you are, the first row goes back one step: out of a folder, out of a
question, out of the thing you opened.

It is there because B has no answer for a finger. The panel's own way out is the
**×**, and that closes the whole card, so without row nought anything opened by
touch could only be left by putting the device down and picking up the
controller. Every button has to have an answer for a hand holding nothing, and
this is B's.

`Row::back` makes it, so it says the same thing and wears the same **‹**
wherever it is at the top of a list.

A page with a line to type in puts the line above it. Typing is what such a page
is for and the rows under it are what the typing is about, so the line goes
first and the way back is the first of the rows.

## A row that opens onto another list says so

`Row::opening` draws a **›** at the end of the row. It means what the strip's
does: there is more that way.

A list that goes deeper looks exactly like one that does not, and without the
mark the only way to find out was to press A and see where you ended up. The
Defaults tab is six rows that each open onto their own choices, and it reads as
six settings and their readings rather than as six lists somebody has to
remember are there. A folder in the files wears the same mark, and so does the
one row of Y's list that opens another one.

The mark is a label rather than something to press. The whole row is already the
way in, and a mark that could be tapped on its own would be a second, smaller
target for what the row does anyway.

## What a row keeps room for at its front

`Row::picturing` keeps a square at the front of the row: a photograph's own
thumbnail, or an icon out of the theme, or nothing. Which icon is a variant of
`console_panel::icons::Icon` rather than a name spelled at the call site, so
that the list of icons this desktop needs is one list a device-tier check can
hold against the theme the machine actually has. It found one that was not
there.

Asked of the whole list rather than of each row, so the names start in one
place. A folder wears the folder icon, which is symbolic and therefore drawn in
whatever ink the row is written in, so it stays in the palette on a highlighted
row as well as a dark one. Anything that has no picture worth making keeps the
room and puts nothing in it, because a page of documents each wearing a small
grey rectangle is harder to read than a page of names.

The picture itself comes out of one file. A row that opened its own was a file
opened, a format worked out and an image scaled -- on the loop that draws,
because that is where rows are built -- and the menu did that once per
application installed before its first frame. Measured, it was most of what
opening the menu cost, and none of the work was new: the same icons, at the same
size, every time. So they are decoded once into `console_panel::pictures`, at
the size a row draws them, and an opening reads that file once and hands out
slices of it.

`panel-pictures` makes it, off the panel and behind it, the way `files-thumbs`
makes the thumbnails and for the same reason. A panel asks for what its own rows
wanted once the real ones have arrived, so what is made is what a list actually
asked for; and a picture the store has not got is opened the old way, which is
slower and never wrong.

## B unwinds, one step at a time

B is not "close". It is "back", and back is a stack.

In the files that is: out of the question about a thing, out of the folder, out
of the folder above that, and only at the top of a place does B mean the panel.
A page says what back means for it with `Page::on_back`, and saying true is
saying there was nowhere left to go. Nothing else has to know.

Pressing B until you are out is a habit somebody can have without being taught
it, and it only works if every surface unwinds by the same rule.

## The shoulders are places, never actions

L1 and R1 move between tabs, and a tab is somewhere you are. Sound, Wi-Fi,
Pictures, Downloads.

Never a thing they do. A shoulder that submits a form on one panel and turns a
page on another is two buttons wearing one name, and the thumb that learnt the
first one is wrong on the second.

The way out is the last of those places, one press past the last tab. Standing
on it draws nothing and reads nothing: the tab that was in front is still in
front and still on the screen, it says so in mint rather than wearing the
highlight, and the **×** takes the highlight instead. A closes the card from
there, which is the shoulder still going somewhere and A still doing the thing.
The d-pad hands the panel back to the list, so nobody is left standing on a
button that closes it.

## Y is about the row, never about the selection

Y asks the highlighted row what else can be done with it. Not the tab, not the
selection, not whatever was last touched.

It is the one button the contract lends out, and it already means "more options"
on the desktop, where it is the right mouse button. A row with nothing more to
offer says nothing, which is why Y can mean the same thing everywhere and still
be silent over most of what it is pressed on.

A row may be about where you are standing rather than about a thing: the files'
way back is the folder, and Y over it asks for a new folder in it. That is still
the row answering. What Y must never become is a menu about the screen, offered
the same wherever the highlight happens to be, because then what it does is a
guess about what was last touched.

A card about one thing is the case that looks like the exception and is not. The
now-playing card is one song from the sleeve to the row of buttons, so every row
of it a thumb can stand on offers the same Y and opens the files panel on that
song. Every row of that card is about the same thing, so the row is still what
answered. What it must not do is put Y on the row a card is *titled* with and
nowhere else: a title is a heading, the highlight walks past it, and a button
offered only there is a button that cannot be pressed from anywhere.

The mark is a different question from the offer. One offer made by three rows
draws three marks down one card about one song, which reads as three different
things a finger could do; so the rows a thumb stands on carry Y with no mark of
their own, and the card draws it once, in the corner of its head, where a card
about one thing has always put what else there is. `Row::ended` with two empty
ends is how a row says it carries the offer and draws nothing for it, and
`one_mark_for_one_subject` is the rule held against the card that comes out.

## The name of a list is not one of its rows

`Row::naming` is the thing the rows under it are about: the file a question is
about, the folder a listing is of, the kind of thing a choice of programs is
for.

It is drawn as a title and not as a row — no card, smaller, quieter — and the
highlight walks past it, from either direction, so a list of six things that can
be done is six rows to a thumb. Written with `said` it was a row like any other,
the same shape a thumb aims at, and a question about a photograph read as though
the photograph were one of the answers to it.

## An empty list says so, and what it says is not a row

`Row::nothing` is the panel saying there is nothing here: no notification
waiting, no song in the folder, nothing that answers to the word typed, no
program that opens this kind of file.

Every tab has one, and every one of them used to be written with `said`, which
is a card the width of the panel in the ink an option is written in — the exact
shape a thumb is aiming at. So a tab with nothing on it read as a tab with one
thing on it, and the only way to find out otherwise was to press A and watch
nothing happen. It declared no intent and no behaviour while wearing the shape
of something that has both.

It is drawn as what it is instead: no card, quiet, small, set across the middle
rather than down the left where the names line up, with room above and below.
The highlight walks past it from either direction. Anything that can actually be
done about the emptiness — clear the folder, look again, add a picture — is
still a row of its own under it, shaped like a row, because that one is an
option and this one never was.

## Every button has an answer for a finger

The device is a touchscreen and it is put down as often as it is held. A button
whose job cannot be done by hand is a thing that can be looked at and not
changed, which is worse than a thing that is missing.

So: a tap on a row is A. Row nought is B. The **−** and **+** on a row are left
and right. The **‹** and **›** either side of the strip are the shoulders. The
**⋯** beside a row is Y. The **×** is the way out of the card.
`crates/console-manifest-engine` holds the bar to this, and the table in the
button contract is the list.

A row may ask to wear no marks at all, by naming two ends that are both empty.
It is what a picture asks for: the whole width is the picture's, a hand steps it
by pushing it aside, and a button hung on the far edge would be a thing floating
in the middle of what somebody is looking at. It takes the marks off and never
the deed -- Y still answers on a bare row, drawn on the row beside it -- which
is how a card with one subject ends up with one **⋯** rather than one per line
it is written on. That is where the two viewers this was read against put it:
Loupe behind a single button on its bar, the phone galleries behind a single
overflow at the end of the actions.

The **⋯** is drawn beside every other row that has something behind Y, the way
the ends of a level are drawn on every row that carries one, and it presses that
row's own offer rather than whatever the highlight is on. That difference is the
whole of why it is not simply Y wired to a button: a thumb asks about the row
it is standing on, and a finger asks about the row it has landed on, and those
are the same question only while nobody is using the screen.

It is the mark the panel drew last, and what it cost to be without it is the
measure of the rule. Renaming a file, deleting one, picking one up, what a
film's subtitles are, how fast it plays, opening it over the whole screen --
every one of those lived behind Y and behind nothing else, so a machine put
down on a table could look at all of them and change none of them.

## What else there is stands beside the row, not inside it

It was a mark at the end of the row, on the card the row is drawn on. Inside the
card it is one press drawn on top of another: the whole row is the thing A
takes, and the last inch of it is a different thing entirely, with nothing but a
change of ink to say so. A hand coming at a row from its right-hand side is
aiming at both of them at once, and what it gets is decided by an inch it was
never told about.

So the row is two shapes in a line now. The card is what the row says and what
the highlight fills; the **⋯** is a button of its own outside it, beside it and
its height, on the same ground. Each of them means one thing, and a hand aiming
at either is not aiming near the other. `#line` is the card and the row itself
is given nothing to be seen by, which is the whole of the change in the sheet:
every rule that dressed a row dresses the card on it.

Being its own thing rather than part of the row is also what lets the d-pad
reach it. Right off a row moves the highlight onto what else there is, left
comes back, A takes it and B comes back the way B always does. The row keeps the
ground it wears whenever the press being made is somewhere inside it -- the same
answer the scrub and the transport already give -- and the button takes the
pink, because the row and the thing beside it are exactly side by side and two
pinks touching is the panel asking a question it does not mean.

Y is untouched and is still the short way: from anywhere on the row, whatever
the highlight is standing on, without walking to it first.

A press that only moves the highlight changes nothing else a check could see,
so `console_panel::telling` says where the highlight is standing -- on the row,
beside it, or nowhere -- and the panel says so again whenever it moves rather
than only when it draws. `right_off_a_row_stands_on_what_else_it_offers` is that
pressed against the viewer's Media page, which is the page every row of which
offers something.

**Right is whatever the far end of the row already was.** A row that carries a
level has its **+** there and right sets it, which is what right has always
meant on it; a row that does not has the **⋯** there and right stands on it. The
marks a row wears are what right does to it, which is a thing a hand can see
rather than a rule it has to be taught, and `nudged` is the whole of the
decision -- no machine in it, so it can be asked twice and answered the same
way.

A row that carries both keeps left and right for the level and keeps Y and the
button for the offer. The offer never loses its answer for a finger, which is
the rule `every_offer_answered` holds every panel to; what it loses is the short
way onto it, on the one kind of row where the short way is already spent.

That is also what a card about one thing is left with. Its offer is drawn once,
on the head, and a head is a title the highlight walks past -- so there the
button is the finger's and Y is the thumb's, which is what it was before this.

## A swipe across a row is that row's level

Left and right on a row that carries a level is the one thing this desktop
offers that a finger could otherwise reach only by aiming at a mark the width of
a thumbnail. Every gallery anybody arriving here has already used steps to the
next picture by pushing the one in front of them out of the way, so a swipe is
wired to the row's own level closure -- the same one the **−** and **+** press
and the same one the d-pad calls. Nothing has to be taught it: a row that gains
a level gains the mark, the button and the hand together, and a panel cannot be
written that answers one and not the others.

The sign is the one every phone uses and it is the opposite of the arithmetic:
pushed to the left brings the *next* thing in from the right, because the hand
is moving the thing being looked at rather than the place in the list. A drag
that is mostly downward is the list being scrolled and is left alone, and a
thumb resting on a row has no speed in it and has asked for nothing.

Touch and the touchpad both, which is what Loupe does. Nothing is lost by taking
both -- a press and a release in the same place has no speed in it, so a tap is
never read as a push -- and it is the difference between a gesture a check can
make and one only a thumb can.

## Opened out, the way back comes off the strip

A picture opened over the whole screen has no tab strip, which is where the
**×** lives, and a card with no × is a card a finger cannot leave. So the mark
comes off the strip and lies on the picture, in the corner, and it keeps the
same hours as everything else the card draws: a page that has put its own rows
away -- a film being watched, a few seconds after the last press -- is a page
that wants the screen, and the next tap anywhere brings the rows and the mark
back together. That tap is spent doing only that, which is the rule the pad has
always been held to and the screen was not.

## A question is a surface, not a list

`Showing::sure` asks it: what is being asked, the thing it is about beside it,
and the answers on the line under. Left and right walk the answers, A takes the
one standing, B is no.

A list here is a list of things to go into, and a question written as rows is a
sentence in an inventory. It also cost the answer a row of its own: yes was a
row saying "Yes, delete" and no was row nought, which is the way back and not
an answer to anything.

Nothing is pushed. The question stands over the list it was asked on and either
answer leaves it, so there is no page to walk back out of and no state saying
which question a tab is in the middle of.

The answers are the caller's, so a question with more than two is the same
surface. Moving onto a name that already exists is three: replace it, keep both,
or leave it alone.

## A question that cannot be taken back opens on the answer that does nothing

Every other list opens with the highlight on the first row that does something,
because the first press of A should do the obvious thing.

A question is the exception, and it opens standing on no. It is the one place
where the obvious thing is a photograph thrown away by a thumb that pressed A
twice. The answers that do something are drawn after it and wear the warmer
colour.

## A card about one thing opens on the press it is for

A list opens on the first row something happens to, which is the right answer
for a list: the first thing on it is the first thing a thumb wants.

A card is not a list. Walking down the now-playing card, the first row anything
happens to is the bar the song is scrubbed with — one row above play, which is
the press a hand opened the card to make. So every opening of that tab began
with a press of down.

`Row::chief` is the row saying which press that is, the way `Press::chief` is
the press a strip is for. It is asked only while the highlight has not been put
anywhere yet: the playing tab is drawn again every second, and a card that went
back to its own press on every reading would take the highlight off whatever the
thumb had walked to a second after it got there.

A row nothing happens to cannot be the row a card opens on, whatever it says
about itself. The highlight would be standing where A does nothing, which is the
fault this rule exists to avoid rather than one to introduce by another door.

## A card about one thing is set in the middle of its room

A list starts at the top of the card and grows down it, because that is where
reading starts and because a row that lands late must not move the rows above
it.

A card is not a list, and the now-playing card is the case that shows it. It is
three rows -- the sleeve with the title beside it, the bar, the row of buttons
-- and the card it is drawn on is the same height as the one the menu fills,
because the card is one size whatever is on it. So the whole player sat against
the top edge with a third of the card empty under it, which reads as a surface
that has not finished loading rather than as one that is about a single song.

`Page::in_the_middle` says the page is a card, and its rows are set in the
middle of the room instead of from the top. It is a property of the page rather
than something worked out from how many rows there are: a tab that centred
itself whenever its rows happened to be short would move every row on it the
moment one more arrived, which is the fault the remembered rows exist to avoid.

It is the same answer a picture opened out already got, and now it is one
answer rather than two: `Opened::Out` centres what it draws because the screen
is the picture's, and a card centres because the card is about one thing.

## What is slow does not happen where the drawing happens

A panel that is waiting is a panel that has stopped answering the buttons, which
reads as a machine that has crashed rather than one that is working.

Rows are read on a thread of their own, so a folder on a stick over USB does not
freeze the card. Anything that writes goes to `Showing::later`, which runs it
off the main loop and draws again when it is done. Anything that takes a moment
to find out goes to `Page::on_arriving`, so the panel appears at once and fills
in.

And anything slow says so. `Showing::note` puts one line in the corner of the
screen for six seconds, over the card rather than in it, and takes it down on
its own. It is what goes with `later`: a press that hands its work away leaves
the panel looking exactly as it did, and a wallpaper that arrives a minute after
it was chosen is a press that appears to have done nothing twice. It says what
has been set going, so there is nothing to answer and nothing to dismiss.

Drawn by the panel rather than raised as a notification. Every one of these
surfaces is a layer over everything on the screen, so a notification raised from
a panel is drawn behind the panel that raised it.

## A tab that cannot be drawn in advance is drawn as it was last time

`Page::meanwhile` is the tab as it stands before the machine has answered
anything, and it works because most of a tab is known without asking: the three
power profiles are the same three whatever powerprofilesctl says, and all the
answer decides is which of them is marked.

Some tabs are not like that. Sound is whatever is plugged in and whatever is
playing, Wi-Fi is whatever is in the air, Bluetooth is whatever has ever been
paired, the menu is whatever is installed. There is nothing to draw in advance,
so those went up empty and filled in — which is the whole card changing height
a moment after it appeared, under a thumb already moving down it. Of the two,
that is the worse one: a reading that lands late moves nothing, and a row that
lands late moves every row under it.

They are not unknowable, though. They were known last time. `console_panel::before`
writes down what a command said as it answers, and the tab's `meanwhile` builds
its rows out of what was written down. The menu keeps its own list the same way,
under `console_applications::kept`.

Three things make it honest rather than a guess drawn as an answer:

**It is the same builder, fed an older reading.** Never a second list that has
to be kept in step with the first. `wifi_at` draws the tab from three readings
and does not know which of the two it is doing; a row added to it cannot go
missing from the other.

**The rows work.** This is the whole reason the reading is remembered and not
the row. A row built from a remembered reading carries the same `Does` as the
row that replaces it, so A on it does what it says while the machine is still
being read. A greyed-out list of last time's words would keep the card the right
height and be a page of things that cannot be pressed, which is the fault the
empty list at least declared.

**What goes stale is not remembered.** A sink is a fact about the machine that
keeps; a stream is something that was playing once, so the speakers row is drawn
from memory and what is playing is not. The list of applications keeps and the
count of how often each was opened is one file already, so the list is remembered
and the order it comes out in is read fresh.

It is a cache and it says so. `~/.cache/console`, and not beside the notes under
`~/.local/state/console` where a panel keeps the tab it was left on and the room
it was granted: those are things the desktop remembers about itself and could
not work out again, and this is the machine's own answer to a question anybody
can ask it again. Clearing it costs one opening, drawn the way every opening was
drawn before any of this.

## A tab that has to be looked for looks while it is the tab in front

A watch is a program a tab keeps open so it is told when its own answer changed
rather than asking on a clock: `pactl subscribe` under Sound, `busctl monitor`
under the bell. Every one of them redraws the tab it belongs to and is ignored
while any other tab is up, so a watch running behind a tab nobody is looking at
was a process spending a battery to be thrown away. They start when their tab
comes to the front and stop when it leaves, and the one that made that worth
doing is Bluetooth.

Looking for a device is not a reading. It is the radio doing something, it costs
power for as long as it lasts, and bluez holds what it finds only while it is
still looking: stop, and everything not already paired is dropped within
seconds. The tab used to offer a press that scanned for eight seconds, and then
drew the list *after* the scan had ended -- the leavings of a scan, which is a
column of nameless addresses that have not aged out yet and, most of the time,
nothing at all of the mouse somebody was holding down a button on. It asked for
the answer at the one moment it was guaranteed to be gone.

So the looking is the tab: `bluetoothctl scan on` is the tab's watch, and it
runs for exactly as long as somebody is standing on that tab. What comes out of
it is three things at once. The row where the press used to be says **Looking
for devices** while the radio says `Discovering: yes`, which is a tab that
answers a press instead of appearing to swallow it. Every line the scan prints
redraws the list, so a device arrives when it arrives and its name arrives when
the advertisement carrying it does, rather than both landing eight seconds later
if they land at all. And nothing is dropped underneath somebody halfway through
reading it, because the discovery is still on while they read.

A number does survive: `--timeout`, because bluetoothctl will not stay without
one. It is a ceiling and not a wait -- the scan is killed the moment the tab
stops being in front -- and ten minutes is longer than anybody stands on this
tab and short enough that a panel left open on it overnight is not a radio left
looking overnight.

## What a stranger says about itself is what puts it in order

A scan in a room is mostly watches, earbuds and phones, and bluez names a device
that has told it nothing after its own address. So the list was a column of hex
in whatever order bluez keeps its cache, with the one thing somebody wanted
somewhere in it.

Both of the answers to that come off the reading already being taken. **What
said a name goes above what did not**, because a device that has told the
machine what it is is a device somebody might be looking for, and an address
written out with dashes is bluez saying it has nothing to offer. And **what is
loud goes above what is faint**: `bluetoothctl info` carries an `RSSI` for
anything heard during
discovery, so the same reading that says paired and connected says how far away
it is, drawn as the bar the Wi-Fi tab already draws a signal with. A mouse in
the hand is the loudest thing in the room, and the room's own noise sits at the
bottom under a strength bar that says why.

The strength is only there while the radio is looking, which is the other half
of why the looking is the tab. A device with nothing heard from it draws the
same `…` it drew before, and is still a row that can be pressed.

## What an opening costs is written down as it happens

Every one of the decisions above was made about a wait nobody had measured. The
remembered rows, the reading on a thread of its own, the note in the corner:
each of them is somebody's account of which part was slow, and none of them
could be checked afterwards on the machine it was decided for.

So a panel times itself, and writes one line when it appears. From the press --
the daemon stamps the moment it decided, in the child's environment, so what is
timed is what the thumb waited and not what the program took -- through the
loader, the wait for whatever chooser had the screen, GTK coming up, the card
being built, the rows going on it, and the first frame. One line per opening,
with the stretches as its fields, so a question about the menu is a question
anybody can ask the file rather than a print somebody adds and takes out again.
`console_response_times` is the writing and `console-response-times` is the
reading.

It is on always. Timings that have to be asked for are timings nobody has when
they want them, because the opening worth reading about already happened.

The line is handed to a queue and written by a thread of the process's own, so
the stopwatch does not cost the thing it is timing: the frame that was about to
be drawn does not wait for a disk. A program whose whole run is one wait -- the
two session switches, the desktop starting -- calls `settled` before it exits,
because a queued line is in that process and nowhere else.

### `press`, and the lines that do not have one

`press` is the only stretch nothing in the timed process can see, and it is
there when whoever started the program stamped it. That is the daemon, which
holds a press for as long as a turn of its loop takes before it starts
anything, and a panel starting an application because somebody pressed a row.

An opening from the bar has no `press`, and that is not a stamp going missing.
Waybar forks on the touch, so the fork *is* the press: `exec` already holds the
whole of that wait and there is nothing before it to measure. The bar says so
rather than leaving it to be worked out -- its clicks carry `CONSOLE_FROM=bar`,
and every line records `from` under `with`.

Two rules keep the field honest, and both exist because it was wrong before
them. A stamp that is not there leaves the field out rather than writing a
zero, because zero is a measurement and means the machine answered instantly.
And a stamp goes stale: the environment is inherited, so a panel started by a
press hands that press to everything it ever starts, and a stamp read back
minutes later measured a wait that was over minutes ago. Anything past `STALE`
is not a press, and anything a panel starts that nobody pressed -- a picture
drawn in the background, a program asked a question, a watcher -- has both
marks taken off it.

### What is timed that is not a panel

Three waits are not a GTK surface and had to say so themselves.

The **keyboard** is cairo on a layer surface of its own, so nothing above
reaches it. It writes `keyboard`/`starting` once, with the palette, the
keymaps, the compositor and the typist as its stretches; `keyboard`/`showing`
every time it is asked onto the screen, from the signal to the first frame; and
`keyboard`/`language` around a layer change, which is where an xkb keymap is
compiled and the one place it is likely to be slow.

The **daemon** writes `controller`/`press`: the stretch between the pad saying
something and the daemon deciding to act on it, which is what says whether a
slow opening is the daemon or the toolkit. A turn happens twenty times a second
and almost all of them decide nothing and write nothing. One that starts a
program always writes; one that only scrolled writes if it took longer than a
frame.

The **session switch** is the longest wait on the machine. `console_session`
times the steps of going and coming back, but a switch usually does not get to
write its line -- the session going down takes the program writing it too. What
always lands is `session`/`starting`, measured in the session coming up, which
is the half somebody is sitting there watching.

Two things it says that reading the code does not. The first is that most of an
opening is the rows: they are built one at a time and each of them opens its own
picture before the first frame, so what a tab costs to appear is decided by how
many rows it has and how many of those wear a photograph or an icon rather than
a space. The second is that the same work is done twice on every opening -- once
for the tab as it was last time, and again when the reading lands -- which is
the price of the card being up at once, and worth knowing the size of.

The store is `~/.local/state/console/waited.jsonl`, beside the tab a panel was
left on, and not under `~/.cache` with the readings a tab draws itself from:
those are the machine's own answers to questions anybody can ask it again, and
this is the only record that the menu was slow on Tuesday.

It is kept. The rotation is at ten gigabytes, which on this device is a stop
against a program stuck in a loop rather than a retention policy: the question
these lines exist to answer is whether the machine is getting slower, and a
store that holds a week cannot be asked it. So nothing is thinned to make room
-- the music panel's line per refresh stays where it is -- and the reading is
what changed instead. `console-response-times` walks the file a line at a time
and holds a window of the last few thousand; `--all` is for whoever wants the
year and has the memory for it.

## Most of what an opening cost was the machine being asleep

The first thing the store said, once there was enough of it to read, was that
no part of an opening was slow. Every part of it was: the loader, GTK coming
up, the card being built, the rows going on it, the first frame — all of them
over by the same factor, which is not what a slow function looks like. It is
what a slow processor looks like.

This is a handheld and it idles at the bottom of its range, and a processor
that decides how fast to run by watching how busy it has been is always
deciding about the moment before. A panel is a moment's work and then nothing,
so the whole of it is answered at whatever clock the machine happened to be at
when the thumb arrived, and by the time the load it made could have been
noticed there is nothing left to hurry.

So the daemon that reads the pad asks for speed as it starts a panel, before
the fork rather than from inside the program it started — most of an opening is
spent before the panel has a line of its own running, and a program that asks
on its own behalf has already missed the part it would have helped most. It
puts back what was there a moment later. `console_haste` is that, and it is one
word per processor: there is no per-task version of it on this machine, so what
is raised is the machine's own hint and it has to be given back or the profile
somebody chose stops meaning anything.

None of it is a reason not to do the other work. The rows are still built one
at a time, the tab is still drawn twice, and both are still worth what they
cost. It is a reason to measure a change against a machine that is awake, since
otherwise half of what any change appears to buy is the clock.

## And the rest of it was starting a program at all

The store went on saying the same thing after the clock was dealt with, and by
then it was saying it about a machine that was awake: on every surface the two
largest stretches are `exec` and the toolkit coming up. Neither is work about
the panel. They are what it costs to begin from nothing, and this desktop began
from nothing fifteen times an evening because a panel was a process.

So the panels are held by one program. `console-panels` opens the display once,
parses the stylesheet once, reads the icon theme once, and draws whichever
panel it is asked for; `crates/console-panels` is that program and its head
is the argument. What each panel crate hands it is a `Card` — a closure that
builds the pages, and the shutting-down its `main` used to do after the loop
ended — and what decides whether one may open at all is a `Door`, which is a
name and a rule about opening it twice.

**The panel's own program did not go away, and that is the load-bearing part.**
The one-chooser lock is a `flock` held open for as long as a process lives, and
the kernel drops it however that process ends — which is what lets a chooser
that was killed outright leave nothing behind for the next one to trip on. A
host that outlived every panel would be a host that never let go. So `launcher`
is still exec'd by the bar, the paddle, the compositor's key and the menu's own
`.desktop` file; it still takes the lock, and it now stands there with nothing
to draw until the host says the surface is gone. Everything that reads the
screen — the daemon deciding a chooser is up, the bar lighting an icon, a check
asking `hyprctl` what is on which layer — sees exactly what it saw before,
under the same namespace.

**A panel with no host is merely slower.** This is a daemon and it can be down,
and nothing may be written as though it cannot: a socket that refuses is a
panel that draws itself in its own process, the way every panel did, and says
so on the journal rather than on the screen. It is the rule
`console-events` already keeps, for the same reason.

**Nothing is kept between openings.** The window is destroyed and built again
rather than hidden and shown, and the card is built by the same call the old
`main` made. That gives back a little of what the change bought — the layer
surface and the first frame are still per-opening — and it is worth it: a
surface that survived would be a surface holding a reading nobody refreshed,
which `docs/programs.md` names as the one real hazard in a program that holds
state. The two caches the menu kept in `static`s are one opening's now for
exactly that reason. `330-a-panel-opened-again-is-drawn-again` presses it on
the device, and presses it three times rather than twice: it is the third
opening, the one over a panel that was drawn in between, that would come back
built out of somebody else's leavings.

**A closed panel has to cost nothing.** This is what a resident process owes a
handheld, and it is not automatic. A GTK loop with no surface mapped has no
frame clock to tick and sits in `poll`, so being warm costs memory rather than
battery — but only if what the panel held while it was up is let go when it
closes. A watch is a `busctl monitor` or a `pactl subscribe`, an actor is a
thread, and both used to be bounded by the process ending. `shut` releases them
by name now, and `340-a-closed-panel-is-holding-nothing` is the check, on the
device, because a child left behind is a child on the device.

## One card, one size

`console_panel::shape` is the only place that says how big any of this is, as a
share of the room rather than a number of points.

Three surfaces used to be three widths and three heights, so opening one after
another moved the edges of the screen about and read as three programs rather
than one desktop. The tab strip is what the shoulders act on, and it was never
twice in the same place.

## What is kept honest, and what is not

`crates/console-input-controller/tests/what_reaches_the_desktop.rs` presses
buttons against the one table that decides what they do, on the desktop and with
a chooser up. Change what A does and it fails. It reads the table the daemon
itself reads rather than a copy of it, which is why it does not go stale.
`crates/console-input-gamepad/tests/the_button_contract.rs` keeps the part that
is about the files: every profile the switcher names exists, and every one of
them publishes all three devices.

`crates/console-test-checks` opens each surface in a nested desktop and asks
whether anything was drawn. That is what catches a panel that raises a window
and then fails on its first screenful, which no unit test can see.

Everything else on this page is a decision somebody has to keep by hand. Row
nought, the unwinding, what the shoulders are for and where a dangerous question
opens are conventions, not checks. They are written here so that the next panel
is built to them on purpose rather than by copying whichever one was nearest.
