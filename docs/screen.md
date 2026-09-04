# The screen

How big it draws, how bright it is, and what it does when nobody is looking at
it. The first is on the **Screen** tab of the settings, with the brightness and
the evening switch; the rest of this is what the machine does on its own.

## How big everything is

The panel is 2560 by 1600 and the desktop is laid out at two and a half times
the density it is drawn at, so a window sees 1024 by 640. That number is the
size of everything -- the rows of a panel, the words in a browser, how much of a
folder fits at once -- and it was a constant in the compositor's file that
nothing on the machine could reach.

**Screen**, on the settings, offers three:

| | | |
| --- | --- | --- |
| Tiny | 1.0 | 2560 x 1600 |
| Smaller | 2.0 | 1280 x 800 |
| Normal | 2.5 | 1024 x 640 |
| Bigger | 3.2 | 800 x 500 |
| Huge | 4.0 | 640 x 400 |

Five plain words, two either side of the size this device is set up as, and no
sentence among them. What a rung costs is written here rather than in the row it
would have to be read out of: a list whose ends argue with themselves is a list
nobody reads to the bottom of.

These, because a density is not a free number here. The compositor lays the
desktop out in whole logical pixels and rounds off a scale that leaves a
fraction, so the size chosen would not be the size given. 2560 and 1600 share
320, and every scale that divides them both is 320 over a whole number. The ones
that are also a tidy number are 1.0, 1.25, 1.6, 2.0, 2.5, 3.2 and 4.0, each
about a quarter from the next -- far enough that changing rung is a change
somebody meant to make. **1.5 is not one of them**: it leaves 1706.67 pixels
across, and 1.6 is the nearest rung to it.

**Tiny** is the odd one and it is here on purpose. 1.0 is the panel at its own
pixels, on eight and a half inches, which is about a third the size everything
in this repository is drawn to be read and hit at -- the stylesheets here open
by saying that nothing on this device is smaller than it needs to be, and this
is smaller than that. It is offered anyway, because the machine this was written
on is not the only one that will ever run it.

**The live change is `hyprctl eval`, and this is the trap.** A Lua-configured
compositor answers the obvious command --

    hyprctl keyword monitor eDP-1,1600x2560@144,auto,2,transform,1

-- with `keyword can't work with non-legacy parsers. Use eval.` It is the same
shape as the `dpms` trap below: the command every example on the internet gives
comes back with a complaint nothing here would have seen, and the only symptom
is a setting that appears to do nothing. It has to be

    hyprctl eval 'hl.monitor({ output = "eDP-1",
        mode = "1600x2560@144", position = "auto",
        scale = 2.0, transform = 1 })'

-- a whole screen and not just a number, because a monitor described without its
transform is this panel turned back upright.

The answer is remembered in `~/.config/console/scale` and put back on at every
login, by `console-scale apply` in `session-start`. Not in the compositor's own
file: that file is this repository's byte for byte, and a machine that wrote its
own preference into it would be reported as drift for ever after. Same shape as
the evening switch, and for the same reason. `console-scale apply` cannot fail,
because the step after it in `session-start` is the whole desktop.

**The bar has to be told.** Its apply strip is a gradient with a hard stop in a
box, and a gradient's percentages are percentages of the box -- so the box has
to be the width of the screen, which is the one number in this repository that
depends on the density. `console-scale` writes it into
`~/.config/console/bar.css`, which the bar's stylesheet imports, and restarts
the bar onto it. GTK takes `@import` only at the top of a file, so that rule
cannot win on order and is written to outrank the stylesheet's own instead. The
number in the stylesheet is what the strip is at the size this device is set up
as, and `the_strip_is_as_wide_as_the_screen` still holds it against the
compositor's declaration.

**The home screen has to be stood back up.** It is not told the density -- the
grid takes whatever screen its surface is given -- but the surface it has is
the one it was mapped onto, and a density changed under a running layer surface
leaves it wearing the logical screen that was. `console-scale` restarts it in
the same transaction as the bar, so one press is one round of the desktop
coming back at the size that was asked for.

