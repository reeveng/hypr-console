# How a program is built

*A plan, not a description. Nothing below is on the machine yet.*

[`docs/panels.md`](panels.md) says how a card is drawn. This is the layer under
it: what a program on this device is, what reaches it, and what it is allowed to
do about that. The point of it is one sentence — **the same words in, the same
doings out, every time, forever** — and everything here is what that costs.

## What is already right

Three things in this tree are already the shape this asks for, and the plan is
mostly to make the rest of the machine look like them.

`console-input-controller` is the whole idea, written down once already:
*"Nothing in this library opens a device. What arrives is handed in and what to
do about it is handed back, so every decision the daemon makes can be asked of
it twice and answered the same way."* `Doing` is a decision said without being
carried out, and every test in that crate is a transcript. It is the most
reliable thing here and that is not a coincidence.

`console-core-reconnect` is the retry, in one place, with the reason written
down. Four watchers used to hold four copies of it. `console-repository` is the
same move made once more: four programs each walked up the tree looking for the
top of it, and each had its own idea of what marks it.

systemd is the supervision tree, and a good one: `Restart=always`, `PartOf=`,
and `console-fell` so a daemon dying quietly is not a thing that can happen. The
Elixir half nobody has to build is this half.

## What is not, and where the weirdness lives

**A program holds no state, so there is nothing to test.** `Rows::Asked` asks the
world at the moment of drawing — `hyprctl`, `pactl`, `nmcli`, the filesystem. Two
draws a second apart can honestly differ, and neither is reproducible on a
laptop. A panel is not wrong today; it is unfalsifiable, which is worse, because
it means nothing can be proved about it before it reaches a thumb.

**State that belongs to no one is kept where anyone can write it.** The
controller profile is a global variable inside InputPlumber that six programs
set. `$XDG_RUNTIME_DIR/console/tab` says which tab is in front. Another file
remembers the profile from before the keyboard came up. The daemon is stopped
and started with `SIGSTOP` and `SIGCONT` sent at its unit; the keyboard is
toggled with `SIGRTMIN` sent by name. Every one of these is a variable with no
owner, and the comments around them are a record of what that has already cost:
a hook that died between the stop and the start left the daemon stopped for good;
a profile laid over a stale one left the pad answering to a panel that was not
there. **This is the class of fault that works all day and is broken after a
restart**, because a restart is the one moment when nobody knows who wrote last.

**Every program subscribes to everything separately.** Each panel and each bar
module opens its own `pactl subscribe`, its own `nmcli monitor`, its own socket
to the compositor. Twenty-five orphaned subscriptions were once found alive on
the device, the oldest four hours old. That was fixed by making each program
tidy up after itself; it is fixed properly by there being one of each.

**Effects are everywhere.** Thirty-three `Command::new` sites across the tree,
none of them behind a seam, so what a program *does* can only be observed by
letting it do it.

**Thirteen shell scripts, four hundred and forty lines, no tests**, holding the
mode switching: which profile the pad has, which session owns the screen, what
happens when the keyboard goes up. The most stateful logic on the device is in
the only language here that cannot be asked a question twice.

## The contract

One crate, `console-program-contract`, and it is written. This document called
it `console-turn` for a year, which was the wrong name for the one crate every
other program depends on: a name that has to be learned before it can be read.

```rust
/// A program: what it holds, what reaches it, what it does about that.
pub trait Program {
    /// What it holds. Replaced only by `heard`, never reached into from
    /// outside. `PartialEq` is how the loop knows anything changed.
    type State: Clone + Debug + PartialEq;

    /// The words only this program can be told, and the effects only it can
    /// ask for. `console_core_never::Never` where there are none, which is most.
    type Hears: Clone + Debug + PartialEq;
    type Does: Clone + Debug + PartialEq;

    /// What it starts holding, and what it wants said to it.
    fn opening(argv: &Argv) -> Opening<Self::State>;

    /// One word in, and what it decided.
    ///
    /// Pure. No clock, no filesystem, no process, no environment. Everything
    /// it is allowed to know is in `state` or in `word`, which is what makes a
    /// transcript a proof rather than an anecdote.
    fn heard(state: &Self::State, word: &Word<Self::Hears>) -> Turn<Self::State, Self::Does>;
}

/// One turn: what it holds now, and what it wants done.
pub struct Turn<S, D> { pub now: S, pub doings: Vec<Doing<D>> }
```

Four things in that are not what this document asked for, and each of them is a
thing writing it settled.

**`showing` is gone from the trait, and there is no `Doing::Show` either.** The
draft above listed "draw these pages" as an effect *and* gave the trait a
`showing`, which is the same thing said twice; only one of them can be the truth
about what is on the screen. It is `showing`: the loop redraws from the state
when the state changes, which is what `PartialEq` is for. And it is not on this
trait, because seven of the fourteen programs never draw and a `Vec<Page>` they
all return empty is exactly the ceremony this document is against. A program
that draws implements a second trait, in the crate whose vocabulary a page is.

**`offers()` is gone until something wants it.** Stage 6 was already conditional
on two programs asking; a registry in the trait before then is the piece most
likely to be built bigger than anything needs.

**`Hears` and `Does` are new, and the reason is stick-scroll.** `Word` and
`Doing` are closed sets shared by every program, which is what makes them worth
having and is also how this crate becomes the shelf `CLAUDE.md` forbids: a
variant nobody else uses is one program's private business kept in a shared
type. The sets were settled against three real programs -- `stick-scroll`,
`console-sky` and `settings-panel` -- and one of them broke the shape.
Everything the other two do is a kind of effect anybody might ask for.
`stick-scroll` emits pointer motion on a virtual device it holds, and no other
program here will ever want to, because the whole plan is that one program reads
the input and the rest are told.

