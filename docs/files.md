# The files

**Files** in the menu, or `files-panel`. A tab is a place, a row is a thing in
it, and the four buttons mean here what they mean everywhere else.

| | |
| --- | --- |
| **D-pad** | up and down the folder |
| **A** | a folder goes into it, a file opens |
| **B** | back to the folder above, and out of the panel at the top of a place |
| **Y** | what else can be done with the thing you are on |
| **L1** and **R1** | the next place, and the one before |

[`docs/panels.md`](panels.md) is the grammar all of that belongs to, and it is
worth reading before changing anything here.

## Why it is not Dolphin

Dolphin is on the machine and a person holding the device cannot use it, for a
reason that is nothing to do with Dolphin.

The desktop's A is a mouse click where the pointer is, and its d-pad is the
arrow keys. So the row a thumb moved to and the thing A acts on are two
different objects, and a list walked with the d-pad is confirmed by clicking
whatever the left stick happened to leave the pointer over. Every panel here
avoids that by taking the chooser's buttons for as long as it is up, which is
what makes A mean the highlighted row, and a program that is not ours cannot
ask for them.

The rest follows from the same place. B is Escape, which in Dolphin clears the
selection; going up a folder is Alt and Up, which no button sends. Focus travels
between the places panel, the toolbar, the breadcrumb, the view and then a
modal dialogue, and only Tab moves between those, and there is no Tab. Nothing
in that list is a bug. It is a program built for a pointer, being held by
somebody who has none.

Dolphin stays installed for the day something needs a real file manager.

## Where the tabs come from

Home first, then the folders a home directory keeps, then anything plugged in.

Where each place is is the machine's answer rather than a name written down
here, because a home directory records where its folders are and on a machine
set up in another language they are called something else. That record only
names the folders something has once had a reason to write down, which on this
device is three of the six, so a place it says nothing about is looked for under
its own plain name before it is given up on. A folder that is not there is not a
tab: a tab that can only ever say it is empty is one the shoulders have to be
pressed past.

Anything plugged in is a tab after those, kept to what can be unmounted. That is
what a stick and a card are and what the disk this is running from is not,
and without it the strip carries a tab for the boot partition.

## What it stands on

`gio`, which the GTK every panel here is drawn with already links. Reading a
folder, copying, moving, trashing, watching for a change, noticing a stick and
knowing what opens a file are all its, so none of it is written here.

KIO was the other candidate, because it is what Dolphin itself does this with.
It is a C++ Qt library with no C interface, so reaching it from Rust means a
shim and a Qt event loop inside a GTK process; `kioclient` is the only thing it
offers a program that is not Qt, and that is a command per operation with no
progress and no promise about what it prints between one KDE release and the
next. GIO is already here and already linked.

## What a folder is read as

Folders first, then files, each by name without regard to case. Folders first
because walking is what this is for. Case ignored because a folder called Photos
and one called photos at opposite ends of a list is the alphabet of a machine
rather than of a person.

Dotfiles are not shown. A home directory has more of them in it than things
anybody put there, and shown, the first screen of Home is configuration nobody
opened this to look at.

Nothing is written beside a folder. How many things are in one is another read
of another directory for every folder on the screen, and on a stick over USB
that is the listing arriving in its own time rather than at once.

A folder wears the folder icon at the front of its row and the **›** at the
end, which is the mark every list on this desktop uses for a row that opens onto
another one. A photograph and a film wear themselves once the picture has been
made, and everything else keeps the room and puts nothing in it, so the names
line up. The room is kept for a whole listing or for none of it.

## What the line finds

The line at the top of a folder is not a filter on the rows behind it. A word
typed there is looked for in this folder and in everything under it, and each
row that comes back says which folder it was found in. Files called notes.txt in
several places are a list nobody can choose from, and Holiday beside one of them
is the whole answer. Something found right here says what the listing would have
said about it instead.

Nearest first, a folder at a time. What is being looked for is usually near
where the typing started, so the folders alongside are read before the ones
below them, and what the list holds when the search stops is the near part of
what there was.

It does stop. A home directory holds more than anybody is going to read down,
and a folder that links to the one above it is a walk with no end, so the search
gives up on finding and on looking, at whichever comes first.

A found file opens the way any other row does. A found folder is walked into a
step at a time, so B out of one arrives at the folder above it rather than back
where the search began. The line empties on the way in, because the word was
about the list it narrowed and not about the one that replaces it.

## What Y offers

Open, Open with, Rename, Copy, Move, Delete. A folder gets all of those except
the two about opening one, because a folder is walked into.

Not in the alphabet, which is the rule for lists of names and the wrong one
here. Open is first because it is what most presses of Y are on the way to, and
Delete is last because it cannot be taken back and the last row is the hardest
one to reach by accident. The question it asks opens standing on the way out,
so a thumb that presses A twice has said no.

Copy and Move pick a thing up. What is held shows as a row at the top of every
folder until it is put down, so carrying something is a thing you can see rather
than a mode you are in. It is one thing for the whole panel and not one per tab,
because carrying a photograph from Pictures to a stick is the reason it exists
and those are two tabs.

Rename and New folder are typed, and the keyboard is X like everywhere else.
What is typed is shown: `ask` hides it, because the only thing anything here had
to type before was a network's password, and a filename typed blind on an
on-screen keyboard is a guess about whether the last key registered.

A name with a slash in it is refused. It would make the name a path, so a rename
could put a thing somewhere else entirely, and `..` would make it disappear.

## What writes, and what reads

Reading is GIO: the listing, what kind of thing a file is, what can open it, and
which sticks are plugged in.

Writing is `mv`, `cp`, `mkdir` and `gio trash`, handed to `Showing::later`,
which runs them off the main loop and draws the folder again when they are done.
Copying a film off a stick takes seconds and the panel would answer nothing for
all of them. They are commands for the same reason the settings ask `pactl`
about the volume: the panel is a thing that draws, and `mv` already knows what
moving across two disks means.

Delete is `gio trash` rather than a removal, so it is in the wastebasket and not
off the disk. Nothing on this device shows that wastebasket, so a delete is
still asked about first.
