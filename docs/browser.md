# The pad, in a page

Everywhere else on this desktop the d-pad moves between things and A takes the
one it is standing on. A page was the exception. The stick pushed a pointer at
a link and A clicked wherever the pointer had got to, so the browser was the
one window on the machine where getting somewhere was aiming rather than
choosing, and the smaller the link the longer it took.

`crates/console-web` is the add-on that keeps the promise inside a page.
[`docs/button-contract.md`](button-contract.md) is what the buttons mean;
this is what they come to once a page has the screen.

## Nothing here asks for a button of its own

Every button on the front of this machine already has a job, and the rule that
one button means one thing is the reason a person can learn this device once.
So the add-on was written inside what a page is already sent, which is four
things:

| | |
| --- | --- |
| **Y** | Arrives as a right click. The guide calls it *more options*, and on a page it labels every single thing that can be pressed |
| **D-pad** | Arrives as the arrows. It walks between those things, and types a label while the labels are up |
| **A** | Arrives as a left click. It takes what is being stood on, and where nothing is, it is a click, as it always was |
| **B** | Arrives as Escape. It puts the labels away, then a card, then the highlight, and having nothing left of ours to put away, goes back a page |

The last of those is a promise this desktop was making and not keeping. B goes
back everywhere on this machine, and in a browser Escape has never gone back:
it stopped the page loading and left you where you were.

## A label is written in arrows

Vimium writes its labels in letters, and a letter here costs the on-screen
keyboard, which is the slowest thing on this device. The d-pad sends four
arrows, so a label is a short run of them, drawn on the thing it belongs to and
pressed as it is written: **↑ →** is up then right.

The labels are prefix-free, which is worth a sentence because it is the whole
of why this is quick. No label is the beginning of another one, so a press
either finishes a label or narrows the list, never both, and nothing has to be
confirmed. The short ones are handed out first: on a page with five links,
three of them are one press, and it takes a page with more than sixty things on
it before any label is longer than three.

### Where one goes

A label has to say two things at once -- what to press, and what it belongs to
-- and the second is the harder one on a page that is mostly links. Each one
takes the top-left corner of its own thing and bites into it, far enough to be
plainly attached and not so far as to cover what is written there: a label that
hides the word *Home* has answered the first question by destroying the answer
to the second. A thing too thin to be bitten into -- a link in the middle of a
sentence -- has its label hung just above instead, touching the top of the
word, because a label sitting on a word is a word nobody can read, and what is
above a line of prose is usually more prose rather than another thing to press.

Where that corner is already taken, the label moves on to the next corner of
the same thing that is free, and a thing wide enough to hold several -- a nav
bar, a row -- offers the places along its own top edge. Two labels drawn in the
same place are two labels nobody can read, which on a page of small links used
to be most of them.

## What is along the bottom

The labels are for the page. The bar along the bottom is for the browser
around it -- the tab strip, the address bar and the browser's own menus are all
chrome, and chrome is a pointer and a small target on a screen held at arm's
length. It carries the same kind of label, drawn from the same pool, so there
is one thing to press and not two:

    Look for something      Find on this page       The tabs
    The address bar         The browser's menu      A new tab
    Close this tab          The tab that was closed
    Load it again           The top of the page

Pressed a second time, **Y** asks the other question: *open in a new tab*.
That is the second thing a link can do, on the button whose job is what else
can be done with the thing in front of you, rather than on a modifier nobody
would find.

It used to be the same screen with a different line of text along the bottom,
which is a mode nobody sees: the eyes are on the labels, and the labels had not
changed. So the second press draws a different page. Only links are labelled,
because opening in a new tab is the one thing only a link can do and a label on
a button would have been a label that lies -- the press would have been the
ordinary press. The labels are drawn in sky rather than pink, the bar keeps the
one line saying what this is, and the deeds go away, because *close this tab*
is not something that opens in a new tab. Fewer things also means shorter
labels, so the second press is quicker than the first.

## A finger still gets the browser's own menu

Y is a right click, and taking a right click away would take the browser's menu
-- copy a link, save a picture -- off the machine altogether. It is not taken.
A press of Y is the pad, which is a mouse; a finger held on the page is a
touch, and the event says which it was. So the pad gets the labels and a finger
held down gets the menu Firefox draws, and neither is a mode anybody has to
know they are in.

Every deed along the bottom can also simply be tapped, and every card draws a
**×**. A button with no answer for a hand holding nothing is the thing
[`docs/button-contract.md`](button-contract.md) is most insistent about.

So do the bars themselves, which for a while were the exception. Both of them
said which arrows to press and offered the glass nothing: the labels could be
put away with B or by taking a deed, and not by simply deciding against them,
and the bar the find draws counted its matches and let only the d-pad walk
between them. Each now carries a **×** at its right end, in the same place the
card's is, and the find's carries **↑** and **↓** beside the count. The rest of
a bar is read rather than aimed at, so those are the only things on one sized
for a thumb.

## The card is the same card

*Look for something* and *The tabs* draw a card, and it is deliberately the
card the menu and the panels draw: a line to type into at the top, rows under
it, the one your thumb is on in pink and what is already true in mint. Somebody
who has used the menu has used this. It is drawn in a shadow root out of the
browser's own palette file, so a site's stylesheet cannot dress it and it
cannot break the site.

The keyboard comes up with it. A card with a line to type into is a card
somebody came to type into, so it is raised with the card rather than waited
for: the surface used to arrive having made every decision except the one it
was drawn for, and the press of X afterwards was a press that only ever had one
answer. X still works, and now what it does is put the keyboard away.

