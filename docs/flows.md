# Flows

A check asks one question about one feature. A flow walks the desktop the way
a person does -- across programs, across crates, across minutes -- and asks
at every step what that person would see. The checks say each promise holds
on its own; a flow says they hold in a row, because most of what goes wrong
on this machine goes wrong *between* features: a mode that lingered, a
chooser that stacked, a panel that kept a button it should have handed back.
No check about the viewer and no check about the files panel can catch the
press that falls between them.

[`checks.md`](checks.md) is the doctrine and it is not repeated here: assert
what a person would see, wait for the thing and not for a number of seconds,
a green check can be a lie, it is somebody's machine. Everything below stands
on that.

## The promises

What somebody holding the device can expect, always, wherever a flow has
taken them. The flows further down are proofs of these; a promise nothing
walks through is not a promise, it is a hope, and it belongs in `todos.md`
as owed until something holds it.

**B always leaves.** One step at a time, from anywhere, and pressed enough
times it leaves nothing over the wallpaper. There is no surface B cannot back
out of and no depth it cannot unwind. The right paddle is the same promise in
one press.

**The d-pad reaches everything.** Whatever is drawn, the d-pad can stand on
it and A takes it; whatever a finger can press, a button can reach, and the
other way round. [`button-contract.md`](button-contract.md) is the full
statement of both directions.

**The buttons are yours.** A job moved onto another button is moved for good
and everywhere: the new binding means it in every place the job applies, the
old one stops meaning it the same moment, and nothing else so much as
shivers. Several buttons may play one job; a chord is its own binding and
takes nothing from the bare button under it; and one press still does one
thing, so a button moved onto is a button taken, with the job that had it
left saying so.

**The shoulders are places, never actions.** L1 and R1 move between
workspaces or between tabs and do nothing else, so moving around is always
safe to try.

**A button means what the screen says.** What a press does follows what is in
front, read off the compositor in the moment and never remembered, so there
is no stale meaning to be caught by. The guide says what every button does
right now, read out of the same table the daemon reads, so it cannot drift
from the truth.

**One thing is in front.** One chooser at a time, one window per workspace,
nothing floats and nothing stacks. Whatever is in front is what the buttons
belong to.

**The keyboard has the pad while it is up.** Every press goes to the letters
and nothing behind them hears a thing.

**Opening is owned.** A on a thing opens the one surface that owns its kind
-- a picture or a film is the viewer's, a song is the music panel's -- and
no kind is owned by two surfaces. The same thing always opens the same way.

**The thing comes before its furnishings.** What was asked for is drawn
first and the rest fills in around it. A picture shows before the words
under it, the library's rows show before the index has read their tags, and
nothing spins in the meantime.

**Music outlives its panel.** The panel is a remote control, not the player.
Closing it changes nothing about what is playing, and reopening it agrees
with what the ears already know.

**The home screen holds nothing.** Asleep it owns no button: the stick is
the pointer's, A is a click, a tap on the bar lands on the bar even with the
home screen drawn under everything. The first d-pad press wakes it without
moving, and putting the highlight away hands every button back.

**Falling is loud and cheap.** A piece of the desktop dying costs seconds
and says so on the screen; nothing repairs itself in silence. Everything a
person waits for writes down how long it took.

## The flows

Each flow is what somebody does with their thumbs and what they would see,
in order. Every step carries its own assertion: a flow checked only at its
end is a flow that cannot say which promise broke on the way. The files
under `scenarios/` are the vocabulary -- press, hold, drag, tap, wait --
and a flow is a scenario grown expectations.

### Making it yours

The first flow, and first on purpose: every other flow walks the desktop
through the table of what a button means, so the table being movable is the
claim under all of them.

