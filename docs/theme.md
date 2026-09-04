# Blossom

The desktop is pink, dark, and measured.

## One place

Every colour is decided in `theme/palette.toml`. Nothing else on the machine
decides one.

Almost nothing on the machine holds one either. A stylesheet, a Lua table, a
TOML file, an ini file, a shell script and a browser cannot share a variable
with each other, but most of them can import a file written in their own
language. So `console-theme` writes one small palette file per language,
and everything else imports whichever speaks its own:

| The palette, written | Imported by |
| --- | --- |
| `~/.config/console/palette.css` | waybar, the panels, GTK 3, GTK 4, libadwaita, Breeze |
| `~/.config/console/palette.toml` | alacritty |
| `~/.librewolf/console/chrome/palette.css` | `userChrome.css`, `userContent.css` |
| `/usr/local/lib/console/palette.sh` | the keyboard, which reads it on the way in and hands the colours to itself as arguments |

The stylesheets say `@pink` and `@text`. The keyboard says `"$pink"`. None of
them holds a hex, so none of them can fall behind.

The names libadwaita asks for and the longer list Breeze asks for are defined
in `palette.css` as references rather than as colours, so a role changing its
shade changes every name that stands for it.

Four cannot import anything and are written into between a pair of markers:

- `kdeglobals`, because KDE's ini format has no include. It is also the odd one
  out in another way: Qt wants three decimal numbers rather than a hex.
- `user.js`, because a browser preferences file is a list of literals.
- `hyprland.lua`. Lua could import it, and the compositor is the one place
  where a file failing to load costs the whole session rather than one window,
  so its two border colours are written rather than read.
- `console-paper.service`, because a systemd unit is a list of literals too. Its
  one colour is the ground the wallpaper daemon fills the screen with before
  `console-sky` has chosen a picture.

The placeholder icon is drawn. So is the wallpaper, which has a section of its
own below because it cannot be read back the way the rest can.

## Where to change it

`theme/palette.toml`, then `just theme`, and `just garden` if you moved
anything the picture is painted with. Nothing else.

Two tests stand behind that. One refuses a checkout where a generated file no
longer matches the palette. The other reads every file under `files/` and
refuses any colour, in any of the five ways a colour is written down here,
that the palette does not declare. A hex typed in by hand is invisible until
somebody looks at the screen in the right light, and by then it has been there
for months.

## How the colours are chosen

They are not chosen. A colour is declared as a hue and how much of it, together
with what it has to be readable against, and the lightness is computed: the
softest shade of that hue that still clears the ratio it was given.

That is why the theme can be pastel and still legible. Pastel and readable pull
the same way on a dark ground, so asking for the palest colour that clears 7:1
gets a colour that is both, and gets it again on its own if the ground behind
it ever moves.

## What is promised

Everything that is read clears **7:1**, which is AAA at any size. Every pairing
is in `theme/report.md` with the ratio it actually reached, rewritten whenever
the palette is.

The exceptions are named:

- A border is looked at rather than read, so `edge` clears the **3:1** a border
  needs.
- `ash` is what a terminal means when it asks for black. It clears **4.5:1**
  and no more, because black at 7:1 would be lighter than half the palette and
  would stop meaning black.

Nothing else is under AAA, including text Qt considers disabled and text a menu
has greyed out. Fading an entry until it cannot be read is a convention, and on
a device somebody is meant to be able to use it is a bad one, so Qt's dimming
is turned off rather than tuned.

The two scripts that print to a terminal ask for colours by number rather than
by shade, so they answer to the same palette. Neither uses the dim attribute,
which halves whatever colour it lands on: half of a colour picked to clear 7:1
is a colour that does not.

## Where the numbers come from

Contrast is WCAG 2.1 relative luminance, measured after the colour has been
quantised to eight bits a channel. That is what a checker reads off a screen,
and it sits about a tenth of a point away from the same arithmetic done on the
unrounded values: the difference between a palette that measures 7.02:1 and one
that measures 6.92:1 to anybody who tests it.

`crates/console-colour` is the arithmetic, in Oklch. It is the same arithmetic
as `Codincod.Design.Oklch` in the Codincod repository, which was written first
and for a different purpose, and the two were checked against each other:
colours and ratios agree to four decimal places. Those cases are vectors in
`crates/console-theme/tests/the_desktop.rs`, so this implementation cannot drift
away from the other one without a test saying so.


## The garden

The wallpaper is a cherry blossom garden with a path through it, a tree close
and a tree far. It rests, and then every seven minutes the wind comes through
and takes the blossom with it.

It is an animated WebP, and that is the whole reason it can be moving at all on
a machine running off a battery. A WebP frame declares how long it lasts, and
`awww`'s daemon sleeps in `poll()` for exactly that long. The first frame
declares seven minutes, so for seven minutes out of every seven and a bit
nothing on this machine is running: no timer, no wake-up, no compositor frame,
no GPU. The wind is the last few dozen frames, and each of those redraws only
the band of the picture the petals cross, which is what keeps a moving
wallpaper down to the size of a photograph.

