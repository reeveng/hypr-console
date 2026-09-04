# The wallpapers

The picture on the screen changes with the hour, the weather and the time of
year. `crates/console-sky` is all of it: the press that makes a picture, and the
daemon that decides which one is up.

`theme/sky.toml` is the whole of what a person edits.

## What is on the screen

    console-sky              keep the right picture up
    console-sky --now        put the right one up and stop
    sky-press               press what the table names and is not here yet
    sky-press --again       press all of them
    sky-press --dropped     press what is in Pictures/Wallpapers
    sky-press --take PATH   press this, wherever it came from

Settings has a **Wallpaper** tab. It turns following the weather off, picks one
picture and leaves it, and takes up whatever is in `~/Pictures/Wallpapers`.

## Choosing

A picture may name four things, and anything it does not name it answers all
of.

| | from | what it can be |
| --- | --- | --- |
| `sky` | the sun's height here | `night` `dawn` `sunrise` `day` `sunset` `dusk` |
| `weather` | open-meteo | `clear` `cloud` `fog` `rain` `snow` `storm` |
| `season` | where the sun is on the ecliptic | `spring` `summer` `autumn` `winter` |
| `moon` | the clock | `new` `waxing` `full` `waning` |

The most particular picture wins: the one naming the most things that are true.
A picture for a full-moon winter night beats one for any winter night, which
beats one for any night, which beats one naming nothing at all. So the set is
grown by adding a picture and never by editing the ones already there, and a
picture that names nothing is what is up when nothing else fits.

Pictures that name the same number of true things take turns. The turn is the
clock cut into two hour lengths, so one of them holds for a couple of hours and
the next of the same standing takes it from there: two pictures for a clear day
are a morning and an afternoon, and the pair of them comes round again tomorrow.

Nothing is remembered between one turn and the next. The turn is the clock
divided by two hours and nothing else, so the daemon, `console-sky --now` and a
test all say the same thing about the same moment without any of them keeping a
place, and a machine that was turned off for an hour comes back to the picture
the hour asks for rather than to the one it was showing. The two hours are
counted from the epoch rather than from midnight, so where they fall in the
local day depends on the offset, and the daemon looks again every five minutes,
so a picture changes over within five minutes of its turn ending.

Which of them goes first is the order they are written down in, which is
arbitrary and is arbitrary somewhere a person can see it and reorder it.

Taking turns is only ever between equals, which is what keeps it from undoing
the rule above: a picture that names the weather as well as the hour is up for
the whole of the weather it names, however many pictures for that hour alone are
waiting behind it.

Three of the four are arithmetic and cannot fail. The weather is the one thing
that needs a network, so it is the one thing that can be missing, and a picture
that names a weather is not chosen while it is: guessing would put a snowy
picture up in a heatwave on the one day the network was down.

`sun.rs` works the sun's height out from the place and the moment, which is why
dawn is half past eight in December and five in the morning in June without a
table of times anywhere. The place is not written down anywhere: `here.rs` takes
it from the timezone the clock is already keeping, which the timezone database
already describes the position of. So a machine set up in one country and
carried to another follows its new sun without anybody editing anything, and no
address is stored to be carried anywhere else. The zone's own city can be a few hundred
kilometres from the person holding the machine, which moves the bounds of the
day by minutes and never moves the season at all. The seasons come from the same arithmetic rather than
from the calendar, which gets two things for free: the bounds are the solstices
and the equinoxes themselves, and the southern hemisphere gets its own seasons
rather than the north's with the wrong names on.

## Looping

A source loop is a video: nine to thirty-six seconds, twenty-four to sixty
frames a second, every frame a whole picture. The picture on the screen is the
whole of that, played over and over, at twelve frames a second rather than the
artist's. Twelve is a third or less of every rate she works at, and these are
hand-drawn loops of water, snowfall and firelight, none of which has an edge
sharp enough to judder at twelve.

It loops rather than stopping because a loop that stops is stranger to look at
than one that does not. What used to pay for stopping is now had another way,
by the picture not moving at all while anything is in front of it, and that is
the section below.

**Size.** Every frame at the size of this screen is twelve megabytes before it
is compressed, so the first frame is the whole picture and every frame after it
is only the rectangle that differs from the frame before, painted over what is
already there. The frames neither blend nor dispose, and a loop wraps back onto
that first whole frame, so the wrap repairs the picture exactly however many
rectangles have been laid over it.

