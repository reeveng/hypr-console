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

A means the highlighted row only because a panel takes the chooser's buttons for
as long as it is up. On the desktop A is a mouse click where the pointer is,
because there is no highlight out there to confirm. `console_panel::panel::show`
asks for the `tabs` profile before it draws and gives the desktop's back as it
goes, and that switch is the single reason a list on this device can be walked
with a thumb. A program that cannot ask for it cannot be driven this way, which
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
thumbnail, or an icon named out of the theme, or nothing.

Asked of the whole list rather than of each row, so the names start in one
place. A folder wears the folder icon, which is symbolic and therefore drawn in
whatever ink the row is written in, so it stays in the palette on a highlighted
row as well as a dark one. Anything that has no picture worth making keeps the
room and puts nothing in it, because a page of documents each wearing a small
grey rectangle is harder to read than a page of names.

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
**×** is the way out of the card. `crates/console-manifest` holds the bar to
this, and the table in the button contract is the list.

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

## One card, one size

`console_panel::shape` is the only place that says how big any of this is, as a
share of the room rather than a number of points.

Three surfaces used to be three widths and three heights, so opening one after
another moved the edges of the screen about and read as three programs rather
than one desktop. The tab strip is what the shoulders act on, and it was never
twice in the same place.

## What is kept honest, and what is not

`crates/console-pad/tests/the_button_contract.rs` reads the four profiles and
holds the button table to them. Change what A does in a profile and it fails.
It reads the profiles rather than a copy of them, which is why it does not go
stale.

`crates/console-checks` opens each surface in a nested desktop and asks whether
anything was drawn. That is what catches a panel that raises a window and then
fails on its first screenful, which no unit test can see.

Everything else on this page is a decision somebody has to keep by hand. Row
nought, the unwinding, what the shoulders are for and where a dangerous question
opens are conventions, not checks. They are written here so that the next panel
is built to them on purpose rather than by copying whichever one was nearest.