What it does with that screen is `console_home::shape`. A square used to be a
number of logical pixels, which is a number that is only right at one density:
turn the desktop down and the same square is a third of what it was as a share
of the screen, turn it up and the grid no longer fits under the bar. So the
pane is divided into cells, the picture is a share of the shorter side of a
cell, and everything else about the square -- the space inside the plate, its
corners, the gap to the next one, the size of the name -- is a fraction of the
picture, written into the stylesheet on every redraw. One number moves and the
whole square moves with it, which is what makes it the same square at every
rung of the ladder above.

And it is hers to argue with. **The home screen** on the Screen tab is three
rows under that ladder -- how many across, how many down, and the same five
words either side of what the room suggested -- written to
`~/.config/console/home-screen` and said down the home screen's own door, so a
press changes the grid under the panel that made it. Narrowing the grid folds
whatever was off it round onto the end of the pane, and onto a fresh pane where
that is what it takes: a press of minus is never a way to lose an application.

**A panel that is not this one** gets a ladder written for a screen it is not.
`the_offered_sizes_divide_the_panel_into_whole_pixels` reads the compositor's
own declaration and fails if any rung stops dividing it, so a fork that changes
the screen is told to change the ladder rather than finding out on the device.

Nothing else in this repository is told the density. The panels take fractions
of whatever screen they are given, and the on-screen keyboard reads the scale
off the output it is drawn on.

## When nobody is looking at it

Two things the compositor's own people wrote and this desktop only decides for:
the screen dims and goes out when nothing is happening, and its colour warms
through the evening on a clock. Neither is written here, and the reason is
worth saying once: both need to be told what idle is and what a colour
transform is by the compositor itself, and a version of either written in
this repository would be guessing at what Hyprland already knows.

| | |
| --- | --- |
| After two minutes | The screen dims, to the same floor the rocker will not go below |
| After five | It goes out |
| Anything at all | Both come back |
| Dusk, and again at dawn | The colour slides warm and back, on a clock |
| **Night colours**, on the Screen tab | Whether the clock gets to say at all |

## What counts as something happening

`hypridle` does not watch devices. The compositor tells it, and the compositor
only counts devices it has bound. Asked on this machine, `hyprctl devices`
lists `inputplumber-keyboard`, `inputplumber-mouse`, `stick-scroll`, the
touchscreen, the touchpad, and wvkbd's virtual keyboard. **There is no gamepad
in that list at all.**

That sounds like a device that dims in your hands and is not, and the reason is
where the keys come from. A button reaches the compositor twice over. The ones
the profile routes to keys arrive on `inputplumber-keyboard`, which is bound
and counted. And whatever a press comes to -- an arrow key, a click, a
scroll -- is sent by the controller daemon through a device of its own, which
is `stick-scroll` in that list and is counted too. What the compositor never
sees is the pad itself: the face buttons and the d-pad arrive on a gamepad it
has not bound, and the daemon's answer to them is what wakes the screen.

There is one place that matters. The keyboard profile maps nothing on purpose,
so that the on-screen keyboard can read the pad itself -- and while it is up,
moving the highlight from key to key is invisible to the compositor. Only a key
actually typed comes back, through wvkbd's virtual keyboard. So a person who
spends two minutes deciding what to type will watch the screen dim, and typing
anything brings it back. That is a small enough fault to write down rather than
build for.

## What it does not do

**It does not lock.** The only way to type on this machine is the on-screen
keyboard, which this desktop puts on a layer above whatever is up. A lock
screen takes the keyboard for itself and would sit above that in turn, so the
password could be asked for and not answered. A handheld nobody can unlock is
worse than one that was never locked.

**It does not suspend.** Nothing here has ever shown that this machine comes
back from one. InputPlumber rebuilds the pad on resume, and whether it comes
back is the kind of thing that gets found out by a device failing to wake in
somebody's hands. **Sleep** is on the System tab, where a person chooses it and
is there to see what happens. The battery a handheld spends with its screen off
is small; the trust it spends by not waking up is not.