How much that is worth depends entirely on the picture, and the spread across
the set is wide enough to matter. What decides it is not how many frames a loop
has but how much of each frame moves. A campfire is a lamp in the
corner of a picture that is otherwise still, and the rectangle that changes is a
small one: it costs the daemon almost nothing over its resting size. Rippling
water, falling snow and a candle-lit room are a new painting every frame, the
rectangle is the whole screen, and the daemon can be carrying the better part of
a gigabyte.

`sky-press` says which is which as it presses, picture by picture, so what any
one of them costs is a press away rather than a number written down here to go
stale.

Two things make the dear ones affordable. It is given back in full the moment
the still goes up, measured: swapping a dear picture for a cheap one took the
daemon straight back to its resting size. And the still goes up whenever
anything is in front of the picture, which on a handheld is nearly always.

Frame rate is the lever if a picture ever needs to cost less, and it is a linear
one: dropping a share of the frames takes that share off and no more.
`rest_seconds` is the other lever and a much larger one, and `theme/sky.toml`
says what it does. Neither is used by anything the machine ships with.

The device presses at 2560x1600, which is the panel through the quarter turn the
compositor gives it, so nothing is ever resampled. `console-screen` reads that
out of `hyprland.lua` rather than anybody writing it down twice.

Pressing the whole set takes minutes and one core, and grows with the set, which
is why it happens at `console apply` and never on the machine while it is in
use. A picture already pressed is left alone, so an apply that changes nothing
about the wallpapers costs nothing; `sky-press --again` is what presses them all
over.

## Not moving where nobody can see it

This is the half that pays for the other half. A moving picture behind a window
costs exactly what one nobody is behind costs, so the movement is put away
whenever anything is over it: the daemon is handed the still instead, which is
one frame that lasts for ever, and a daemon holding one frame is a process
asleep in `poll()` rather than one drawing.

Put away rather than paused, because the wallpaper daemon has no pause. It
plays what it was given, so what it is given is the thing that changes.

Which is also why it is not put away at once. A daemon handed a file starts it
at the first frame, and the picture rests for most of its loop and stirs for a
few seconds of it, so a menu opened and closed during the stir threw the stir
away and started the rest over: the movement did not carry on from where it had
got to. Nothing here can make it carry on, so nothing here interrupts it for
something that is about to go away again. The movement is put away only once
the wallpaper has been covered for fifteen seconds, which is longer than a menu
is up and much shorter than a window is open.

Two things can be over it, and both count.

A window. One window per workspace and nothing floats, so the wallpaper is
covered exactly when the workspace being looked at holds one, and that is a
number the compositor already keeps.

A menu, a panel, the guide or the on-screen keyboard. None of those is a
window; they are layer surfaces, and the compositor lists them separately. What
is asked of them is not which they are but whether they are there at all,
because the answer for every one of them is the same. So `covered.rs` names
what is allowed to be **behind** rather than what is allowed in front, and that
list is two entries long: the wallpaper daemon's own surface, which is the
wallpaper, and the bar, which is up for as long as the machine is on and would
otherwise mean the picture never moved at all. A panel written next year is
counted the day it is written, without anybody remembering to add it.

`journalctl --user -u console-sky -f` and open the settings: it should say so.

## Pressing a picture on the Wallpaper tab

The tab writes `~/.config/console/sky.toml` and nothing else, which is a file
being written and is instant. Then it asks `console-sky --now` for one pass of
what the daemon does every five minutes anyway, because five minutes after
choosing a wallpaper is not choosing a wallpaper.

That pass is the slow half, and it used to be waited for where the panel is
drawn: the settings answered no button between the press and the picture, which
reads as a machine that has crashed rather than one doing what it was asked.
Three things stand between the two now.

The pass goes to `Showing::later`, so it runs off the drawing and the tab is
drawn again when it is over. The corner says what was set going, because a panel
that looks exactly as it did is a row somebody presses a second time. And
`--now` asks the weather only where the answer could turn on it: a picture
somebody pinned is that picture in any weather, which `choose::pinned` answers
without a network, and that takes curl's eight second timeout out of the
commonest press this tab has.

What is left is the picture itself. The still goes up first and is one frame, so
the screen holds the right picture in the moment; the loop over it is decoded
whole before any of it is drawn, and on a picture the daemon has not seen since
it was pressed that is the part that can take half a minute.

Taking up what is in `~/Pictures/Wallpapers` is the other slow press, and it is
slow in the same way for a different reason: each picture is decoded, graded,
cut to this screen and written out again, which is tens of seconds apiece. It
has always run off the drawing. What is new is that the corner says so and says
how long, which is the difference between a press that is working and a press
that did nothing.