So the shared sets stay about *kinds* of effect and a program names its own.
What is not given up is the transcript: a private doing is still a value that
was decided rather than carried out, so it is still in the list a test asserts
on. What is given up is that shared code can carry it out, which is the honest
cost -- the program that asked for it is the one that knows what it means. The
first one is `controller-profile`'s touchpad buzz, which is a glob over
`/sys/bus/hid/devices/*/touchpad/vibration_enabled`: how many there are and what
they are called is the driver's, so it is not a path anybody can write down.

**The loop is a crate of its own, `console-program-runtime`, and it is not the
only loop.** GTK draws on one thread and will not be driven from somebody
else's, so the panels keep theirs and drive the same contract from glib. What is
here is the loop for the programs with no toolkit: it carries out the doings,
runs a program's asks to completion and puts the answer on the queue for a later
turn, hands the stretches of time back as words, and ends when a program has
nothing left to wait for. What is shared between fourteen programs is the shape
of a decision, not the shape of a loop.

`Word` is everything that can arrive — the runtime is up, a subscription spoke,
a timer the program asked for came round, another program asked it for
something, something it set going has come back, it is being stopped. `Doing` is
every effect, said without being carried out — run this and tell me what it
said, start this and forget it, ask that program for this, draw these pages,
listen to this, stop listening, write this file, say this on the screen, stop.

The runtime carries out the doings. `heard` never does. That is the whole
mechanism, and everything the plan claims falls out of it:

- **Repeatable.** Same transcript, same doings. A test is a list of words and a
  list of what should come back, and it runs on a laptop with no compositor, no
  controller and no network.
- **Testable.** The two things that cannot be tested today — what a panel decides
  and what a daemon does about a button — become the only two things there are.
- **Consistent.** One shape for twenty-one programs, so learning one is learning
  all of them, the way learning one panel is already learning all of them.
- **Immutable.** State is replaced, never mutated. Nothing outside a program can
  reach into it. Nothing on disk is shared.
- **Write once.** A program that is a pure function of its words does not rot when
  something else on the machine changes; it either gets a word it knows or it
  does not.

## Every shared variable gets an owner

This is the half that fixes restarts, and it is worth stating separately because
it is the part that is not a refactor.

The controller profile is owned by the controller daemon. Nothing else sets it,
and nothing else has to ask: what the pad should be wearing is a function of
what the compositor says is in front of you, so the daemon reads it rather than
being told it. A panel that wants the chooser's buttons does nothing at all --
the buttons a chooser needs are a column in the daemon's own table and a
chooser being up is a thing the daemon can see. The keyboard hook is gone the
same way. No `SIGSTOP`, no `SIGCONT`, no
`$XDG_RUNTIME_DIR/console-profile-before-keyboard`, and no `osk-hook` at all.

Which tab is in front is owned by the panel that has it. What the bar shows is
owned by the bar. If two programs need to agree about something, one of them
owns it and the other asks.

The rule, said as a rule: **a program may hold only state it has a `Wants` for.**
Anything with no subscription behind it must be asked for at the moment it is
drawn, exactly as today. This is the one real hazard in the whole plan — held
state that nothing refreshes is a reading that is confidently wrong — and this
rule is what keeps it out.

## One pool, `console-events`

A daemon that holds one subscription per source and hands the words to everyone:
the compositor's socket, `pactl subscribe`, `nmcli monitor`, the bus name mako
owns, systemd's unit changes, the player, and a path being watched. It speaks
over a socket in the runtime directory, and `console-core-reconnect` is what
reconnects to it.

Two things it does that no program can do for itself. It **replays the last word
on a topic to whoever has just subscribed**, so a panel opening knows the volume
before anything changes it — which is most of what `meanwhile` is working around
today. And it **drops a subscriber that has gone**, which is the orphan problem
solved by construction rather than by everyone remembering to tidy up.

It restarts like everything else here, and a program with no pool is a program
that asks the machine directly and is merely slower — the same rule
`console-bar`'s tick already keeps.

### What is in

`console-events`, and `console-events` is the program. Two sources — the
compositor and the sound — and every program that watched either of them asks
the pool. The other topics answer nothing and say so on the journal, which is
what the one-at-a-time rule should look like: a topic that is quiet rather than
a pool that is broken.

The sound came second because it is the one with sixty-five processes behind it
in the measurements, and it was watched before it was believed — under the
unit's own confinement rather than in a shell, which is where the doubt was.
`PrivateDevices=yes` and `RestrictAddressFamilies=AF_UNIX` do not stop `pactl`:
it reaches the server over a socket in the runtime directory and never opens
`/dev/snd`. Killed underneath, it is started again and whoever was listening
goes on hearing without reconnecting, because the reaching-again is on the
pool's side. A pool killed with `-9` leaves no `pactl subscribe` behind, which
is `alongside` doing what it says and was checked rather than assumed —
leaving one would have been this crate committing the fault it exists to end.

Three things settled by writing it rather than by arguing about it.

**The pool does not parse anything.** A line arrives as a line and leaves as a
line. Which of the compositor's lines are worth waking for is still
`console_wallpaper::covered::worth_waking_for`, because the bar keeps different
ones and a pool that decided that would be deciding it for both. What the pool
knows is who wants what.