Somebody moves the screenshot off its paddle and onto R2 + A, the way the
setup screen moves it: the move worked out against what everything is bound
to, written to the file, and the file read back the way the daemon reads it.
The new chord takes the picture and does not also click; the old chord takes
nothing, and the paddle bare of its second job goes on scrolling; A on its
own is still a click. Then the desktop is walked with the moved table in
place -- a chooser up, the home screen asleep and standing, the keyboard
raised -- and at every stop the chord still takes the picture, the place's
own buttons still mean what the place says, and under the keyboard nothing
is acted on at all. The file is asked for everything else it can say:
several buttons playing one job, a chord that leaves the bare button alone,
a job with its button taken off playing nothing, a job from some newer
desktop left alone, a file with one bad line refused whole while the table
already loaded goes on answering. And a move onto a taken button takes the
button, with the job that had it written down as playing nothing.

What it walks: the buttons are yours, a button means what the screen says,
the keyboard has the pad, the home screen holds nothing. What it crosses:
the setup screen's move and the file (`console_pad::jobs`), the table and
the daemon (`console-controller`), the stage (`console-stage`), the words to
the home screen (`console-door`).

This one runs: `crates/console-flows/tests/making_it_yours.rs`, at the fast
stage, on every `just test`.

### Pictures, then a film

From an empty desktop: the menu opens, Files is taken, the Pictures place is
walked to, a photograph is stood on and A opens it. The viewer comes up
already showing the picture -- the picture before the words under it. R1
steps to the next thing in the folder; stepping lands on a film and the film
plays; B leaves the film where a picture would have been left, B leaves the
viewer for Files standing where it stood, B leaves Files for the desktop,
and nothing is left over the wallpaper.

What it walks: B always leaves, opening is owned, the thing before its
furnishings, the shoulders are places. What it crosses: the controller's
tables, the pad's routing, the chooser, the files panel, the viewer's kinds
and reel, the door.

### An evening of music

The menu again, Music this time. The Music tab shows the library's folders
before any tag has been read. X raises the keyboard and every letter typed
lands in the search line and nowhere else; a song the index has not reached
yet is still found by its name. A plays it: the Playing tab shows the
sleeve, the name, and a bar that moves. B closes the panel and the song does
not so much as flinch. The panel opened again agrees with the ears -- same
song, further along. Y hands the song to Files, which opens standing on the
file.

What it walks: the keyboard has the pad, the thing before its furnishings,
music outlives its panel, opening is owned. What it crosses: the controller,
the panel, the keyboard, the music library and its player over MPRIS, the
files panel, the door.

### Getting around, and never being lied to

R1 twice and L1 once, and the workspace is where the count says. L2 held, L1
carries the window along and the window actually comes. The guide is raised
in the middle of all this and says what the buttons mean *now*; raised again
with a chooser up, it says the chooser's meanings instead. The launcher
pressed while a chooser is already up does not stack a second one. The right
paddle, pressed deep inside a panel's tabs, leaves nothing at all.

What it walks: the shoulders are places, a button means what the screen
says, one thing is in front, B always leaves (in its one-press form). What
it crosses: the controller's mode reading, the pad, the chooser lock,
hyprland by way of the daemon's dispatches, the guide.

Under the walk there is a sweep, and it is the half that catches drift
nobody went looking for: every job that applies where you are standing and
has a bare button on it is pressed there, and the guide is asked about the
same button in the same breath. A line naming a button that does nothing and
a button doing something the guide never mentions are the two ways a guide
starts lying, and neither of them can be shut by reading the table twice,
because one half of each assertion is a press.

This one runs: `crates/console-flows/tests/getting_around.rs`, at the fast
stage, on every `just test`. Two halves are handed up rather than answered
there. Whether the compositor went where it was asked is the device's, as it
is for every dispatch. And whether a second chooser actually replaces the
first on the screen is a lock between two processes, pressed as one in
`console-panel/tests/the_lock.rs`; what this flow answers is the daemon's
half, that the door it asks through is the door that keeps.

### The home screen wakes and sleeps

