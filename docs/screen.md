# The screen

How big it draws, how bright it is, and what it does when no one is looking at
it. The first is on the **Screen** tab of the settings, with the brightness and
the evening switch; the rest of this is what the machine does on its own.

## Display Zoom

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
no one reads to the bottom of.

These, because a density is not a free number here. The compositor lays the
desktop out in whole logical pixels and rounds off a scale that leaves a
fraction, so the size chosen would not be the size given. 2560 and 1600 share
320, and every scale that divides them both is 320 over a whole number. The ones
that are also a tidy number are 1.0, 1.25, 1.6, 2.0, 2.5, 3.2 and 4.0, each
about a quarter from the next -- far enough that changing rung is a change
someone meant to make. **1.5 is not one of them**: it leaves 1706.67 pixels
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

The answer is remembered in `~/.config/console/screens/eDP-1/scale-wider` --
under the connector it is about and the shape that screen was standing in,
beside that screen's `turn` -- and put back on at every login, by `console-scale
apply` in `session-start`, which walks every screen the compositor has and puts
each one back to its own. A rung is a canvas divided into the panel's own width,
so the same word is a different density on every screen it is said about, and a
different density again either way up: **Normal** held landscape is 1024 points
across 2560 pixels and **Normal** stood on its end is 1024 across 1600, which is
everything on the screen a third larger. One word for the whole machine put the
handheld's quarter turn on the monitor plugged into it and the handheld's
landscape rung on its own portrait; a rung per screen per shape is neither.
Two shapes and not four quarters, because the half turn is the same width as
the quarter opposite it and a rung is only ever about the width. Not in the
compositor's own file: that file is this repository's byte for byte, and a
machine that wrote its own preference into it would be reported as drift for
ever after. Same shape as
the evening switch, and for the same reason. `console-scale apply` cannot fail,
because the step after it in `session-start` is the whole desktop.

**The bar is not told, and it used to have to be.** Its apply strip was a
gradient with a hard stop in a box, and a gradient's percentages are
percentages of the box -- so the box had to be the width of the screen, which
made it the one number in this repository that depended on the density.
`console-scale` wrote it into `~/.config/console/bar.css` at every login and
restarted the bar onto it. `console-bar` draws its own surface and asks the
compositor how wide that surface is, so the width is a fact it already has and
the file is gone. `console-scale` still restarts it, beside the home screen and
in the same transaction, so both come back onto the screen that is now there.

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

And it is hers to argue with. **Home Screen** on the Display tab is three
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

## Rotation

A row per quarter under the ladder -- **Turned left**, **Not turned**, **Turned
right**, **Turned over** -- and they are quarters either side of the way the
panel is mounted rather than degrees from nothing. This panel is 1600 by 2560
with a transform of 1 in the compositor's file: that quarter is what makes a
portrait panel a landscape desktop, and it is what **Not turned** means here. On
a panel mounted the other way the same words mean the same things, which is the
whole reason the mounting is read rather than written down. The half turn is
there because a panel stands four ways and not three: on a handheld it is the
way round that puts the sticks where a stand does not foul them, and on a panel
screwed in upside down it is the only way up that reads at all.

Nothing drawn is told. Every surface takes fractions of the screen it is given
and a rung is a canvas divided into the panel's own width -- so turning the
screen changes which of its two sides that width is, and the size has to go
with the turn. `console-scale` describes the screen whole, in one eval, and
restarts the bar and the home screen exactly as a change of size does. A turn
that sent the transform on its own would leave the desktop at a density no one
chose.

It is remembered in `~/.config/console/screens/{connector}/turn`, beside that
screen's scale and for the same reason, and `console-scale apply` wears both at
every login. A screen with nothing under its name wears what `console apply`
wrote out of its own mode, so a monitor plugged in for the first time stands
upright at its own density rather than wearing whatever the last screen was set
to.

**The touchscreen turns with it.** A touch panel reports in its own orientation
and the compositor reads it through a quarter of its own, which was a number in
the compositor's file: right for the one way up this device had ever stood, and
left behind the moment the screen could be turned. A desktop standing at a
quarter with its touches still read at the mounting is one where every press
lands a quarter away from the thumb that made it, which reads as panels that
ignore you and a bar that takes seconds to answer. So the touch device is part
of describing the screen -- `console apply` writes it into `monitor.lua` out of
the panel's own mode, and `console-scale` says it in the same `eval` as the
monitor, because the two are one answer.

Which way round left is cannot be settled without the device in someone's
hands: left is one quarter on from the mounting and right is three. If that
reads backwards in the hand, the two arms swap and nothing else moves.

## How bright it is, when the room decides

This panel has an ambient light sensor on the AMD sensor hub, and **Auto-Brightness**
on the Display tab is what it is for. Most machines that will ever run
this desktop have no such sensor, which is why the row is not drawn when there
is none and `console-light.service` answers its own `ExecCondition` with no
before it starts anything: a unit skipped that way is inactive rather than
failed, so nothing restarts it and nothing raises a card.

**Nothing here ships a curve.** A table of light against brightness written in
this repository would be one person's eyes, on one panel, in one room, and
every machine that ran it would inherit a preference nobody on it had
expressed. It is the same argument the monitor block and the size ladder
already make: what is one machine's is not written down here.

So it is taught. Every press of the brightness rocker is somebody saying how
bright they want the screen in the light they are sitting in, which is a
reading and a level and therefore a sample, and it costs them nothing to give
because they were pressing the rocker anyway. Before the first press it knows
nothing and does nothing. The first thing it ever does is something it was
told.

The table is a band per half-decade of the sensor's own number rather than a
fit: the light in a room runs over four decades from a dark bedroom to direct
sun, the eye reads it as a logarithm, and nobody presses the rocker often
enough to fit anything. A band taught twice keeps half of what it knew and half
of what it has just been told, so a press made for some other reason moves it
rather than replacing it. Between taught bands it interpolates, outside them it
holds the nearest, and it acts when the band changes rather than continuously
-- a screen that creeps every time a cloud goes over is worse than one that
never moves.

**Being wrong costs one press, and the press is the next sample.** That is what
makes it safe to leave on. There is no state it can reach that a person cannot
correct in the way they were already going to, and correcting it is how it
stops being wrong. It is also why there is no button to make it forget: two
presses carry a band most of the way back on their own.

The reading is the sensor's raw number and the scale beside it is not read.
Nothing here ever says a number of lux out loud; what the reading is for is
which band of light the room is in, which is a question about this reading
against the others this sensor has given, and the raw number answers it exactly
as well without putting a decimal in the middle of the one calculation that has
to give the same answer twice.

**The colour channels are not used, and it is not an oversight.** This sensor
offers colour temperature and chromaticity beside the illuminance, which is
what the evening warmth below would want -- a room's own colour instead of a
clock. On this machine every one of them reads zero for ever while the
illuminance tracks the room, and the kernel says why every few seconds:
`hid-sensor-hub: Event data for report 4 was too short`. The firmware sends a
longer record than the driver accepts and the driver drops it. So the warm
curve keeps its clock.

**It does not follow while the screen is dark.** The follower stands down as
soon as the dim has happened, which keeps it from arguing with `console-brightness
dim` and keeps it from waking the sensor hub for a panel nobody is looking at.
Whatever is learned is the rocker's, never the follower's own: a daemon that
both chose the brightness and recorded its choice as a preference would teach
itself its own answer until the table said one thing everywhere.

## When no one is looking at it

Two things the compositor's own people wrote and this desktop only decides for:
the screen dims and goes out when nothing is happening, and its color warms
through the evening on a clock. Neither is written here, and the reason is
worth saying once: both need to be told what idle is and what a color
transform is by the compositor itself, and a version of either written in
this repository would be guessing at what Hyprland already knows.

| | |
| --- | --- |
| After two minutes | The screen dims, to the same floor the rocker will not go below |
| After five | It goes out |
| After ten | The machine sleeps, unless something is holding a sleep lock |
| Anything at all | All three come back |
| Dusk, and again at dawn | The color slides warm and back, on a clock |
| **Night Shift**, on the Display tab | Whether the clock gets to say at all |

## What counts as something happening

`hypridle` does not watch devices. The compositor tells it, and the compositor
only counts devices it has bound. Asked on this machine, `hyprctl devices`
lists `inputplumber-keyboard`, `inputplumber-mouse`, `controller-desktop`, the
touchscreen, the touchpad, and wvkbd's virtual keyboard. **There is no gamepad
in that list at all.**

That sounds like a device that dims in your hands and is not, and the reason is
where the keys come from. A button reaches the compositor twice over. The ones
the profile routes to keys arrive on `inputplumber-keyboard`, which is bound
and counted. And whatever a press comes to -- an arrow key, a click, a scroll
-- is sent by the controller daemon through a device of its own, which is
`controller-desktop` in that list and is counted too. What the compositor never
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
password could be asked for and not answered. A handheld no one can unlock is
worse than one that was never locked.

**It used to not suspend, and that was the expensive decision here.** The
refusal was argued rather than measured: nothing had shown this machine comes
back from a sleep, InputPlumber rebuilds the pad on resume, and a device that
will not wake in somebody's hands spends more trust than a battery is worth.
Measured, it was false -- the device suspends and resumes over and over, and
`amd_pmc` reports S0i3 entered and left cleanly every time. What the refusal
cost is the difference between 2.78 W awake with the panel already out and
about a quarter of a watt asleep: every hour past the blank was about a
sixteenth of a full battery spent on a screen nobody was looking at.

So it sleeps, five minutes past the blank rather than at it. The screen going
out is a guess that nobody is there and it is wrong often; the gap is for the
person still holding it. Five rather than ten because the case a wrong suspend
used to ruin is now answered rather than waited out -- a playing song holds a
logind `sleep` lock of its own, which `console-awake` argues for, and the
suspend simply fails while it is held. **Sleep** is still on the System tab for
somebody who wants it now.

The lock is `sleep` and deliberately not `idle`. hypridle reads the idle
inhibitors once for the whole of its config, so a player that took one would
hold the panel lit as well -- and the panel is the expensive half.

## Where a change goes

The times are in `files/home/@user@/.config/console/hypr/hypridle.conf`. What each
listener runs is ours, and deliberately:

`console-brightness dim` and `undim` rather than `brightnessctl -s` and `-r`.
Putting a screen back means having remembered where it was, and this desktop
already keeps what full and floor mean on this panel in one place --
`console_settings::screen`, which is also what the rocker and the level on the
Display tab read. The pair here adds one rule that a saved value in someone
else's file could not: a screen that is no longer where the dimming left it is
a screen someone has touched, and it is left where they put it. Otherwise the
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

**Only one thing puts the panel back on.** `console-brightness undim` does it
before it does anything else, because a screen that has gone dark and stayed
dark is the thing there is otherwise no way out of from the device itself. The
blank listener used to send the same dispatch on its own resume, which looked
like belt and braces and was not: hypridle resumes every listener that timed
out at the same instant, so waking a machine that had been idle past five
minutes sent the compositor two identical enables at once. Both answered `ok`.
The compositor then read as on -- `dpmsStatus: 1`, the monitor not disabled --
with the panel still dark, and the only way out was a disable and an enable
sent from somewhere else entirely, which on a handheld means another machine
over ssh. The second dispatch is gone and a test holds it gone: every
`on-resume` in that file has to be the one that also knows how bright the
screen was.

## The color

`hyprsunset` hands the compositor a color transform. That is why it is used
rather than a shader over the top: what it changes is not captured, so the
screenshot the top right paddle takes at eleven at night looks like the one
taken at noon.

The screen follows the clock. It cools nothing all day, slides from daylight
down to lamplight across the two hours of dusk, holds there through the night,
and climbs back over the half hour before morning. It used to be a switch and
one temperature, and that is a decision someone has to remember to make twice
a day: the evening it is wanted is the evening no one thinks of it.

The slide is what makes it invisible. A screen that changed color in one step
at half past seven would be a thing that happened to you; this is a thing you
never catch happening. Its steps are spaced evenly in mireds rather than in
kelvin, because the same thousand degrees is an enormous change at the warm end
and barely visible at the cold one, so a curve stepped evenly in kelvin crawls
all evening and then lurches.

The whole curve is a file the daemon reads once, so nothing of ours has to be
awake to keep the screen honest at three in the morning. The file is written
out of `console_settings::warm` by `console-warm curve` rather than by hand,
and a test holds the two together: a curve written twice is a curve that goes
out of step, and out of step here is a screen that changes color at a time
nothing in this repository mentions.

## Saying no to it

There is still a way to say no, and it had to change shape. `hyprsunset`
re-applies its profile at every step, so telling it `identity` is undone by the
clock -- three minutes later during dusk, and not until morning at midnight.
That is one switch behaving two ways depending on when it was pressed, and
there is no way to ask the daemon to stop following its own profiles.

So off means the daemon is not running. `console-warm` writes the answer down
and restarts the unit; the unit asks `console-warm switched-on` in `ExecCondition=`
before it starts anything. A compositor with no color transform on it is a
screen showing its own colors, which is the one state that is true whatever
the hour and survives a reboot without anyone re-asserting it.

A condition that says no leaves the unit inactive rather than failed, so
nothing restarts it and nothing is reported. `console-report-crash` had to be
taught that `exec-condition` is not a fault, or every boot on a machine where
someone prefers their own colors would raise a card saying the screen daemon
had stopped on its own.

## Game Mode

Neither unit is running there. Both are `PartOf=console.target`, which Game
Mode stops behind the switch, so the screen over there is Steam's to dim and
nothing of ours is running to disagree with it. `console-idle` puts the
brightness back on the way out, so a machine that left for Game Mode while
dimmed does not arrive there at its floor.

**The screen a game gets is not the one above.** Gamescope selects the panel's
own `1600x2560@144` and patches the EDID it presents inward, so an Xwayland
under it stands at `2560x1600@144`: the turned screen at no scale at all, where
everything else here is a logical screen with a transform and a density on it.
Nothing of ours writes that number and nothing of ours could -- it is decided on
the other side of the switch.

It is written down because a game that guesses its resolution rather than asking
arrives at neither number, and the fault that makes is silent. The Sims 4 wrote
`1920x1200` into its own settings on a first run, drew its interface at that size
onto a surface that was not, and came up black with a menu in one corner --
rendering the whole time, with nothing in any log of Steam's or ours to say so.
What answered it was gamescope's own stats pipe, which names the application it
is presenting and the rate it is presenting at.