**Where the compositor's socket is was in this tree three times** —
`console_onscreen`, `console_wallpaper::covered` and `console_panel::door` each
worked it out. That is the same fault as the subscriptions, one layer down. The
wallpaper's copy went with the subscription it was for, and the pool asks
`console_onscreen`.

**Four watches on that socket are one.** `music-bar`, `bar-door`,
`stick-scroll` and the status bar's `watch` each opened it to learn the same
thing, and all four ask the pool now through `console_events::layers`.
`console_onscreen::watching_layers` is gone rather than left standing beside
them, which is the rule this crate only earns by being kept: a source with two
observers is a source nobody has moved off.

Which lines are worth asking after stayed in `console_onscreen`, on the near
side of the socket, because the bar keeps different ones from the wallpaper and
a pool that decided would be deciding for both. And `Heard::GotIn` came out of
it: what these watches carry is *ask again* rather than an answer, so a gap in
one is an icon quietly wrong with nothing on the way to correct it. The pool
says *you are in* on every reconnection, whoever does not need it ignores it,
and `bar-door` — the one with no tick under it — is why it exists.

**The orphan fix is a write that would not go through.** Nothing has to be
tidied up by the program that left, and nothing has to notice that it left: a
socket nobody is holding refuses a write, and that is the same moment.

What the pool costs is a second thing that has to be up, and that is the honest
half of this. A program that held its own subscription now depends on a daemon
as well as on the source. The answer is the one every unit here already gives
— it restarts, and the client reaches for it again for as long as it wants
words — and that answer is only good because both halves of it are true.

Where a source is opened is handed in rather than reached for, the way
`console_input_controller::turning` is handed a machine. So the test runs the
real serving loop, the real wire and the real client over a real socket, with a
source the test speaks through. Without that seam the only compositor is the one
the test is running under, and *is a late program told the last word first*
could not be asked at all.

### What the runtime does with the ask

`Wants::Words(Topic::Sound)` is *tell me when this changes*, and for as long as
the runtime answered it with a line on the journal both halves of this were
green and the desktop still polled. The program said what it wanted, the pool
could have told it, and nothing carried the question from one to the other.

It does now. `console-program-runtime` holds one `Listening`, `Doing::Listen`
adds a topic to it and `Doing::Deafen` takes one away, and a `Changed` from the
pool reaches the program as `Word::Changed` the way a stretch of time reaches it
as `Word::CameRound`. The decision that made this safe was already made above:
the round stays as what a program falls back to when the pool is down, so a
program with no pool is slower and never wrong.

**It is one wait now rather than a sleep.** The loop blocks on the pool's
channel until the next round falls due, which is the same wait for both reasons
a program can be woken, and it is why nothing in that crate declares a sleep any
more — `due()` was the one wait in the runtime EXPLICIT021 made say so, and it
is gone. A program that wants no topic at all still waits there: the channel is
open and quiet, and a `recv_timeout` that times out is the round coming round.

**Deafening works, and a panel coming back is the reason it had to.** `listening`
holds what is wanted rather than taking it once, so `also` and `not` write a
`listen` or a `deafen` down the connection that is already open, and a
reconnection asks for whatever is wanted at the moment it gets in. What makes
that worth having is the replay: a program that asks again is told the last word
on the topic straight away rather than waiting for the next change, so hanging up
costs a panel nothing when it comes back. That is `the_deafening`, pressed
through a real socket, because it is the half `pool`'s arithmetic cannot answer.

The seam itself is pressed too. `console-program-runtime`'s `the_words` runs a
real pool on a real socket and a real program on the real loop, and the only
thing the test supplies is the source — because the only sound card is the one
it is running on, and because without that test the two halves would go on being
green separately.

### What is not in, and it is the half that matters

**Every panel card still matches `Doing::Listen(_) | Doing::Deafen(_) => {}`.**
A panel drives this contract from glib, so what the runtime now learns a panel
does not. That is the cost of the two loops this document argues for elsewhere,
arriving where it was always going to arrive: a thing worth doing has to be done
twice, once in the runtime and once where a panel's main loop lives. The second
half is not written.

It is worth saying what those arms are and are not, because from a distance it
reads as a panel doing its own polling instead. It is not: no card has ever
emitted a `Doing::Listen`, so there is nothing being translated into anything.
The arms are a match staying exhaustive over a shared set. What is owed is a
`Listening` held by the panel with its words pumped onto the main context — and
it waits on a card that would ask, which waits on a source.

**A topic with no source is what everything else waits behind.** One source at a
time is the rule and it is the right rule. `Sound` is there now; `Player` is not,
so the music bar and the music card — the two with the most to gain — still have
nothing to ask for. Each new source is a program of somebody else's watched
until it is known what it does when it is restarted underneath, which is the
work, and it is why the number of sources goes up slowly and on purpose.

The volume rocker is the one to stop expecting anything from. `console-volume`
presses `pactl` and reads the level back in the same run; it holds no state
between two presses and has nothing to subscribe to. What wanted `Sound` was the
status bar's reading of it, and that is what moved.

**Nothing has yet chosen to deafen.** The machinery is there and pressed; what
is missing is a program that hangs up when it stops being looked at. A panel
that is not on the screen wants nothing; a bar module behind a chooser wants
nothing until it is uncovered. Today both would be woken for every word, which
is the pool doing to a handheld's battery a gentler version of what the orphaned
subscriptions did. Whether a listener should be connected is a question about
the window's own state machine — shown, covered, in the background — and the
panel already holds that answer, because that state is what it draws itself
from. The subscription should follow it.