`awww` is here and hyprpaper is not, for that reason alone. hyprpaper paints
one still image. mpvpaper would play a video, and a video decodes at its frame
rate whether anything in it is moving or not.

The one thing to know about `awww` before changing the picture: it keeps every
decoded frame in a cache file under `~/.cache/awww`, named after the picture's
path, its size, and how it was fitted to the screen. Nothing in that name comes
from what is inside the file. Redraw the garden, install it at the same path,
and `awww` plays the old picture's frames over the new picture's still: the
screen fills with rectangles of the two mixed together, worst where they differ
most, which is the band the wind redraws.

Nothing empties that cache wholesale. Those frames are what a picture costs to
put up: with them a wallpaper arrives in the moment it is asked for, and without
them the client decodes and compresses the whole loop first, which was measured
on the device at twenty-five seconds of a core. Emptied at every start, that was
paid at every boot, at every return from Game Mode, and every time a window
stopped covering the screen. What is thrown away instead is the entries older
than the picture they are entries for: `console_sky::place::freshen` does it by
their date, before that picture goes up, and `sky-press` throws the cache away
when it writes one. `console apply` throws it away when it writes a background,
which covers the garden: that one is painted by hand rather than by
`console-sky`, so nothing else holds its date against what the daemon kept. It is
an entry in `units::WAKES`. The same apply restarts the service when the picture
changes, which is why `named_by` looks at a unit's arguments and not only at the
program it runs.

So when the background looks wrong on the device, the first question is not
whether it was drawn wrong. Three steps, in this order, and the answer is
usually in the second:

1. `awww query` says which file it thinks it is showing. A path you did not
   expect ends it there.
2. `ls -l ~/.cache/awww/*/` against the picture's own mtime. A cache older than
   the picture it caches is the whole fault, and nothing about the drawing is in
   question: the next pass that puts that picture up throws it away, and `rm` on
   the entry is the same thing sooner.
3. `grim` to a file, and look at the band the wind redraws, which is the top
   half. Two pictures mixed together shows there first, because that is where
   consecutive frames differ.

One caution about the second rung, because `150-the-wallpaper` now says it in
its own failure message and somebody will read it as a verdict. An mtime stands
in for the thing actually wanted, which is that the cache was decoded from these
bytes, and the two can disagree: a picture restored from a backup or copied with
its times kept is new in content and old on paper. Older than the picture is
strong evidence and not proof. It is still the rung to reach for first, because
it is right nearly every time and it costs one `stat`.

`crates/console-garden` draws it. There is not one colour written down in it:
`[garden.paint]` in `theme/palette.toml` says which palette colour every part
of the scene is painted with and how much of it reaches the picture, and the
tool holds only the shapes. An alpha lives in the palette because a wash at a
tenth is a decision about colour; the shape of a tree is not.

Both trees throw a shadow, and neither shadow is a shape anybody drew. The tree
is drawn a second time through a transform that tips it away from the light and
flattens it into the ground, from the same seed, so a shadow cannot be of a
different tree than the one standing in it. There is no sun in this picture,
only what is left of the day in the air behind the hills, and a light that broad
carries no outline as far as the ground. Drawn at full size the branches came
out as scratches on a field. So the shadow is drawn at an eighth of the picture
and stretched back up, and the stretching is the blur, which cairo has not got.
It is laid down three times, each reaching a little further and fainter than the
last, which is why it is darkest where a trunk meets the ground and gone by its
far end.

Where that light is gets said in three places, and they do not all say the same
thing. The bark is a gradient across each tree, lighter to the right, which is a
direction and not a position. The glow is an ellipse centred at 0.54 of the
width, which is a position, and it is the only one in the picture. `THROW` is
the direction a shadow leans, and it follows the bark.

For the near tree, which stands well to the left of the glow, the three agree.
For the far tree they do not. It stands to the right of the glow, so a shadow
cast by the glow would fall the other way, and it leans left anyway. That is
deliberate. The glow sits low and broad and reads as light left in the sky
rather than as a sun at a place, and two shadows leaning the same way is a
stronger thing to look at than two shadows leaning correctly. At the size this
picture is looked at, the correctness is invisible and the consistency is not.

It is written down because nothing is going to change. Move the glow, or turn
the bark gradient round, and the other two will still be here saying what they
said, and the shadow that comes out pointing the wrong way will be months from
whoever moved it.

A picture cannot be searched for a hex that should not be in it, so it is held
to the palette from both ends. `theme/garden.stamp` records what the drawing
was made from, and a test refuses a checkout where the palette has moved and
the picture has not. The same stamp records what the picture came out as, which
is what the device-side check compares against the screen, because nothing on
the device can take a VP8 bitstream apart.

The picture is drawn at 2560x1600, which is the panel turned the quarter the
compositor turns it, so nothing ever resamples it. A test reads the mode and
the transform out of `hyprland.lua` and refuses a picture that is not that
shape. That test is there because it was wrong once: the wallpaper was drawn
the shape of the panel rather than the shape of the desktop, the daemon cropped
it to fit, and because what it held was a gradient there was nothing on screen
to say so.