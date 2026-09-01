# The screen, when nobody is looking at it

Two things the compositor's own people wrote and this desktop only decides for:
the screen dims and goes out when nothing is happening, and it can be made warm
for the evening. Neither is written here, and the reason is worth saying once:
both need to be told what idle is and what a colour transform is by the
compositor itself, and a version of either written in this repository would be
guessing at what Hyprland already knows.

| | |
| --- | --- |
| After two minutes | The screen dims, to the same floor the rocker will not go below |
| After five | It goes out |
| Anything at all | Both come back |
| **Warm colours**, on the Battery tab | 3400K instead of daylight, and it is remembered |

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
Battery tab read. The pair here adds one rule that a saved value in somebody
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

It runs whether or not anybody wants warm colours, because with no arguments it
sits at 6000K, which is the neutral it would be without it. So the switch is a
message to something already listening rather than a daemon started and stopped
under a thumb, and the switch answers in one press.

The daemon forgets when it stops, so `console-warm again` is run by the unit
once it is up, and says what was decided last night. Without that the switch
would be a setting that lasts until the next restart -- the kind people stop
trusting and then stop using.

What warm is, is one number in `console_settings::warm`, for the same reason
full is one number in `screen`. 3400K is a lamp rather than daylight: far
enough from the daemon's own 6000 to be a switch with two visible sides, and
not so far that the wallpapers go orange.

## Game Mode

Neither unit is running there. Both are `PartOf=console.target`, which Game
Mode stops behind the switch, so the screen over there is Steam's to dim and
nothing of ours is running to disagree with it. `console-idle` puts the
brightness back on the way out, so a machine that left for Game Mode while
dimmed does not arrive there at its floor.