What makes hanging up safe is already built: `listens` replays the last word on
a topic to whoever has just subscribed. Without that, a panel coming back would
be a panel drawn from whatever it remembered before it left, and deafening would
be trading a warm battery for a wrong screen. With it, a panel that comes back
asks and is told what is true now — so the expensive thing to be is *visible*,
which is the only sensible answer for a machine somebody is holding.

## Threads, honestly

GTK draws on one thread and that is not negotiable, so the panels stay
single-threaded where they are drawn. Nothing else here is CPU-bound: a panel
waits for a thumb, a daemon waits for a button. Threads bought by the dozen
would be threads asleep.

Two jobs are genuinely worth the parallelism and both are already written down
as wanted: `music-index`, which is minutes of `ffprobe` over thirteen hundred
songs, and the download panel's ten pictures, which are ten curls and ten ffmpegs
run one after another. Those get a small pool. The runtime gets one thread for
the pool socket and one per `Run` in flight, so a slow command never makes a
panel deaf — which is what `later` is for today, one panel at a time.

Programs never share memory. `heard` owns the state; everything that leaves is a
message.

## Spawning and extending

Two mechanisms, both small, and the second one waits until something wants it.

**Spawning.** `Doing::Start` runs a program in a scope of its own rather than in
the caller's control group. This is also the settling of an open item: everything
opened from the menu is currently in the controller's control group, so
restarting the controller takes every application with it.

**Extending.** A program declares what it `offers()`. A panel's tabs are gathered
from what the registry says is on offer rather than written into the panel, so
Music can put a Playing page on the bar and Download can put a row on Files
without either crate knowing about the other. The registry is `desktop.conf`'s
`[build]` list, which is already the list of every program on the machine.

Left until last on purpose. It is the piece most likely to be built bigger than
anything needs, and the only honest reason to build it is two programs already
wanting it.

## Everything arrives, and each program decides what it meant

The input is where this document's argument is easiest to see, because the
question "what is X bound to" has no answer on this machine.

Not the profiles: all four pass X through untouched, `North` in and `North` out.
Not the compositor: Hyprland binds `SUPER+K` and `XF86Calculator`, and a
controller sends neither. Not the controller daemon: `BTN_NORTH` is not in any
of its tables. What actually shows the keyboard is wvkbd having the same device
file open and deciding for itself. Two programs open `/dev/input` and one of
them takes it to mean something.

That is not a binding. It cannot be one either, because `hyprctl` cannot bind a
pad button: the only thing that can give a controller button a meaning is a
program reading that pad. So the meaning of every button on this device is
distributed across four YAML files, three Rust tables, a Lua config and a C fork,
and no single place can be asked what X does.

**One program reads the input, and everything else is told.** The pad, the
keyboard InputPlumber publishes, the touchpad, the volume rocker: one reader
opens them, and every press goes into the pool as a word. Nothing else opens a
device. Every program then decides what it makes of a press, the same way it
decides what it makes of the volume changing, and the ones that make nothing of
it make nothing of it.

What this buys is not tidiness.

**Modes stop being device surgery.** The pad is switched between profiles today
to change what the buttons mean while a panel is up, and *a profile switch
destroys the pad and builds a new one every time* --
`console_input_controller::turning::Gone` says so and calls it the ordinary
state of things. Everything downstream is built to survive that: reopening a
turn or two later, throwing away a quarter second of backlog, settling for half
a second after. All of it exists because the meaning of a button is stored in
the device rather than in a program. When the reader holds the mode instead, the
device is never rebuilt, and that whole machinery goes with it -- along with the
race this document opened with.

**A button's meaning becomes one table, tested on a laptop.** X shows or hides
the keyboard because a line says so, and the guide, the button contract and the
behaviour are read off the same line. Today the guide can only promise what the
YAML says, and the YAML says `North`.

**The signals go.** The keyboard and the daemon do not have to be stopped and
started at each other with `SIGSTOP` and `SIGCONT` to keep them off one pad,
because there is one pad reader and it knows who is in front.

**The fork loses its reason to read a pad.** Stock wvkbd shows and hides on a
signal and has no opinion about gamepads; the reading was added to it because
nothing else was going to do it. What it does not lose is the fork, and an
earlier draft of this document said otherwise. Told rather than reading is
still told, and stock wvkbd has no way of being told anything: its whole
outside is one signal that toggles it. A keyboard that takes a direction from
somewhere else is a keyboard that was changed to take one.

That is still the argument and it no longer needs a fork to be the answer to.
The keyboard is ours, so it reads the pad because it was written to, in
`console_pad::vocabulary`'s words rather than in kernel codes. What has not
changed is the consequence below: a keyboard that reads a pad needs the pad
routed to it, which is the profile.

Two things it does not buy, said plainly.

InputPlumber stays. It grabs the Legion Go's real devices and normalises them
into one pad, and that is worth having and not worth rewriting. What changes is
that it is given one profile that never changes and translates nothing, so no
target is ever destroyed. The meaning moves out of the YAML and into the reader;
the normalising stays where it is.

Game Mode is the one real profile switch left, and it should stay one. Steam
wants a real pad, and the reader steps aside when Game Mode has the screen. That
is a switch per session rather than a switch per panel, which is the difference
between something that happens when you decide to leave and something that
happens every time you open a menu.

