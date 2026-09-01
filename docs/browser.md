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

## What is along the bottom

The labels are for the page. The bar along the bottom is for the browser
around it -- the tab strip, the address bar and the browser's own menus are all
chrome, and chrome is a pointer and a small target on a screen held at arm's
length. It carries the same kind of label, drawn from the same pool, so there
is one thing to press and not two:

    Look for something      Find on this page       The tabs
    A new tab               Close this tab          The tab that was closed
    Load it again           The top of the page

Pressed a second time, **Y** says the labels again and says at the top what
taking one will do now: *open in a new tab*. That is the second thing a link
can do, on the button whose job is what else can be done with the thing in
front of you, rather than on a modifier nobody would find.

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

## The card is the same card

*Look for something* and *The tabs* draw a card, and it is deliberately the
card the menu and the panels draw: a line to type into at the top, rows under
it, the one your thumb is on in pink and what is already true in mint. Somebody
who has used the menu has used this. It is drawn in a shadow root out of the
browser's own palette file, so a site's stylesheet cannot dress it and it
cannot break the site.

X raises the keyboard over it, as it does everywhere.

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

There is no way to type an address here, and that is on purpose. The menu is
where a question is typed on this device -- a line that narrows to nothing is a
question, and it opens in the browser -- so the address bar has an answer
already, and it is one press of the left paddle away.

## A new tab is a question being asked

The new tab is a page of the add-on's own, and it opens with the search card up
and the line already holding the keys. There was nothing else it could
usefully be: a new tab on a handheld is somebody about to ask something, and
the alternative was a blank page with an address bar at the top of it that
takes a pointer to reach.

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

The chrome. Nothing here can move the browser's focus into the address bar or
open its menus: an add-on is not allowed to, and the way to those is a finger.
Everything the bar along the bottom offers is there because it is something a
page can ask the browser for and a thumb otherwise could not reach.