## Bringing a picture into the palette

The artist never heard of this machine, so the pictures arrive in their own
colours: a river in bright greens, a campfire in olive and brown. The bar sits
over them in pink on plum, and a picture sharing no colour with the thing
standing on it reads as two pictures.

What is done about it is not a filter chosen by eye. The palette already holds a
ramp from its darkest ground to its lightest ink, and that ramp has a hue,
because this whole theme is plum. So a pixel is asked how light it is, the ramp
is asked what colour the theme is at that lightness, and the two are mixed.

    keep      how much of the artist's own colour survives
    pull      how much of the theme's colour is laid over it
    floor     where the picture's black lands. Zero is a real black
    ceiling   where its white lands, as a share of the lightest ink

`ceiling` is the one that matters most and the one worth being brave with. The
two daylight snow scenes came out as pale fields that the bar could not be read
against, and it took a ceiling of about a half to make them pictures somebody
could put a panel on top of.

The mixing happens in Oklab's a and b rather than in hue and chroma. Hue is an
angle, and the average of two angles is a question with two answers; the average
of two points on a plane is one point. A green pulled halfway to plum through
the plane passes through grey, which is what fading a colour out looks like.
Pulled through the angle it would pass through orange, which is what a different
picture looks like.

It travels to ffmpeg as a cube, because the grade is arithmetic in a perceptual
space and ffmpeg's own filters are arithmetic in sRGB. Working it out here for a
lattice and letting ffmpeg interpolate means the picture is graded by this
repository's rules at ffmpeg's speed.

Try one before writing it down:

    sky-press --try SOURCE 0.35,0.70,0.0,0.68 /tmp/look-at-this.webp

A green daylight picture takes the pull worst, because a bright green scene in a
dark plum theme is a contradiction and pulling it hard turns it grey. Those keep
more of themselves and give up more of their brightness instead.

## Where the pictures come from

Not from this repository. They are somebody else's work, they are twenty
megabytes each, and this repository is source: `theme/sky.toml` holds an address
and the checksum the source had when it was written down, and the device presses
them the way it compiles the programs.

The checksum is not there to catch a bad download, which curl already refuses.
It is there because these are fetched from a site that mirrors somebody else's
work, and a picture quietly becoming a different picture is worse than one
failing to arrive. A mismatch stops that one picture, says both sums, and every
other picture is pressed as usual.

Every picture the machine ships with is by **Abi Toads**, who gives them away:
<https://abitoads.com/pages/animated-wallpapers>, through Wallpaper Engine on
Steam. The settings names them beside each picture.

Added pictures go to `~/.local/share/console/sky`, which an update cannot
replace, and are looked in before the set the machine came with.

## What is behind them

Not the garden. `console-paper.service` brings the wallpaper daemon up and fills
the screen with `night`, the deepest ground, and `console-sky` paints a picture
over that once it has chosen one. So the ground is a colour: it is what a
machine with no pressed pictures shows, what stays up if `console-sky` will not
start at all, and what is on the screen for the fraction of a second before the
first picture arrives.

The colour is written into the unit by `just theme`, because a systemd unit is
a list of literals and can import nothing. It is the fourth file to be written
into that way and `docs/theme.md` names the other three.

The cherry blossom garden used to be that ground, and being the ground was the
whole of what it did once the pictures arrived: a hand-drawn scene nobody
chose, in front of everybody, for a moment at every boot.
`crates/console-garden` still draws it and `just garden` still runs, the
picture still ships, and
`awww img /usr/share/backgrounds/console.webp` still paints it. It is the one
wallpaper here that is ours rather than somebody else's, and `console-sky`
presses every picture with its webp muxer. It is simply not what is behind
anything any more.

## When it is wrong

`docs/theme.md` has the three rungs for a wallpaper that looks wrong, and the
first one is still the answer most of the time: awww names a cache entry after a
picture's path and after nothing inside the file, so a picture pressed again at
the same path is served out of the old one's frames. `sky-press` throws that
cache away after it writes anything, which is the moment before it would matter.

Past that:

1. `awww query` says which file it thinks it is showing. The Wallpaper tab
   reads the same line, which is why it reports what is up rather than working
   out what ought to be.
2. `journalctl --user -u console-sky -f`, and open and close a window. A daemon
   that says nothing when a window opens is not reading the compositor's socket.
3. `console-sky --now` in a terminal, which does one pass and prints what went
   wrong rather than restarting around it.