This replaces most of stage 4. The ordering fix already on the machine --
`console-input-keyboard.service` waiting for the controller -- is a splint on
the version of the world where two programs share a pad. When one program reads
the pad, the ordering stops mattering and the line should come out, along with
the check that watches for it.

### What is in, and what the fork still holds

The mode is read off the compositor now, and what a button means is one table.
`console_input_controller::mode` is Desktop, Tabs or Keyboard, decided by what
the compositor says is on its own screen; `console_input_controller::means` is
the job each button does and who carries it out. Three ownerless variables went
with them: the profile-before-the-keyboard file, the SIGSTOP that made the
daemon stand down, and the SIGCONT in `controller-profile` that undid it.

The daemon standing down by itself rather than being stopped is worth more than
it sounds. Stopped is not deaf: the devices stayed open, the kernel went on
queueing, and the whole backlog arrived in one instant when the keyboard went
away -- every button pressed while typing, in order, against a desktop that had
moved on. That is how the machine once left for Game Mode on its own.

`osk-hook` is gone rather than converted. It ran at both ends of the on-screen
keyboard and did two things: it stopped the daemon with a signal, and it loaded
the pad profile the keyboard needs, remembering the one that was there in a
file so it could be put back. The daemon does both now, and remembers neither.
Which profile the pad wants is `Mode::profile`, a function of what the
compositor says is in front of you, so leaving the keyboard is not a restore --
there is nothing to restore to, only somewhere you now are. That is what
retires the file, and with it the one case the hook had to guard: a panel
closed while the keyboard was over it had already put the desktop back, and
laying the remembered profile over that left the pad answering to a panel that
was gone.

The unit lost its `WVKBD_ON_SHOW`, its `WVKBD_ON_HIDE` and its `ExecStopPost`
with it. The last of those was there because a keyboard that died left the pad
on a profile nothing was listening to; a keyboard that dies now takes its layer
off the screen, the compositor says so, and the daemon puts the pad where the
screen says it belongs.

What is not in is the part the keyboard holds. Raising it is a job like any
other: X arrives at the daemon as a key the keyboard cannot see, and the daemon
runs `keyboard-toggle`. Putting it away is the keyboard's, because while it is
up it reads the pad itself and the daemon acts on nothing, so the second press
reaches it as the pad button it always was. One button, two programs, and
neither of them ever acting on the same press -- which is the one thing here
that has to be pressed rather than read about, and `todos.md` says so.

The profile is still switched when the keyboard opens, and that is the one
switch left. The keyboard needs raw d-pad events off the gamepad target to move
between keys, and under the profile the desktop wears the d-pad is routed to
the daemon instead. Opening a menu used to cost a switch too, several times a
minute; that is gone, and this one happens when a person asks for a keyboard
and can afford the beat.

So the last profile switch used to rest on one binary that no other repository
could reach, and that was the decision this document kept arriving at from
different directions. The binary is ours now. The switch is not gone with it --
it is a function of reading a pad, not of who wrote the reader -- and what is
left of it is `todos.md`'s claim crate.

## Everything in Rust, and what is written in something else

The aim is that nothing on this machine is written in a language that cannot be
asked a question twice. Every script the desktop runs has gone, and so have the
laptop tools: the deploy, the rename, the pull and the uinput rule are
`Program`s now, and `tools/` holds nothing but the lint suite. What decides
whether the list is finished is whether anything left under `files/` or `tools/`
has a `#!` we wrote.

`/usr/local/lib/console/palette.sh` was going to go with them, and does not.
This document said it existed only because `osk-start` was a shell script and
needed the palette as shell variables, and that was wrong: it has two other
readers. The nested desktop sources it to set its ground before anything else
is up, where a shell is all there is, and the checks read it to know what colour
a thing on the screen should have been.

So it stays, and what changed instead is that there is one reader of it.
`console_core_colour::spent::read` is that reader; `osk-start` and the checks
each had a copy of the same six-line parser, which is the fault this desktop
keeps having with colours, in miniature.

What stays in another language stays for a reason, and it is worth being clear
about each, because all but one of them are fine and that one was not.

**The InputPlumber profiles** are YAML because InputPlumber reads YAML. It is
their format, not ours. What we can do is what `console-input-gamepad` already
does: hold the button contract as a Rust test over those files, so a profile
that stops meaning what the guide says it means fails on a laptop.

**`hyprland.lua`** is Lua because Hyprland reads Lua. The same answer, and
`console-button-guide` already reads the binds out of it.

**`steamos-session-select`** is SteamOS's script, carried unchanged. Rewriting
someone else's ninety lines to be ours is how a carried file quietly becomes a
fork nobody remembers maintaining.

**The migration sweeps** in `migrations/` are shell because a sweep is a handful
of `mv`s against paths a manifest stopped naming, and
`console-manifest-migrations` -- which decides *what* is owed, and is Rust -- is
the half that could be wrong. `docs/migrations.md` is the argument.

**`wvkbd-mobintl` was the one that was not fine, and it is now ours.** It was a
compiled binary in `files/`, built from a C fork that lived on the laptop and
nowhere else: everything else on this device could be rebuilt from what is
written down, and this could be rebuilt from nothing but that one directory,
while holding the X button.

It is a Rust program in `crates/console-input-keyboard`, in `[build]` like every
other program here, and the device compiles it. Which is the third of the four
ways out below -- the one this section argued against, on a premise that was
wrong. The four are left standing because the argument is worth having in front
of you, and the correction is written where the mistake was made.