Nothing is over the desktop and the home screen is drawn under it, asleep. A
thumb on the touchpad moves the pointer and A clicks: the home screen owns
neither. The first d-pad press wakes it -- a highlight appears and nothing
moves. Walking, standing, and A opens Files. Back on the home screen, A held
on a square picks it up instead of opening it; carried and put down, the
grid is rearranged, and the arrangement survives the home screen being
started again. Through all of it a tap on the bar lands on the bar. B puts
the highlight away and every button is the desktop's again.

What it walks: the home screen holds nothing, the d-pad reaches everything,
a button means what the screen says. What it crosses: the controller, the
homeward socket and the awake note in the door, the home screen, the state
file it keeps.

### Being interrupted

A film is playing in the viewer when a notification card arrives: the card
does not take B, and the film does not lose it. Music is playing when the
controller daemon is killed: it is back inside seconds, says so, reads the
mode fresh off the compositor, and the next press means what the screen says
it means -- the flow continues where it stood, and the music never noticed.
This is the flow for everything that arrives uninvited, because a person is
always in the middle of something when it does.

What it walks: falling is loud and cheap, a button means what the screen
says, music outlives its panel, one thing is in front. What it crosses: the
controller's restart path, the notices, the viewer, the player, the door.

### The whole evening

The long one, and the only one allowed to be slow: waking the desktop from
Steam with the held Legion button, the home screen, music started and left
playing, files browsed under it, a film opened and left again, a screenshot
taken, the brightness nudged from inside a panel, settings opened from the
same place, and back to Steam. Nothing in it is new -- every step already
appears above -- but the promises are asserted the whole way along, because
a row of features that each work is exactly where a mode that lingered or a
button that was kept shows itself.

## Where a flow runs

The stages are the checks' stages, and a flow states its steps once and
answers each one at the cheapest stage that can see it.

**Here.** The daemon's whole loop runs in this process against the captured
devices and the real profile files, with a clock turned by hand. Every
decision is visible -- what was run, what was written to the virtual device,
what was told to the home screen -- so the controller half of every step of
every flow is answerable here, in a fraction of a second, on every `just
test`. What is not visible here is a surface: this stage sees that the
viewer was asked for, not what the viewer drew. [`programs.md`](programs.md)
is the plan that closes that -- a program as a pure function of what it has
heard -- and every program that takes that shape moves its steps of a flow
down into this stage. Until then a flow here asserts the daemon's half of
each step and hands the drawn half up.

**Desktop.** The device's desktop, nested in a window and photographed. It
can say what a person would see in the one way that cannot be argued with:
by looking. A flow at this stage plays its thumb-script and takes a picture
at each named step, and the picture answers -- a card is up, the card is
gone, the picture in the viewer is the picture that was pressed.

**Device.** Somebody's machine, at the end of a deploy, asked only what
nothing else can answer -- the real player, the real decoder, the real
compositor quirk. Everything `checks.md` says about the device tier binds
flows twice over, because a flow holds the machine longer than any check:
`--dry` first, `--yes` knowingly, and whoever is holding the device decides.

## What a flow may assert

The same things a check may, and nothing else: what a person would see. A
flow does not ask which profile is loaded, it asks whether the menu is gone.
It does not sleep for a guess, it waits for the thing. And each step is
asserted before the next is taken, so a red flow names the step and the
promise that broke, in its own words -- `pictures, then a film: stepping
onto the film left the picture's words up` is a finding; `flow failed` is
not.

Ask of every step what it would say if the feature were broken. A flow whose
answer is the same either way is not walking, it is strolling.

## What exists and what is owed

The flows live in `crates/console-flows`: the library carries what every
flow needs said once -- the compositor's answers for the places a flow walks
through -- and each flow is a test file beside it, run at the fast stage by
`just test`. *Making it yours* runs today. The scenario files under
`scenarios/` still play through the real profiles and assert that they parse
and press; the checks still answer their single questions at all three
stages. What stands between this page and the rest of the flows is written
in `todos.md`, each line with what would settle it, so this page describes
the destination and owes nothing to the present tense.