The same rule reaches a field on the page. Taking one with A focuses it and
raises the keyboard, because that is the same decision made about somebody
else's form.

## Where a question goes is the browser's own answer

The rows on the search card are the engines the browser has, with the one it
uses first at the top. There is no list of engines in the add-on. The settings
panel's **Web** tab writes a policy that tells the browser which engine to
default to -- [`crates/console-defaults`](../crates/console-defaults) is that
-- and this reads back whatever that came to. A list here would be the same
choice made twice, and wrong the first day somebody changed it.

Under the engines are two more places to look: **on this site**, which is the
same engine asked about this host alone, and **on this page**, which is the
browser's own find. What is found stays on the screen with the count beside it
and the d-pad steps through the matches, because a count nobody can walk to is
not an answer.

There is no way to type an address into this card, and that is on purpose: a
card that both searches and navigates is a card that has to guess which one was
meant. The menu is where a question is typed on this device -- a line that
narrows to nothing is a question, and it opens in the browser -- and the
browser's own address bar is a row on the bar along the bottom.

## A new tab is a question being asked

The new tab is a page of the add-on's own, and it opens with the search card up,
the keyboard up, and the line already holding the keys. There was nothing else it could
usefully be: a new tab on a handheld is somebody about to ask something, and
the alternative was a blank page with an address bar at the top of it that
takes a pointer to reach.

It is the home page as well, and that is not the same claim. A browser started
with nothing to restore does not open a new tab, it opens the home page, so a
machine switched on in the morning got LibreWolf's own page and not this one --
the one moment a person is most likely to be about to ask something. The
manifest claims both. A browser normally asks about that on first start, and is
told not to by `extensions.installedDistroAddon.web@console` in `user.js`: that
question is worth asking about an add-on that arrived from a store, and this one
arrived by the same apply that wrote the file asking.

## The browser around the page

The bar along the bottom offers the address bar and the browser's own menu, and
for a long time it could not. Those are chrome, and an add-on written in the
ordinary way cannot touch chrome at all -- it can ask the browser to open a tab,
and it cannot put the focus in the address bar of the window it is running in.

`crates/console-web/web/around.js` is how it does now. An experiment API runs
in the parent process with the browser's own privileges rather than an add-on's,
and it is allowed here for exactly the reason the add-on is unsigned here: a
build without `MOZ_REQUIRE_SIGNING` is one where `EXPERIMENTS_ENABLED` follows a
pref, and `user.js` sets it. Release Firefox refuses both in the same breath.

Three functions, and deliberately only three. Everything that can be done from
inside a page is done from inside a page, where a mistake is a broken label
rather than a broken browser; this holds what nothing else can reach. *The
address bar* takes what is in it, so the keyboard types over the address rather
than into the end of it, and *the browser's menu* is a list of rows the arrows
already walk.

The third is not the browser at all. The on-screen keyboard is a program of
this desktop's, raised by a signal, and no page and no ordinary add-on may send
one -- which is the same test the other two pass. `keyboard-show` is what it
runs, and it is deliberately not `keyboard-toggle`: a program asking on
somebody's behalf is asking for a keyboard, not for the other side of a switch,
and a card opening while the keyboard happened to be up would otherwise have
taken it away from the person it was drawn for. Nothing waits on the answer. A
keyboard that did not come up leaves a card that still draws, still takes a
row, and still has the pad.

The pref has to be true before an add-on carrying an experiment is installed. A
browser that reads such a manifest with it switched off does not disable the
experiment, it refuses the whole add-on -- and a page would lose its labels
along with everything else. `console apply` writes `user.js` and packs the
add-on in that order, so one restart has both.

## How it gets onto the machine

`console apply` packs it and then tells the browser about it, in that order.

1. `console-web` reads the profile's own `palette.css`, packs it with the files
   from `crates/console-web/web/` into `/usr/local/lib/console/console-web.xpi`,
   and writes a note beside it saying what was packed and as what version.
2. `console-engine` writes the browser's policy, which installs the add-on from
   that file.

It does nothing at all when nothing has changed, which is nearly every apply. A
browser takes an add-on again when the version in it goes up, so a version
raised for its own sake would be the browser reinstalling something nobody had
touched every time the machine was told to catch up. The note beside the file
is what makes that answerable without unpacking anything.

The archive is written by hand, in `pack.rs`, and nothing in it is compressed.
An add-on is a zip; a stored zip is a legal one and is what a browser reads
either way, and it is a hundred lines rather than a compressor pulled onto a
handheld to save forty kilobytes on a file read once at startup.

Nobody has signed it and nobody is going to. LibreWolf is built to install an
add-on that nobody has signed, and the profile says so in `user.js`; release
Firefox is not, and would refuse this every start without saying so anywhere
anybody is looking. That is why the policy offers it to LibreWolf alone, which
`the_add_on_this_desktop_wrote_is_offered_to_the_browser_that_can_take_it`
holds it to.

## What it does not reach

Worth knowing before somebody meets one of these with a thumb and thinks it is
broken.

A page inside a page. The add-on runs in the top frame only, so a comment box
or a video player that a site has put in a frame of its own has no labels on
it. The pointer still reaches all of it.

The browser's own pages. Nothing runs on `about:` pages, on the add-on store,
or on the handful of sites Mozilla keeps content scripts off. The new tab is
ours and works; `about:config` is not and does not.

The chrome was the third of these and is not any more --
[the section above](#the-browser-around-the-page) is what became of it -- but
the labels themselves still stop at the edge of the page. The address bar and
the menu are reached by name, from a row along the bottom, rather than by a
label drawn on the thing itself. Everything else up there still wants a finger.