Bring the fork's source into this tree, so `console apply` builds it like every
other program and the button contract is written down where it can be read.

Take the gamepad reading out of it, so a program of ours reads the pad and
tells the keyboard what to do. This document used to prefer this one and no
longer does, because it is the first one with an extra step. Stock wvkbd cannot
be told: it has one signal and no other way in, so being told means patching it
to listen, which is bringing the fork into the tree with more work attached.

Write our own keyboard, in Rust. Everything except the typing is already here
and used by five other surfaces -- gtk4, gtk4-layer-shell and cairo for the
drawing, `console-core-colour` for the palette `console-input-keyboard`
currently launders into wvkbd's argv. **This is the one that was taken.**

Bring the source in and take `gamepad.c` out of it, leaving a small socket to
be told on. This was the one preferred here, and the rest of this section is
why -- with the correction that undid it marked where it belongs.

**Thai is the thing that decides it.** She writes Thai and the on-screen
keyboard is the only keyboard this device has, which
`crates/console-manifest-engine/tests/the_keyboard.rs` says in its first line
and guards in two checks. Thai is not latin with accents: every key carries a
Thai letter and the shift level carries a second one rather than a capital, so
it is a layer of its own.

A keyboard of ours would have to produce those letters, and the obvious way --
a uinput device, using the `evdev` three crates already carry -- cannot. A
uinput device emits key codes and the compositor applies its own keymap, and
`hyprland.lua` says `kb_layout = "us"` and nothing else. So a uinput keyboard
here can produce exactly what a US layout produces. wvkbd does not have this
problem because `zwp_virtual_keyboard_v1` lets a client upload its own keymap,
which is why `keymap.mobintl.h` is twelve thousand lines and carries eight
layouts. Reaching Thai without it means either a second layout group on the
physical keyboard, which couples the keyboard on the screen to the state of the
keyboard nobody is holding, or writing raw Wayland protocol -- and
`gtk4-layer-shell` is not a precedent for that, being a C library called
through bindings rather than protocol written here.

**And that is where this was wrong, in one word: "ours" was read as "uinput".**
Nothing made a keyboard of ours a uinput device. `zwp_virtual_keyboard_v1` is a
Wayland protocol and not a wvkbd feature, and a Rust client of it uploads its
own keymap exactly as the C one does -- `wayland-protocols-misc` has the
bindings, so it is a generated interface rather than protocol written by hand,
and `gtk4-layer-shell` was never the precedent that mattered. The keyboard here
is a `wayland-client` of it, and it is the reason this is not a uinput device.

The twelve thousand lines were not needed either, and this is the part that was
hardest to see from where the argument was standing. `keymap.mobintl.h` carries
eight layouts because wvkbd ships its keymaps; there was no reason for a
keyboard of ours to. `xkbcommon` composes one from the symbols the system
already has, so Thai is a layer because `th` is in `/usr/share/X11/xkb/symbols`,
and adding Russian is the word `cyrillic` on the unit's command line rather
than a table in this repository. Twelve thousand lines of keymap data was the
single largest number in the case against writing our own, and it was a cost
that did not have to be paid at all.

**The fourth costs less and buys more.** The fork's reason for reading the pad
is real and any keyboard here inherits it: a layer surface that took keyboard
focus would stop a real keyboard typing into the window underneath. But the
answer does not have to be the `keyboard` profile, and the profile is where the
damage is, because loading one destroys the pad and builds another every time.
That rebuild is what the X flake is made of. The daemon already reads that pad;
while the keyboard is up it can hold the device open exclusively so nothing
else sees the presses, and let go when the keyboard goes. Told over a socket
what the presses came to, the keyboard needs no pad and no profile, and the
profile and its YAML go.

So the plan was: the source comes in and is built by `console apply` like
everything else, `gamepad.c` goes, and a socket arrives.

What happened instead is that the third was taken once the Thai objection fell,
and it went further than the plan in the direction the plan wanted. There is no
C on the device and no socket, because there is no second program to be told
anything: the keyboard reads the pad itself in `gamepad.rs`, in
`console_pad::vocabulary`'s words, which is the same table the daemon reads
buttons out of rather than a second copy of it. The one part of the plan that
is still owed is the part that was never about the fork -- the `keyboard`
profile, and the pad rebuild that a profile load is. See `todos.md`.

### What the fork actually was, and what internalising it cost

The fork is not the loose pile of C this document has twice called it. It is a
clone of upstream with seven commits on top and a clean tree:

    1212b61  Read the controller directly, and navigate the keys with it
    bd1525d  Measure to a key's edge, wrap at the edges, and read either stick
    9af75ce  Type the selected key on a stick press too
    b38097e  X is the keyboard, and nothing else
    3752fb2  The selected key has a colour of its own, and it is solid
    b3d1627  Thai, so she can type in her own language
    142042f  The keymap follows the keys back to the first layer

So the whole difference between this keyboard and a keyboard anybody can
download is seven patches, and the work is exactly readable rather than
estimated. The first four are the gamepad reading and are what the socket
replaces. The last three stay, and one of them is Thai.

What is actually wrong is not that the fork is unversioned. It is versioned and
it is clean. Its `origin` is upstream, which is a remote nobody here can push
to, and the branch the seven commits are on -- `codincod-controller` -- tracks
nothing. So the fault is not a missing remote, which `git remote add` would
answer in a line. It is that there is nowhere of ours to push to, so seven
commits exist on one laptop and nothing in this repository can reach them. That
is a smaller problem than the one written down here before, and it still needs
somewhere to go.