Music is the case that decides those two. The screen going out while something
plays is right, and it happens: audio does not hold an idle inhibitor. A
machine that suspended on the same timer would stop the music, which is why the
timer that would have done it is not there.

## Where a change goes

The times are in `files/home/@user@/.config/hypr/hypridle.conf`. What each
listener runs is ours, and deliberately:

`console-brightness dim` and `undim` rather than `brightnessctl -s` and `-r`.
Putting a screen back means having remembered where it was, and this desktop
already keeps what full and floor mean on this panel in one place --
`console_settings::screen`, which is also what the rocker and the level on the
Screen tab read. The pair here adds one rule that a saved value in somebody
else's file could not: a screen that is no longer where the dimming left it is
a screen somebody has touched, and it is left where they put it. Otherwise the
press that woke the machine would also undo the change it was making.

The note of where it was lives in the runtime directory, not the home, so a
machine that lost power while dim wakes with nothing to restore rather than a
memory of a level from another day.

**`dpms` has to be written in Lua here, and this is the trap.** This machine's
compositor is configured in Lua, so the line every example of this file on the
internet gives --

    hyprctl dispatch dpms off

-- comes back `')' expected near 'off'` and does nothing. It has to be

    hyprctl dispatch 'hl.dsp.dpms({ action = "disable" })'

Nothing would have reported the first one. The daemon runs the command, the
command fails, and the only symptom is a screen that never goes off, which
reads as a feature that was never installed.

## The colour

`hyprsunset` hands the compositor a colour transform. That is why it is used
rather than a shader over the top: what it changes is not captured, so the
screenshot the top right paddle takes at eleven at night looks like the one
taken at noon.

The screen follows the clock. It cools nothing all day, slides from daylight
down to lamplight across the two hours of dusk, holds there through the night,
and climbs back over the half hour before morning. It used to be a switch and
one temperature, and that is a decision somebody has to remember to make twice
a day: the evening it is wanted is the evening nobody thinks of it.

The slide is what makes it invisible. A screen that changed colour in one step
at half past seven would be a thing that happened to you; this is a thing you
never catch happening. Its steps are spaced evenly in mireds rather than in
kelvin, because the same thousand degrees is an enormous change at the warm end
and barely visible at the cold one, so a curve stepped evenly in kelvin crawls
all evening and then lurches.

The whole curve is a file the daemon reads once, so nothing of ours has to be
awake to keep the screen honest at three in the morning. The file is written
out of `console_settings::warm` by `console-warm curve` rather than by hand,
and a test holds the two together: a curve written twice is a curve that goes
out of step, and out of step here is a screen that changes colour at a time
nothing in this repository mentions.

## Saying no to it

There is still a way to say no, and it had to change shape. `hyprsunset`
re-applies its profile at every step, so telling it `identity` is undone by the
clock -- three minutes later during dusk, and not until morning at midnight.
That is one switch behaving two ways depending on when it was pressed, and
there is no way to ask the daemon to stop following its own profiles.

So off means the daemon is not running. `console-warm` writes the answer down
and restarts the unit; the unit asks `console-warm wanted` in `ExecCondition=`
before it starts anything. A compositor with no colour transform on it is a
screen showing its own colours, which is the one state that is true whatever
the hour and survives a reboot without anybody re-asserting it.

A condition that says no leaves the unit inactive rather than failed, so
nothing restarts it and nothing is reported. `console-fell` had to be taught
that `exec-condition` is not a fault, or every boot on a machine where somebody
prefers their own colours would raise a card saying the screen daemon had
stopped on its own.

## Game Mode

Neither unit is running there. Both are `PartOf=console.target`, which Game
Mode stops behind the switch, so the screen over there is Steam's to dim and
nothing of ours is running to disagree with it. `console-idle` puts the
brightness back on the way out, so a machine that left for Game Mode while
dimmed does not arrive there at its floor.