**The licence decides the shape.** wvkbd is GPL-3.0 and this workspace was MIT
when that was written; it is AGPL-3.0-or-later now, and the port is why -- a
derivative of copyleft work carries the copyleft, so the field was never a
choice. `crates/console-input-keyboard` says GPL-3.0-or-later on its own,
because the AGPL is not a later version of the GPL and wvkbd's "or later" does
not reach it. `console-manifest-publish` already knows: `tree.rs` holds `FORKS`
and `is_fork`, and `lib.rs` says why the two compiled programs are not carried.
But `is_fork` matches *paths under `files/`* -- it is written against binaries.
C source brought in under `crates/` would be tracked like anything else, matched
by nothing, and published. The fork is not to be published, so internalising it
means teaching `console-manifest-publish` a source exclusion beside the binary
one, and `the_forks_are_not_carried` has to cover it. That is real work, it
belongs in the plan, and it must not be discovered at publish time.

Done, and it was `FORK_SOURCES` in `tree.rs`, and then it was not. It went in
with the source rather than after it, which is the only reason it was never
discovered at publish time. `FORKS` lost the keyboard binary in the same commit,
and lost `hyprsession` when the session keeper's source arrived and `[build]`
began making it.

Then the exclusion itself went, and the note written into it is why it could.
That note said, plainly, that it was enforcing a decision and not a law: GPL-3.0
does not forbid publishing this source, it forbids relicensing it, and GPL source
beside a repository under other terms is fine as long as that subtree keeps its
own licence and says so. What was being enforced was a choice somebody made. Put
the other way round -- as though the machinery were a compliance gate -- the next
person to read it would have been afraid to touch it, and would not have changed
it when the choice changed.

The choice changed, because a crate is not a path. Holding a *binary* back leaves
a manifest line to put back; holding a *crate* back leaves `members =
["crates/*"]` and a `Cargo.lock` naming a member the copy does not carry, which
is a public copy that will not resolve, let alone build -- and nothing would have
said so, because nothing published between the session keeper's source landing
and this. So `crates/console-resume` is carried like any other crate. What stayed
is `VENDORED`, which asks the question that is actually somebody else's to ask:
whether the licence it came with travels with it.

**`console apply` would have to build C.** It runs cargo today and that is all
it runs. Settled by not needing to: the keyboard is a crate, the C it was ported
from has gone, and an apply has no C to compile.

**`docs/forks.md` is not missing, and must not be written by hand.** It is
generated at publish from `crates/console-manifest-publish/papers/forks.md`, so
it exists in the public copy and nowhere else -- which is exactly the reader
`the_keyboard.rs` addresses in that skip message, since the public copy carries
neither the keyboard nor the answer it would give. A hand-written one here would
collide with the generated one. What was wrong with it was its contents: it told
a reader to clone upstream and `make wvkbd-mobintl`, which builds a keyboard
with neither the Thai layer nor the X binding, and said nothing about the seven
patches not being published. That was a live fault in published documentation
rather than a plan-dependent one. Fixed in `papers/forks.md`, which now says the
keyboard is this repository's own and that the wvkbd source kept beside it has
since gone.

**Whoever draws owns the answer to "is it up".** `keyboard-toggle` is one line
and stateless because the keyboard answers it, and the comment above it records
an earlier version that kept the answer in a file and guessed wrong every other
press. Anything that lets a program be told what to do makes it tempting for the
daemon to track visibility so it knows when to grab. It should not; that is the
ownerless variable this whole rework is about, arriving by a new road. The
daemon reads it off the compositor, which is what `Mode::seen` is.

## What is not reworked

Some of these programs run, decide, and exit: `console-buttons`, `put-away`,
`one-format`, `download-find`, `download-get`, `files-thumbs`, `music-index`,
`sky-press`, `dictate`, `cover-ascii`, `console-palette`, `console`,
`console-engine`, `console-battery`. They are already pure functions with a
`main` around them. A state machine holding one state is ceremony, and
ceremony is the thing this document is against.

Five more are laptop-only — `capture-devices`, `console-check`,
`console-desktop`, `console-emulate`, `console-manifest-publish` — and a tool
that fails on a laptop fails in front of somebody. They are converted for
tidiness, not for reliability, and they go last.

That leaves **seven daemons and seven panels**, which is the whole of the rework.

## The order, and what each stage costs

Each stage leaves the tree green and the device working. Nothing is a flag day:
a program moves to the runtime one at a time, and the old path keeps working
until the last one is off it.

| | | |
| --- | --- | --- |
| **0** | ~~Find the restart fault~~ | done, and it is the argument below |
| **1** | ~~`console-program-contract`: the contract, tested alone~~ | done, with the transcript harness and three programs on it |
| **2** | `console-events`: the pool, one source at a time | **begun: the compositor, `console-sky` on it, and the runtime asking it** |
| **3** | ~~The shell scripts, into Rust~~ | done: none of ours is left |
| **4** | One input reader, then seven daemons | **begun: the mode and the table are in** |
| **5** | Seven panels | ~1 day each |
| **6** | `offers()`, if two programs want it | ~400 lines |
| **7** | ~~The laptop tools~~ | done: the deploy, the rename, the pull, the uinput rule and the measurement |

**Stage 3 is finished, and it finished by being stage 1's first implementors.**
The three that raise a notification, the four knobs, the two ways to a session
and the one that starts it, and the keyboard's colours went one at a time.
`osk-hook` was not converted, it was deleted, which was the right end for it;
`osk` went the same way, into the keyboard that now answers the signal it sent.
The two that were really a program taking the front of the machine came out with
`console-input-focus`. What was left at the end was `keyboard-toggle`,
`keyboard-show` and `controller-profile`, and those three were written to the
contract rather than merely written in Rust -- which is why the crate landed
holding something up rather than beside it.

The two keyboard ones are a line each and were not converted for the line. What
they buy is that the difference between them is a test now:
`asking_on_somebodys_behalf_only_ever_shows` is what stops the browser's search
card putting the keyboard away by asking for one while it was up, and nothing
could have said so while both were `exec pkill`.

`controller-profile` is the one that was worth it, and it is worth it for the
reason this whole document is about. Its wait for InputPlumber was a `sleep 1`
loop with a counter in it, up to a minute -- a decision that can only be
observed by waiting for it. It asks for a stretch now and is told when one has
gone by, so the cold login is pressed in no time at all and so is the minute
that ends in nothing. Two things fell out of writing it down: the profile paths
are built from `console_input_gamepad::router`'s own constants rather than
spelled a second time, and somebody who asked which profile was on while the bus
was down used to get a blank line and a success, which now says what happened.

Three things the conversion turned up that no test could have asked for while
they were scripts. `grim` is not in `[packages]` and never has been -- the
screenshot reached for it, and the scan that holds the crates against the
package list can only read crates. `date` likewise. And `console-screenshot`
read `$XDG_PICTURES_DIR`, which a login shell sets and a session does not, so
run from a button it fell back to a folder named Pictures every time -- which is
the exact mistake `console_files::places` was written to avoid, in a crate that
already knew better.

Roughly four to six thousand lines net, across about fifteen crates.

**Stage 0 is done, and it is the argument for all the rest.** The fault is the
X button not raising the keyboard after a restart, which comes back on the next
restart. It is a race, and every ingredient of it is something this plan is
about.

Nothing in this tree turns X into the keyboard. The profiles pass it through
untouched -- `North` in and `North` out, which
`console-input-gamepad/tests/the_button_contract.rs` asserts on purpose -- and
no program here reads `BTN_NORTH`. The handler is inside `wvkbd-mobintl`, which
is a compiled binary carried in `files/` whose source is a C fork on the laptop,
reachable from nothing here. `gamepad_read`, `GamepadToggle`, `gamepad_lost` and
`gamepad_alive` are all in it. It finds the pad in `/dev/input` and opens it
itself.

*Written before the port, and left as the description of the fault it is about.*
The keyboard is `crates/console-input-keyboard` now and `gamepad.rs` is where
those four functions went, in Rust and testable without a pad in the room. The
race below is unchanged: it is about two programs opening one device with
nothing said about the order, and rewriting one of them in a language you can
ask questions of does not settle who opens what first.

So the pad is a resource two programs use and neither owns, and they are started
with nothing said about the order:

- `console-input-keyboard.service` and `console-input-controller.service` are both only
  `WantedBy=console.target`. Of the nine units here, `console-sky` is the only
  one that declares an ordering at all.
- `console-input-controller.service` runs `controller-profile desktop` from
  `ExecStartPost`, and a profile switch destroys the pad and builds a new one.
  Its own unit file says so.
- InputPlumber is not on the bus at login, which is why `controller-profile`
  waits a minute for it in a shell loop.

Started together, wvkbd can open a pad that `controller-profile` then takes away
underneath it. `gamepad_lost` and `gamepad_alive` are the fork trying to survive
that, and whether it does is timing. A restart is a coin toss.

Three separate things this document argues for would each have prevented it. The
pad would have an owner, and wvkbd would be told about it rather than going to
look. The units would say what they come after, or better, the keyboard would
ask the controller for the pad and wait to be answered. And the decision would
be in a program that can be handed a transcript rather than in a binary nothing
here can ask a question.

Fixing it is not the rework, though, and should not wait for it. The narrow fix
is an ordering: the keyboard starts after the controller has loaded the desktop
profile, so there is exactly one pad by the time wvkbd looks for it. The check
that would have caught it is one that restarts the target twenty times and
presses X, because a fault that appears on one restart in three is not a fault a
check that runs once can see.

*Settled, and not by the ordering.* The ordering went on the machine, held for a
year, and came out again. The keyboard opens nothing at start now: it takes the
pad and the keyboard beside it when its surface goes up and hands them back when
it comes down, so it finds whatever is there at the moment somebody wants to
type, and a pad rebuilt under it is a claim taken again on the next turn. There
is no order left to declare, which is why the `After=` is gone rather than kept
as a belt. `console-input-focus` is where that lives, and the twenty-restart
check stayed: it is still the only thing that can see a fault that appears on
one restart in three.

The fork is the other half and it is owed a decision. A binary in `files/`
built from a directory on one laptop is the one thing on this machine that
cannot be rebuilt from what is written down here, and it is holding the button
contract. Its
source comes into this tree, `gamepad.c` comes out of it, and a socket takes
its place, so the daemon holds the pad and says what the presses came to.
Writing a keyboard of our own instead was considered and is not the answer:
uinput cannot reach Thai on a compositor whose only layout is `us`, and Thai is
not optional here.

**Stage 3 is the best line-for-line.** Those four hundred and forty lines hold
the mode switching, they have no tests, and they are where a restart goes wrong.
It can be done before stages 1 and 2 if the device is asking for it, because a
script rewritten as a program that answers `heard` is useful before there is a
runtime to run it in.
