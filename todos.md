# What is still owed

Small work, written down so it is not carried in somebody's head. Anything
that grows past a few lines and a reason belongs in `docs/` or in a check;
anything finished leaves this file rather than gathering a tick.

Say who owns a line if anyone does, and say what would settle it. A line
nobody can act on without the device says so, because the device is one
machine and there is usually somebody holding it.

The headings below are areas rather than states, so that picking one thing up
shows what else is in reach of the same afternoon. What each line is waiting for
it says itself: a thumb, a screen, an argument nobody has had. The two sections
at the end are a direction rather than work and keep their own order.

The device was applied on 2026-09-08 and is standing on 9a20c65, so a line that
opens with `just deploy` has had it and is waiting for the thumb rather than the
apply. The device tier was run that day as well, so a line naming a check has had
that run unless it says otherwise. Say the date again when that stops being
true.

## The pad, the keyboard and the guide

- **The router has not been held.** Every button now arrives at the controller
  daemon as itself and what any of it means is one table there, which is a
  change nothing on a laptop can finish checking: the emulator drives the real
  profile and the whole suite passes against it, and none of it is a thumb. What
  wants pressing on the machine, in this order: A clicks on the desktop and
  takes the row in a menu; the d-pad walks a list, which now arrives as a hat
  rather than as arrow keys; B backs out; X raises the keyboard and X puts it
  away, which is the one press two programs share; L2 with the d-pad still
  moves the screen and the sound; L2 and the bottom right paddle still takes a
  picture. Then open and close a menu half a dozen times and watch that the
  keyboard still comes up afterwards -- that is the pad rebuild this rework was
  for, and its absence is the thing that cannot be seen, only failed to happen.

  R2 and both triggers together are layers with nothing on them. Putting
  something on one from Settings, **Buttons** is the other half of the same
  press, and the guide should grow a section headed **R2** the moment it has
  one row.

- **Two buttons on this device have no name, and the setup screen is what will
  give them one.** InputPlumber reports `Gamepad:Button:QuickAccess2` and
  `Gamepad:Button:RightPaddle3` off this machine's hidraw driver, and nothing
  here knows where on the machine either of them is. Both are routed to a key
  of their own like every other button, so a job can be moved onto either the
  moment somebody finds out what they are, and both are in
  `vocabulary::BUTTONS` under the words InputPlumber uses for them, which is a
  placeholder and reads on a row as *quick access 2* and *right paddle 3*.

  `just deploy`, then Settings, **Buttons**, A on any row, and press whichever
  buttons on the machine you cannot account for. The card says what it was.
  Then the spoken names here become what is written on the device -- or what
  a hand would call it, the way `left-paddle-top` is -- and this line goes.

  While you are there: the same screen is what the rest of this changed. Moving
  a part onto a button another part is on used to be refused, in the profile's
  own words, and on this machine that meant nearly every press. It takes the
  button now, and the part that had it says **no button** on its own row. Press
  A on the menu's row, press Y, and check that the card says two lines: *The
  menu is y*, and under it *what else can be done with this has no button*.
  Then **Put every button back**, which is the first row and asks before it
  does anything, and check that everything is where it was.

- **InputPlumber cannot be asked to hold anything down, and every check about
  the trigger layer is pressed by asking it to.** `SendEvent` on
  `org.shadowblip.Input.CompositeDevice` reaches `blocking_write_send_event` in
  `src/input/composite_device/client.rs`, which calls `blocking_send` on a
  channel from inside a tokio worker; the worker panics with `Cannot block the
  current thread from within a runtime`, the caller waits out the method
  timeout, and nothing is ever emitted. `SendButtonChord` on the same interface
  goes another way entirely and is sound. One line on the device shows it:

      busctl --system call org.shadowblip.InputPlumber \
        /org/shadowblip/InputPlumber/CompositeDevice0 \
        org.shadowblip.Input.CompositeDevice SendEvent sv \
        'Gamepad:Button:DPadRight' b true

  This is not a regression and the version it arrived in is not worth looking
  for. It does the same on 0.78.1 and on 0.79.0, `client.rs` is untouched
  between the two, and 0.79.1 does not go near it -- the device was held back a
  version to prove that and then put forward again, and holding a package to
  make a check pass is not a thing this repository should be doing anyway. What
  it means is that `290` has probably never once done what it says: it was
  written as a held d-pad, the hold went nowhere, and the highlight never moved.

  What is done: everything that only needed a press now presses.
  `Device::press`, `Device::presses` and `Device::keyed` are chords, `290` walks
  a square at a time and `410` sends `Keyboard:` capabilities rather than
  `wtype`, which cannot fire a compositor bind at all. `Device::hold` and
  `Device::release` say they cannot rather than sending something the daemon
  drops.

  A chord also lets go in its own time and the call comes back before it does,
  which is the other half of the same fault and cost `290` a day of being red
  for the wrong reason. Watching the pad's own node while chords are asked for
  says it plainly: the button goes down when the call is made and comes up
  eighty milliseconds later, and eighteen chords fired in a shell loop take
  sixty-six milliseconds and arrive as one press and one release. A walk of
  eighteen squares moved one, or four when the loop ran slower, and nothing
  anywhere reported a dropped press, because none of them was dropped -- the
  button was already down. `Device::presses` waits `LET_GO` between one chord
  and the next, and that number is the daemon's rather than the check's.

  What is not, and cannot be from here: a chord will not press a trigger --
  `The event 'Gamepad:Trigger:LeftTrigger' is not a Button capability` -- and
  L2 held is the whole of how this desktop's second layer is reached. So every
  check under `carry`, `screenshot`, `brightness` and `volume` has no way to
  press what it is about on the device, and its emulator half is the only one
  answering. That is the cost, and it is the reason to write this up upstream
  when somebody has an afternoon: a fix there takes the hold back for free, and
  the entry under this one is the only way to it from this end.

- **A check presses through a call meant for a menu, and the way to the trigger
  layer is a device rather than a better call.** `SendButtonChord` is a mapping
  before it is a method: `chord` and `delayed_chord` are what a profile emits
  when a source button is pressed, and the call on the bus is there so a
  companion menu can make the emulated pad say a combo to whatever is in front
  of it. Its timing says who it is for. `write_chord_events` spawns a task per
  event and adds eighty milliseconds to each, so a chord's buttons go down one
  at a time and come up in the same order reversed, and a single button is a
  press at nought and a release at eighty. That is about what a thumb does,
  which is the point when a game is the one being told. It is only wrong for a
  machine pressing in a loop, which is what `Device::presses` is.

  The half that matters more is what a chord cannot say at all. It is press
  these together and let go, so there is no key-down anywhere in it, and `hold`
  and `release` saying they cannot is not the daemon refusing this desktop --
  it is this desktop asking a tap to be a hold. `SendEvent`, a capability and a
  value, is the call that means what we mean, and it is the one that panics. So
  the wrong call is in use because the right one is broken, which is worth
  knowing before anything else is built on it.

  What would be built on it is a device. InputPlumber's shape is sources in, a
  profile, one target out, and a source is matched by config -- which this
  repository already owns. `50-legion_go.yaml` is in `[files]` and already
  carries this desktop's own argument about which sources to claim and which to
  leave alone; one more entry under `source_devices`, matching a uinput pad by a
  name nothing else has, and a check's press goes through the composite device
  and the loaded profile and out the same target a thumb's does, without the
  hardware and its driver -- which is what a chord skips as well. It would also
  be a pad this end holds down, and a hold is the whole of how the second layer
  is reached, so `carry`, `screenshot`, `brightness` and `volume` stop being
  checks whose emulator half is the only one that answers.

  What it costs is a config on every machine naming a pad that exists only while
  a check runs, and a program in `[build]` to make it and unmake it, with a
  `Lifetime` for the run somebody interrupts -- which is the objection
  `console-test-stages`' own head raises against a second pad, and it is a real
  one. Whichever way it goes, `LET_GO` stays where it is: it is what pressing
  through the front door costs today.

- **The claim is in and nothing has pressed it.** Three entries here asked for
  one crate and it is written: `console-input-focus`, a claim held while a
  surface is up with `EVIOCGRAB` underneath. Two layers -- `devices`, which is
  the only part that opens anything, and `said`, which names one event in one
  vocabulary whatever it arrived on, so a press is `South` whether it came off
  the pad, off the keyboard InputPlumber publishes beside it, or off an input
  method nobody has written yet. Three callers moved onto it: the on-screen
  keyboard, `console-asking` and `console-buttons --identify`.

  What went with it is the point of it. `keyboard.yaml` and `asking.yaml` are
  gone -- both translated nothing and were loaded so one program could have the
  front of the machine, which is not a thing a profile can promise, because any
  program can load one over it. `Mode::profile` is gone with them, and with it
  every part of the daemon that was about a profile load being in flight: the
  spawn, the two tries, and the guess about which load had landed. The pad wears
  the router from login to shutdown, and `controller-profile` takes two words.
  The `After=` on `console-input-keyboard.service` is gone as well, because the
  keyboard opens nothing at start now -- it takes the devices when its surface
  goes up, so there is no race to order around.

  One avenue closed without being tried, and it is worth saying why rather than
  leaving it looking untested. `7c` found that InputPlumber's composite device
  carries a `dbus0` target with an `InputEvent(s,d)` signal, and a reader of it
  would get presses without opening the pad at all. That is the wrong shape for
  this: what these three programs need is not to *see* presses but for nothing
  else to see them, and a signal cannot take a device away from anybody. It
  would still be the better way to *watch* the front of the machine, if
  something here ever wants that.

  One thing was added rather than removed, and it is the piece to look at
  hardest: the router profile now sends the left stick to the pad as well as to
  the pointer. The keyboard walks its highlight with that stick and used to get
  it free from wearing a profile that translated nothing. `the_profiles.rs`
  holds the router to both targets.

  **It is on the device and the checks about it are green**, on 2026-09-04 and
  again on 2026-09-08. The half of this that a machine can answer, it has:

  - `240-the-keyboard-comes-back-with-the-desktop` passed, and it is the one
    worth naming. Twenty restarts, and in each one X raises the keyboard and X
    puts it away -- which is the fault the daemon's `let_go` was written for.
    A button still held when the keyboard took the device its release was owed
    to reads as a repeat on the next press, so X would have raised the keyboard
    once per restart and never twice. It did twice, twenty times.
  - `250-the-keyboard-types-into-a-page` passed, so grabbing the pad and the
    keyboard beside it does not stop what the on-screen keyboard writes from
    reaching a page.
  - `110-the-keyboard` and `210-nothing-has-had-to-be-started-again` passed with
    them: nothing crashed into a restart under the new claim.

  What is left is what needs a hand, and nothing here has had one:

  - Both sticks walk the keyboard's highlight, which is what the router change
    is for, and the d-pad still walks it -- the d-pad now arrives named rather
    than as a hat this program reads. No check presses a stick at a keyboard.
  - The pointer under the keyboard. The left stick moves it as well now, which
    it did not while `keyboard.yaml` was loaded. It is cosmetic and it is new,
    and whether it reads as a bug in the hand is not a thing a laptop can say.
  - `console-asking` binding a button, and binding one on a chord -- the trigger
    is read off the pad's axis under the router now rather than off the one
    thing the asking profile passed through.
  - `console-buttons --identify` naming a button while nothing else is up, and
    saying who has the input when the keyboard is up instead of quietly reading
    an ungrabbed device.
  - Leaving for Game Mode and coming back, which is the one profile switch left
    and the one that destroys the pad under whatever is holding it.

- **The stick scrolls slower.** Slower was a first guess. On the device it is
  either enough, or it wants a curve rather than a speed -- slow near the
  middle and quick at the edge, which is what a stick is for.

- **The paddle on the back scrolls down.** A notch a press, and it keeps
  turning while it is held. Whether the notch is the right size and whether
  holding it runs away are both things a finger says at once and a test cannot.

- **Thai reaches the numbers.** The rule is that the numbers key sits beside
  the language key on every arrangement, and a test holds each of them to it.
  What a test cannot say is whether the thumb finds it there.

- **Per-key latency is not measured, and a line per keystroke is not the way to
  get it.** The keyboard now writes a line when it comes up, when it is asked
  onto the screen and when a layer change compiles a keymap, and none of those
  is the number somebody means by *the keyboard is slow*. That one is per key,
  ten a second while a person types, and writing it the way everything else
  here is written would fill the store with the one surface it says least
  about. It wants sampling -- one key in fifty -- or a mode somebody turns on
  while they are looking. Deciding which is the work; the writing is an
  afternoon.

- **The guide has no idea where it was raised from.** It is read out of the
  one table the daemon obeys, which is what keeps its words true, and the
  sweep in `getting_around.rs` presses every bare button in a place and asks
  the guide about the same button in the same breath. What it cannot ask is
  the first thing a person sees. The tab it opens on is decided now, and it is
  decided by the wrong question: `opens_on` reads which hand was last used and
  answers Anywhere for a pad and Typed for a keyboard, so a guide raised over a
  chooser still opens on Anywhere, where A is a click and R1 is a workspace.
  Both of those are false of the screen it was raised over, and the true answers
  are one tab along under Menus.

  The half that was missing is in: `panel::show` takes the tab to start on and
  the guide passes one. What is still missing is the reading behind it -- the
  daemon knows what is in front on every press and the guide asks nobody.
  Settled when the guide raised with a chooser up opens on Menus, and the sweep
  in that flow asks the guide the way a person reads it rather than ordering the
  sections itself.

- **Touch and buttons, both, everywhere.** The rule this desktop is now held
  to: everything it offers has to be reachable with the screen alone and with
  the pad alone. Neither is a fallback for the other -- it is a machine that is
  held, so a thumb on the glass and a thumb on the d-pad are both the ordinary
  way to use it, and anything only one of them can ask for is a thing half the
  device cannot do.

  The home screen is written to it: A opens, holding A picks up, Y is the card
  that says what is on the screen -- and a tap opens, a held finger picks up, a
  swipe moves a pane, a swipe up is the menu. The rest of the desktop has not
  been read against this rule. The browser add-on's bars were the last surface
  known to be missing half of it and are answered, in their own line above; what
  is left is whatever a sweep turns up, every surface asked twice, once with the
  screen covered and once with the pad unplugged.

- **Nothing else has been asked whether it answers a finger.** There is a way to
  press a place on the screen now -- `console-tap`, and `Device::touch` on top
  of it -- and one check that uses it, on one icon of the bar. Every other
  surface on this desktop is still only asked the questions that were being
  asked of the bar the whole time it could not be pressed: is it there, is it
  the right size, is it on the right layer.

  The ones worth pressing are the ones where a finger is the only way in: the
  rest of the bar's icons, the panels' own rows and tabs, the keyboard's keys,
  the notification cards. And the audit that goes with it is every surface that
  asks for `KeyboardMode::Exclusive` -- correct for a panel that is meant to be
  modal, and a surface that takes the whole screen's input away from everything
  else for one that is not.

## Panels and what they draw

- **The panels are held by one program now, and nothing has measured what that
  bought.** This is the line that used to say the only thing left worth doing
  was to stop exec'ing a process per opening -- `exec` and the toolkit coming
  up were the two largest stretches on every surface, and everything a card
  does was a fraction of either. `console-panels` is that program:
  `crates/console-panels`, one unit, holding the toolkit between openings
  and drawing whichever panel is asked for over a socket in the runtime
  directory. Every panel binary keeps its name and its namespace and is a
  stand-in that takes the screen, asks, and draws the card itself when the host
  is not up.

  Most of what this was waiting for the device has answered, on 2026-09-08.
  `console-response-times --last 200` there has no `gtk` mark left on any
  panel opening -- the settings, the launcher and the music panel all come up
  without one -- and what is left of an opening is `exec` at tens of
  milliseconds and a `surface` beside it. `330-a-panel-opened-again-is-drawn-again`
  and `340-a-closed-panel-is-holding-nothing` both passed there, so a panel
  drawn out of another one's leavings and a watch left running behind a closed
  card are both answered rather than assumed.

  One press is left, and it is the one a laptop cannot make: stop the unit and
  press the menu. It should still open, slower, and say on the journal that it
  drew itself.

  What is honestly not done. Nothing measures the idle cost of the host itself,
  which is the promise the unit's own comment makes: a GTK loop with no surface
  mapped should sit in `poll` and wake for nothing, and RAPL on the device can
  say whether it does. And the window is destroyed and built again on every
  opening rather than kept and hidden, which is deliberate -- a surface that
  survived is a surface holding a reading nobody refreshed -- but it means the
  layer surface and the first frame are still paid per opening. Whether that
  is worth keeping a window for is a question for the file above, after there
  is something in it.

- **What else a row offers is a button beside it now, and no thumb has held
  it.** It is settled as far as a laptop settles anything: the arithmetic is
  `nudged`, the drawing is shot in the nested desktop, and
  `right_off_a_row_stands_on_what_else_it_offers` presses it against the
  viewer's Media page. What a laptop cannot answer is the two questions a hand
  answers. Whether the button is the size a thumb wants at the device's own
  scale, where the card is narrower than it is here and the row lost that much
  of its width to make room. And whether right off a row reads as somewhere to
  go: on the rows that carry a level right has always set the level, and the
  mark at the end of the row is the whole of what says which of the two this
  row is.

  `just deploy`, then Files in the menu. Walk down to a file, press right --
  the row should keep its ground and the **⋯** beside it should take the
  highlight -- then A, and the list of what can be done with that file should
  come up. Left, and B, should both put the highlight back on the row without
  leaving the folder. Then Music, **Playing**: the button there is in the
  corner of the card head, which is a finger's, and Y from the scrub or the
  transport is the thumb's, because those rows spend right on their level.

- **Tap the × on a panel with the keyboard up.** It did nothing, and we know
  why: the panel was a stopped process for as long as the keyboard was up, so
  it could not answer a finger any more than a button. A signal sent to the
  controller's unit reached everything in its control group, and the menu, the
  panel and everything opened from the menu are all in it.

  Nothing is stopped now. `osk-hook` is gone, signals and all: the daemon asks
  the compositor what is in front of it and acts on nothing under the keyboard,
  which is the thing the signal was for. So the panel is a running process with
  the keyboard over it and the × should answer a finger.

  What is left is one tap to confirm it, and it still needs a finger, because
  touch is not InputPlumber's to send. Open a panel, press X, tap the ×.

  The × is also a place along the top now: R1 past the last tab stands on it,
  it takes the highlight while the tab in front goes quiet in mint, and A there
  closes the card. The d-pad steps back off it into the list. That half needs
  no finger, so it is one run of the pad on the device: R1 to the end of the
  strip, one more, then A.

- **A GTK 4 popover has no background.** Seen in gnome-weather before it was
  swapped out: the search popover drew its border and its entry, and the page
  behind it read straight through where its own ground should have been. That
  reproduction is gone with the application, and kweather is Qt, so this needs
  a new one; `console-panel` and `console-files` are both GTK 4 and are the place
  to look for it. The
  palette does define `popover_bg_color`, as a reference to `@panel` like
  every other name, so either libadwaita no longer reads `@define-color` for
  this one or it cannot follow a reference. Worth knowing which, because every
  menu in every GTK 4 application on this machine is the same widget. Settled
  by a GTK 4 application with a menu in the nested desktop, and either a
  custom property beside the named colour or a rule for `popover > contents`.

## The home screen

- **The home screen holds the keyboard, and lets go of it.** The one thing on
  it a laptop cannot answer. The surface takes the keyboard exclusively while
  there is nothing over it, because that is what puts the d-pad on the apps
  with nothing to click first; it lets go the moment a layer opens, and lets go
  before it starts anything itself. What is not proven is the beat between: a
  panel opened from a button asks for the focus as it maps, which is before the
  socket has said it exists. If Hyprland does not hand the focus over when the
  home screen lets go a moment later, a menu opened from the paddle comes up
  and answers nothing. Press the menu paddle from the home screen twenty times
  and watch. Falling back is `KeyboardMode::OnDemand`, which costs a tap on the
  screen before the d-pad moves anything and cannot break a panel.

- **Nothing on the home screen says how to put something on it.** Somebody
  holding the machine could not find how to add an application, how to take one
  off, or how to move one from the first pane to the second. All three work: Y
  on a square is the card that says what goes on the home screen, holding A or
  holding a finger picks an application up, and the next press puts it down --
  on an empty square it moves, on a taken one the two change places, and a
  d-pad off the side while carrying takes it to the next pane.

  So this is not a feature that is missing, it is a feature nothing announces.
  The guide has it, and the guide is behind a button somebody has to know
  about first, which is the same shape as the fault the home screen was written
  to fix. What would settle it is the home screen saying it where it is being
  looked at: an empty square already draws as an offer while the highlight is on
  it, and that is the place a word belongs.

- **What folded onto a later pane has no easy way back, and nothing else about
  the arrangement is quick either.** Narrowing the grid folds the squares that
  no longer fit round onto the end, which is right -- dropping them was a way
  to lose applications. What has no answer is afterwards: grow the grid again
  and they stay where they folded to, with the first pane holding empty room.
  Staying put may even be desired -- a hand-arranged pane should not shuffle
  itself -- so the miss is not the folding, it is that gathering everything
  back together, or onto one pane, is a walk of pick-up-and-carry presses per
  square. Said while resizing on the device on 2026-09-04.

  The same session said the wider thing: moving one square between two others,
  or swapping a handful into a new order, is all holding A and carrying, one
  at a time, and it reads as work. What would settle the first half is one
  press that gathers -- the squares in their reading order, packed from the
  first pane -- offered where the shape is set, so it is a choice and never a
  surprise. The second half wants a shape nobody has designed yet, and it
  should wait for one rather than grow options. Said of the icons rather
  than the names: the names have a shadow behind them and the pictures have
  nothing, so a pale icon on a pale part of a photograph is a shape with no
  edge. The plate under each square helps and does not finish the job, because
  the plate is deliberately thin enough to read the picture through.

  What would settle it is the icon carrying its own edge rather than the square
  carrying more darkness -- a shadow under the picture the way there is one
  under the name, which costs nothing on a dark wallpaper and is what makes a
  light one work. Worth doing at the same time as deciding whether an empty
  square shows a plate at all.

## Music

- **The two music modes and the search have not been pressed on the device.**
  Both halves are settled here as far as a laptop can settle them, and one of
  them is settled differently now. kew took a set of `Shuffle` or `LoopStatus`
  as a press of its own key without ever reading the value, so `player::repeat`
  had to ask first and press as many times as the round of three made it. The
  player this desktop owns reads the value it is set to, once, and the panel's
  counting is now a wrong answer to a question nobody asks -- worth taking out,
  and worth checking on the device first that it has stopped mattering rather
  than assuming it. The reading was pressed too: the whole library once, and the
  second run over it is milliseconds.

  Playing at all was watched on the device with kew and has to be watched again:
  the panel used to press A into silence because kew had never been told where
  the music is, and the fix was a line written into `kewrc`. There is nobody to
  tell now. The player is handed the folder on its command line, the folder is
  read from a file of this desktop's own, and the migration that swept the fork
  carried whatever kewrc said into it. So the first press of A after this
  deploy is the check: it either plays or it is silent for a new reason.

  `just deploy`, then Music in the menu. On **Playing**, take **Play them in
  any order** with something on: the row should turn into *Play them in the
  order they are in* with **any order** beside it, and the song after this one
  should not be the song after it in the folder. Then take **Play this one
  over** and let a song end. Both of them again to put them back, and once more
  from the player left repeating the whole list, which is the state the panel
  offers no way into and has to be able to come out of in one press.

  Y over a song in the folder is still worth a press -- the files panel should
  open on Music, standing on that file. Y over the song *playing* is not, and
  will not be again: it read `xesam:url`, the player does not answer it, and the
  now-playing card offers nothing. That was asked for and is not a regression.

  Then **Music**. Arriving on the tab is what sends the library to be read, and
  the corner says how many songs that is. What is worth measuring is how long
  it takes on the device: it is minutes of ffprobe and it is the one thing here
  a handheld will feel. Press X and type an artist nothing is named after, and
  check the songs by them arrive -- until the reading has finished only the
  filenames answer, which is the difference to watch for.

- **The player is this desktop's now, and no thumb has been near it.** kew is
  gone: `crates/console-music-player` decodes by asking ffmpeg for plain samples
  and hands them to pw-cat, answers the same MPRIS surface the panel already
  spoke, and is built on the device like everything else. The three faults that
  cost a fork of somebody's C are prevented rather than fixed -- there is one
  list and a permutation over it, so unshuffling cannot copy an empty list over
  the library, nothing is freed under the thread answering the bus because
  nothing is freed, and a toggle leaves nothing to be worked out so the press
  after it cannot be swallowed. Each was broken again on purpose and watched go
  red before it went green.

  What a laptop cannot answer is the sound. The whole chain was pressed here --
  a song opened, position moving at the rate of the clock, next walking a
  library of three and coming round, pause, seek, shuffle, repeat, and a song
  reaching its own end and the next one starting -- but this machine is not the
  handheld, and three things are known to be worth a hand on it.

  **Position runs ahead of the speaker.** Samples are counted as they are
  written, and what has been written is not what has been heard: the pipe holds
  what a pipe holds and pw-cat holds its latency. `sounding::AHEAD` takes off a
  bounded guess at that. Watch the scrub bar against the ears on the device and
  see whether the number is right there.

  **Nothing is gapless.** A song ends, the pipe closes, and the next ffmpeg is
  started. Between two tracks of one album that gap is audible and it was not on
  kew. The shape that fixes it is the next song decoding while this one plays,
  which this design allows and does not do.

  **A seek opens the file again.** There is no telling a running ffmpeg to go
  elsewhere, so a scrub ends it and starts another at the offset. Whether that
  reads as a scrub or as a stutter is a question for a thumb on the bar.

  Then the ordinary pass: Music, a song, next five times -- five different
  songs, none twice, and the sixth press comes round to the one you started on.
  `280-a-song-pressed-plays-the-library` asks the whole of it, and it now starts
  `music-player` rather than kew.

- **The now playing screen.** Drawn against a nested desktop and against real
  songs, which is not the same as a record playing in a hand: the sleeve, the
  bar under it and the strip of five want to be reached for rather than looked
  at. An evening of holding it settled six things here, and every one of them
  is a thing a thumb says at once: the card opens with the highlight on play
  rather than a press of down above it; Y opens the files panel on the song
  from any row of the card, where it used to be offered on the title, which is
  a heading nothing can stand on; the words under the sleeve are read rather
  than stood on; the sleeve's square is held from the moment the song changes,
  so the card no longer grows under a thumb when the cover lands; left and
  right move the bar five seconds rather than a twentieth of whatever is
  playing; and the strip says what is switched on in mint alone rather than by
  swapping the mark underneath it. That last one also takes out
  `media-playlist-no-repeat-symbolic`, which is a name neither Adwaita nor
  breeze has -- so if that is what Papirus was drawing as a broken square,
  *the buttons look wrong* is settled with it. Worth one look to say.

  Since then the card is three rows rather than five: the sleeve with the title
  and whose it is beside it, the bar the width of the card with a clock under
  each end, and the strip. Five things down the middle is a card taller than
  the screen it opens on, and what hung off the bottom of it was the strip of
  five -- the one thing a hand came for. So this wants the thumb again, and the
  thing to say is whether the sleeve is still worth its half of the card beside
  the words rather than over them.

- **The transport is reachable only by walking to it.** With the music panel
  open, next, previous, shuffle, repeat and play/pause should each have a
  button of their own: a hand carrying the machine wants the next song without
  reading the screen for where the highlight is.

  Nothing here is against it -- the presses are five MPRIS calls that already
  exist -- and the reason it is not written is that the router is one table of
  what a button means, keyed by what is in front, and *what is in front* is a
  panel rather than which panel. So this is either a meaning the daemon takes
  while the music panel is the surface in front, which wants the door to say
  which panel that is, or it is the media keys the pad already has going
  somewhere that answers them. The second is smaller and is the one to price
  first.

## The wallpaper

- **The wallpaper has been pressed on the device; what it does there is still
  unconfirmed.** Every picture the table names and its still are in
  `/usr/share/backgrounds/console` on the device, so the press itself is
  settled. Everything
  in
  `crates/console-wallpaper` is tested here and one pressed picture has been shown on
  the real panel over ssh, which settled the three things a laptop could not:
  the grade reads correctly against the bar, 2560x1600 through the quarter turn
  needs no resampling, and what a loop costs the wallpaper daemon in memory,
  which is 66 MiB for a picture where a sixteenth moves and 697 MiB for one
  where all of it does. What is left needs a deploy. `just deploy`, then
  `sky-press`, which fetches about two hundred megabytes and takes six minutes;
  then three things a thumb has to confirm.

  Open a window, and check the picture stops moving. Then open the settings and
  check it stops for that too: a menu is a layer surface rather than a window,
  and counting it is new. `console-sky` hands the daemon the still whenever the
  workspace holds a window or anything but the wallpaper and the bar is up, and
  the readings it takes are `hyprctl activeworkspace -j` and `hyprctl layers
  -j`. Close them and the movement should come back within a frame or two.

  While a window is open, `ps -o rss= -C awww-daemon` should read about 66 MiB
  whatever picture is up. That is the whole argument for pausing, and it is the
  one measurement that has only ever been taken by switching pictures by hand.

  Then the Wallpaper tab: turn following the weather off, pick a picture, and
  check it arrives at once rather than in five minutes. It is `console-sky
  --now` that makes the difference, and it is run from the panel. Three things
  to watch, because all three are new. The picture, or its still at least,
  should be up in about a second: a pinned picture no longer waits on the
  weather service. The corner should say so while it happens. And the panel
  should answer the d-pad through the whole of it, because the pass is handed
  to `Showing::later` now rather than waited for where the panel is drawn.

  Then drop something into `~/Pictures/Wallpapers` and take it up from the same
  tab. That press runs on the device and takes tens of seconds a picture, so
  the thing to watch is that the panel keeps answering the buttons while it
  happens, and that the corner says how long it is going to be.

## Files, downloads and what opens what

- **A kind of thing is a family of types now, and only Music has been given a
  default to fall back on.** The device says the fault plainly. Each of the
  three settings took effect for exactly one type and the rest of each family
  scattered:

  | | set to | and the rest of the family |
  | --- | --- | --- | --- |
  | Music | mp3 opens in the music panel | flac and ogg opened in Firefox, opus was claimed by nothing at all |
  | Pictures | png opens in Gwenview | jpeg, webp and gif open in Chromium |
  | Video | mp4 opens in mpv | webm opens in Firefox, mkv is claimed by nothing |

  The panel writes the whole family now, so what is owed is one press each.
  After a deploy: Settings, **Configuration**, **Pictures**, choose Gwenview
  again -- the same answer that is already on the row -- and jpeg, webp, gif
  and the rest follow it. Then **Video**, choose mpv again, and webm and mkv
  follow. Music needs no press at all, because this desktop ships its own
  answer for it in `/etc/xdg/mimeapps.list`; the other two point at programs
  that are not ours and this tree should not be choosing them.

  Then the press that started it: a `.opus` file in the files panel, opened.
  It should be the music panel and not a browser window with a scrubber in it.
  `.flac` and `.ogg` are the same question and were the same fault.

  Worth knowing while in there: nothing on this machine runs
  `update-desktop-database` after an apply, so `mimeinfo.cache` is whatever it
  was. It does not matter for any of the above -- a named default in
  `mimeapps.list` beats the cache -- and it is why a browser was winning types
  nobody had ever chosen it for.

- **Nothing on the Download panel has been pressed on the device.** The card
  opens and draws its list in the nested desktop here, and both fetches have
  been run on a laptop: a song arrives in Music as an opus with its cover and
  its title inside it, and a film arrives in Videos as an mkv with the picture
  attached. What no machine here can answer is the half that is a thumb.

  `just deploy`, then Download in the menu. Type a song with X, press A to walk
  off the line onto **Look for**, and press A again: the row should say
  **Looking for** it and the list should arrive with pictures. Press A on one,
  and the corner should say it is on its way into Music; a notification should
  say it has landed, and the row should say **have it** the next time that
  search is made. Then Y on a row, which should offer the video of the same
  thing and the browser.

  The one thing worth measuring while it is open is how long a search takes over
  the device's own network. Ten pictures are ten curls and ten ffmpegs here; if
  that is slow enough to notice on the handheld, fetching them at the same time
  rather than one after another is where to start.

## The browser

- **Nothing the browser's add-on does has been pressed on the device.** What a
  laptop can settle is settled: the archive `console-web` writes is a zip a
  reader opens and finds every file `source::FILES` names in, the version in it
  is read back out
  of the packed bytes, the labels are prefix-free at every size and never
  longer than three presses until a page has more than sixty things on it, and
  no file in the add-on holds a colour of its own. None of that has been near a
  browser.

  `just deploy`, then start the browser -- and if it was already running, close
  it first, because a policy is read when a browser starts. **Console** should
  be listed on `about:addons` after that start. If it is not, the answer is in the Browser Console and it is almost certainly
  the signature: LibreWolf is built to install an add-on nobody has signed and
  `user.js` asks it to, and neither half of that has been watched here.

  Then a page with links on it. Press Y: every link should wear a label of
  arrows and a bar should appear along the bottom. Press the arrows written on
  one, and the page should follow it. Y twice, and the bar should say the
  labels open a new tab instead; take one and the page should say afterwards
  that it went behind this one. Then the d-pad with no labels up: a pink box
  should walk from link to link, A should take the one it is standing on, and
  pushing the stick should take the box away and give the pointer back. Then B,
  which should go back a page -- that is the promise this was written for and
  the first press worth making.

  Then the bar itself. **Look for something**, X, type, and take the row at the
  top: it should search in whatever engine the settings panel's Web tab last
  chose, which is the one thing here that is read off the browser rather than
  written down. **On this page** should count what it found and the d-pad
  should walk between the matches. **The tabs** should list this window's tabs
  with the one you are on in mint. And a new tab should open with the line to
  type into already there and already holding the keys, which is the surface
  nothing on a laptop has drawn at all.

  Two that are answers rather than presses. A finger held on a page should
  still get Firefox's own menu, because the pad is a mouse and a finger is a
  touch and the add-on tells them apart by what the event says about itself. A
  page inside a page -- a comment box, an embedded player -- should have no
  labels on it at all, which is the top-frame limit written down in
  `docs/browser.md` rather than a fault. If either of those is wrong on the
  device, it is the sentence in that file that is wrong.

- **The add-on's bars were the one surface here a finger could not touch, and
  it was the other thumb that was missing.** This entry used to say the
  opposite -- that the bar was reached with a thumb and the pointer and not
  with the d-pad -- and it was read against the code rather than assumed. The
  pad reaches all of it and has since 9f1e546 and 5a142e8: every deed on the
  bar carries a prefix-free label out of the same pool the page's links do and
  `typed()` takes it, and a card walks with up and down, takes on A, and backs
  out on B. It is the glass that had nothing.

  **The literal ask above is unbuildable, and that is worth writing down so the
  next person does not reach for it.** It wanted a highlight walked along the
  bar with the d-pad, the way the pink box walks links. But the bar exists only
  while the labels are up, and while they are up an arrow press *is* a label
  keystroke -- it either finishes a label or narrows the list. One press cannot
  both narrow the labels and move a highlight, so walking the bar means giving
  up the labels, which is the quick thing this was written for. The two are
  alternatives, not layers.

  What was actually owed is the rule in **Touch and buttons, both, everywhere**,
  applied to the two surfaces that had missed it. The labels bar could be put
  away with B or by taking a deed, and not by simply deciding against them; the
  bar the find draws counted its matches and let only the d-pad step between
  them. A finger had no answer to either. Both now carry a **×** at the right
  end, in the same place the card's is, and the find's carries **↑** and **↓**
  beside the count.

  Not read off the listeners. The host is `pointer-events: none` and only the
  things a finger is offered take it back, so a control that draws and does not
  claim the point is drawn and dead -- which is the fault this repository keeps
  meeting in other languages. `~/.cache/console-shot/find` puts each surface up
  headless, takes a point in the middle of every control and asks
  `elementFromPoint` whether it lands on the add-on or falls through to the
  page. All four new presses answer, at 68x40, the height the deeds already
  are.

  Two things that harness is worth keeping for, because both were quietly
  wrong. `--screenshot` does not run extensions in this build, so no picture
  taken that way has ever had the add-on in it; the camera has to be inside the
  add-on. And a palette copied out of the tree is not the palette that ships --
  `source::hosted` rewrites `:root` to `:host, :root`, because inside a shadow
  root `:root` matches nothing -- so a harness that copies it plainly draws
  every surface with no colour at all and looks like a stylesheet that is not
  loading. The copy under `~/.cache/console-shot/sh` still has that fault.

  What is left is a thumb, and one decision. The thumb: on the device, tap the
  × on the labels bar, and with a find up tap ↑ and ↓ and then its ×. The
  decision: the bar's own ground still passes a tap through to the page
  underneath it, because `.bar` never claims the point and only the controls
  on it do. That is either right -- the bar is a thing to read and the page
  goes on taking what is not aimed at one of its presses -- or it is a panel
  covering something that answers taps meant for it. Nobody has taken it
  deliberately either way.

## Voice

- **Dictation: three fixes are in and none of them has been near a
  microphone, and the comparison that decides the model has not been run.**
  This is a morning of work on the device and nothing here can advance it.

  What landed, all of it settled as far as a laptop can settle it:

  - The language is chosen rather than guessed. Settings, **Configuration**,
    **Dictation**, and the choice is a `dictation` line in
    `~/.config/console/defaults` that `dictate` reads on the next press.
    Detection is a guess made on what was said, and one word is not enough to
    guess from -- English is what it falls back to.
  - Thai keeps the marks it is written with. They are nonspacing marks, which
    to anything asking whether a character is a letter is the same answer a
    comma gets, so every short Thai thing was being typed with its vowels
    taken out and the rest broken into pieces.
  - Dutch keeps the apostrophe on `'s`, `'t` and `'n`, which the rule that
    protects `don't` could not see, because it looks for a letter on both
    sides and those have one only on the right.

  `voice-compare` is the measurement, and it runs on the device as the person
  whose session it is -- not as root over ssh, because the microphone belongs
  to a PipeWire that belongs to a session. It is a crate now rather than a
  script sent over, so it is built on the device out of what a deploy pushed
  there, like everything else a person touches. The copy of the old script left
  in somebody's home knows six clips; it is not this.

      ssh <the user>@$CONSOLE_HOST
      cd /etc/console
      cargo run --bin voice-compare -- --record     # sixteen clips, one at a time
      cargo run --bin voice-compare                 # every clip through every model

  The fetching and the building are done: `~/.local/share/console/voice` on the
  device holds `whisper-cli` and all four models -- the turbo it runs today,
  turbo unquantised, the full `ggml-large-v3.bin` and Qwen3-ASR with its mmproj
  -- so `--models` and `--build` have nothing left to do. What is left is a
  voice, which is the half nothing here can supply.

  What decides it is the Thai, read by somebody who speaks Thai. Turbo is
  large-v3 with the decoder cut from thirty-two layers to four, and OpenAI
  names Thai as one of the languages that costs; meanwhile the saving is in
  the decoder, which on a two-second sentence is a hundredth of the work on
  this machine. So the model in use may be paying for Thai and getting nothing
  back. If the full one costs little and hears Thai better it is the one to
  keep, and the line in `docs/voice.md` calling turbo both the accurate one
  and the fast one needs a clause about which language it is not.

  Three things the recording answers that nothing here can:

  - `th-tones` -- one syllable at three tones, with gaps. If the models
    differ anywhere, they differ here.
  - `nl-mixed` -- Dutch with the English words left in, said the way it is
    really said. This is what pinning the language *costs*, and it is the one
    result that could argue for leaving the setting on *Whichever is spoken*.
  - Whether whisper writes `'s ochtends` with the apostrophe at all. The fix
    is right either way -- it is a no-op if whisper never emits one -- but
    nobody has seen its Dutch output.

  Also there while the directory is open, and still there on 2026-09-08:
  `ggml-small-q5_1.bin`, 190 MB, left over from the few hours the small model
  was in use. Nothing reads it.

## The bar, and the row under it

- **Nobody has looked at the bell while notifications are held back.** The
  notices panel's last row keeps cards off the screen, and while it is on the
  bar's bell wears `md-bell_off` rather than the outline.

  Most of this is settled. The codepoint is the one the font's own table gives
  for that name, drawn and checked rather than counted along the alphabetical
  run that turned the other two into a bunk bed and a glass of beer; the device
  answers `fc-list :charset=f009b` with the same MesloLGS Nerd Font that draws
  the two already on the bar; and the bar was seen to print the struck-through
  bell with a count beside it on the machine, with the mode on. What is left is
  a look at it.

  Take **Keep them off the screen**, and check the icon is a bell with a line
  through it. Then take **Let them back on the screen** and check it goes back.

- **Nothing asks whether the strip under the bar actually paints.** Every other
  surface has a check that it draws: the wallpaper, the panel, the keyboard, the
  files, the notices, the download panel. The strip has none, and that is how it
  shipped a release filling to nothing.

  What hid it is the part worth keeping, because the next thing will hide the
  same way. A GTK stylesheet that names a colour nobody defined does not fail:
  the declaration is dropped and everything else carries on, so the file parses,
  the widget lays out, waybar exits 0 and the journal is empty. Asked three ways
  the machine said it worked -- the layer is there at the right size,
  `bar-updating` reports the right class, the stylesheet has a rule for every
  step -- and two tests already held the last of those from both ends. Every one
  of those answers is about the plumbing. None of them is about a pixel.

  So the check to write is the one that looks: put a number in
  `/run/console/updating`, wake waybar, and read the screen back. The strip is at
  a known offset and its colour is `fill` against the bar's ground, so what is
  asserted is that the row is filled from the left to about the proportion asked
  for. No amount of correct JSON passes that.

  Two things to know before writing it. `grim` captures at the panel's own
  resolution while `hyprctl` answers in logical pixels, so a row worked out from
  the layer geometry is off by the scale unless it is multiplied -- reading the
  wrong row looks exactly like a strip that does not paint. And waybar reads its
  stylesheet once at startup: the signal the engine sends re-runs the module but
  does not re-read the CSS, so a check that changes a colour has to reload the
  bar before believing what it sees.

  `every_name_the_desktop_asks_for_is_defined` now holds the colour crossing on
  the laptop, which stops this exact fault. It does not stop the next thing that
  parses and does not paint.

- **The strip is a general row that only one thing knows how to use.** It is
  already always there -- a layer that reserves its row for the life of the
  session, invisible while there is nothing to say because an empty text is a
  module waybar hides and what shows through is the bar's own background. That
  it stays is settled and should not be revisited: a row that came and went
  would reflow everything under it twice per use, and a row that is always
  there is a place anything can draw a length.

  What is apply-shaped about it is only the naming and the path. The file is
  `updating`, the module is `bar-updating`, the layer is `updating`. Nothing
  about a bar that fills from the left is about applies, and there are other
  lengths this machine knows and currently says with a card that covers
  something: a long copy, a download, a scan.

  The second caller arrived without the renaming: `console-test-stages`'
  `watching` fills the same row while a device run walks the checks, through
  `console_notifications::updating` like the engine does. So the protocol is
  already shared and only the words are not -- a row that says *updating* while
  something else entirely is being watched is the fault this entry was written
  before. What would settle it is a name and a protocol that are not an apply's:
  one place a length and a word are written, one module that draws whatever is
  there, and the apply as the first caller rather than the only one.

  The callers are already written and are not hypothetical. The volume, the
  brightness and the battery each raise a notice carrying a value, and mako
  fills the card to that proportion in the same colour the strip fills in -- so
  the card *is* the bar today, and it is a bar that covers whatever is under it
  for as long as it stands. Moving those to the strip is the whole argument for
  generalising it: the same reading, in a row that covers nothing.

  Instead of the cards rather than beside them: a rocker stops raising a
  notification and the strip is the whole of what it says. Which means the
  figure goes -- the strip is a length and nothing else, there being no room in
  it for a word, which is why its text is a space. That is the intent and not an
  oversight: a row filled four tenths of the way across the screen is the
  reading, and `Volume 40%` written out is the same thing said twice. Worth
  knowing it is a decision, because it cannot be had both ways without putting
  back the card that covers something.

  Two things left to settle, and neither is the drawing.

  *Going away by itself.* A rocker's reading is worth about a second and a half
  and the strip has no notion of that: what ends an apply's fill is the engine
  deleting the file. A length that expires is a new idea in it.

  *Two at once.* A rocker held down fires many times a second, and somebody can
  reach for the volume during an apply. One row cannot show both, and the honest
  answers are a queue or the most recent winning -- worth deciding before there
  is a second caller rather than after.

- **An apply rewrites the bar underneath the thing reporting the apply.** The
  strip is drawn by waybar out of files the apply is in the middle of replacing,
  and waybar reads its stylesheet at startup, so the one surface whose whole job
  is to be watched across an update is also a surface the update invalidates
  half way through. A palette written during an apply is not the palette the
  strip is drawing with until something restarts the bar.

  The asked-for shape is that everything is built first and swapped at the end,
  with whatever draws the progress swapped last of all -- so the bar being
  watched is one process from the beginning of the apply to the end, and the
  update to itself is the final item rather than an interruption in the middle.
  The build already happens before anything is put in place; what is not ordered
  is the putting-in-place, and `laying.rs` already has a notion of a swap in
  progress to build that ordering on.

## Bluetooth

- **Nothing has yet been paired from the tab.** The Bluetooth tab offered
  `connect` on every row and never `pair`, so a keyboard nobody had bonded was
  the one device the tab could not do anything with -- the first one on this
  machine was introduced over ssh instead, by hand. A stranger's row now opens a
  page of its own, More Options opens the same page for a device already known,
  and `console-bluetooth introduce` is the pairing, the trusting and the
  connecting in one press because a row runs one program. All of it is unit
  tested and none of it has met a radio.

  What settles it is a device that has never been seen from this machine, put
  into pairing mode and taken all the way from the row to typing, and then a
  reboot: the trusting is the half nothing on the screen shows, and a keyboard
  that comes back a stranger is how it would say it did not happen. Forgetting
  wants pressing too, and wants the row to be gone afterwards rather than
  standing there saying Connect.

- **A device found by looking can be gone before it is pressed.** "Look for
  devices" is `bluetoothctl --timeout 8 scan on`, and the list under it is
  `bluetoothctl devices`, which is bluez's own cache: when the scan ends bluez
  drops the unpaired devices it can no longer hear, and the row a person was
  reaching for goes with them. Watching it happen took three scans to catch one
  keyboard that was advertising the whole time.

  Eight seconds is the number to argue with, and the better shape is probably
  discovery that lasts as long as the tab is open rather than as long as a
  press. Both want the device, because what decides it is how long a real
  keyboard sits quiet between advertisements.

## Power, heat and Game Mode

- **The way into Game Mode is a road the boost does not watch.**
  `console-cpu-boost` says of itself that a daemon which does not reach `settle`
  leaves every core at `balance_performance` with the only copy of what was
  there gone, and that a later run cannot tell that wreckage from a machine
  whose profile genuinely asks for that word. The roads it watches are a crash,
  the target stopping, an apply restarting it and a battery that ran out. The
  session switch is not one of them. `stick-scroll` holds the only `Hurrying`
  and does not run in Game Mode, so whether the hint goes back before Steam has
  the screen depends entirely on how that daemon is ended, and nobody has
  looked. The note in the runtime directory is what should catch it on the way
  back -- a session switch is not a reboot and the directory survives one --
  which is the half that makes this worth pressing rather than assuming either
  way.

  Scroll a stick on the desktop, hold the left Legion button, and read the hint
  under `/sys/devices/system/cpu` in Game Mode. It should say whatever the
  profile asks for and not `balance_performance`; come back, and the note should
  be gone rather than waiting.

- **Consistency is the thing to hold, and nothing here has ever been asked to
  hold anything.** A handheld that reaches a high frame rate and falls off it is
  worse to play than one that never reached it, because a person feels the fall
  and does not feel the ceiling. The hand holding it says the same: a device
  that is warm and stays warm is comfortable, and one that swings is not. So
  what a power profile promises should be a number this machine can keep for an
  hour, and the entries under this one are what that would take.

  The argument is already in the tree once, in the other direction and about a
  different subject. `console-cpu-boost` raises the hint for about as long as an
  opening takes, because the whole of a panel opening is over before load-based
  scaling has noticed the load it made, and that is the right answer where the
  work is a moment and the machine is asleep on either side of it. Sustained
  work wants the opposite answer and the two do not contradict each other:
  hurry when nothing is sustained, hold when something is. What is missing is
  anything at all that tells those two cases apart.

  The choice stays where it already is. Three power profiles, the same three
  whatever `powerprofilesctl` says, and no fourth switch asking somebody to opt
  into the machine behaving well -- a list of options is a way of making a
  person responsible for arithmetic they cannot do from where they are sitting.
  Somebody picks how much machine they want; making that a number the machine
  can keep is not a preference. `More than one machine` reaches the same shape
  from the other end, where each profile declares its power answers and a check
  reads them back on whatever machine it is standing on.

- **Nothing on this machine reads a temperature.** Not a crate, not a doc, not a
  line of the manifest. Heat is the one hardware fact this desktop has never
  asked for, and every decision above wants it.

  It is worth being clear that this is not a tidying job. The sysfs this tree
  already reads has one owner each and they have not drifted: the hint under
  `/sys/devices/system/cpu` is `console-cpu-boost`, the backlight and
  `/sys/power` are `console-settings`, the batteries are
  `console-default-applications`, the pad's own devices are
  `controller-profile`. There is no second reading to go and collapse. There is
  a subject nobody has written.

  Which also settles what it must not become. A general layer over the operating
  system -- a `console-sysfs` every caller depends on, growing a name each time
  something new is read -- is the shelf `docs/crates.md` argues against at
  length: the general crate reads the shape, the owning crate spells the words,
  and the shape of sysfs is a file holding a word, which
  `console_core_atomic_writes::read` already is, absence separated from
  unreadable. So this is named for what it does rather than for where it reads.
  How warm the machine is, how much of its envelope has been spent, and what it
  could hold from here -- and it reads whatever it has to in order to say that.

  RAPL and the amdgpu hwmon both answer on this device and were measured on it;
  there is no `powertop` or `turbostat` here to lean on. Which of the two says
  what, and what a reading is worth on a machine whose fan curve nothing can
  read back, is the first thing to write down rather than the first thing to
  code.

  What settles it: a number, on the device, that moves when a game does.

- **The boost hurries with no idea what the machine is already doing.**
  `Hurrying` has one caller -- `stick-scroll`, in the daemon that reads the pad
  -- and it raises the hint on every core for three quarters of a second per
  press and puts it back. On an idle desktop that is the whole point of it. In
  the desktop session with a game in front, it is a person's own thumb on the
  stick making the clock swing underneath the thing they are playing, which is
  the boost and the fall this desktop should be the one preventing.

  The signal that tells the cases apart is already read, and already in this
  desktop's own words rather than the compositor's. `console-compositor` reads a
  window's `fullscreen` into `Filling`, and `fullscreen>>` arrives as
  `Stirred::WindowFilled`, so a window that has taken the screen is an event
  this tree already hears and has never asked a question about. The boost asking
  before it hurries is the second caller the reading wants, and the first one
  somebody would feel.

  What settles it: the same stick, in the same game, with the hint watched.

- **A game is a window, and Game Mode is one road to it rather than what a game
  is.** `console-session` is right that the machine leaves entirely for Steam's
  session, and it is easy to read that as games being somebody else's business
  the moment the switch is made. They are not. Steam is an application that is
  installed here; it runs in the desktop session like anything else does, and
  when it does, this tree is the compositor, holds the pad and draws its panels
  over it. Every question above belongs there, because there is where this
  desktop is the one deciding.

  Which leaves an inventory question standing. No `[packages]` line names Steam,
  and the manifest mentions it only where something else is carried along for
  it, while `console-applications` walks the Steam library roots to find what is
  installed and what its icons are. So the desktop already knows about the games
  on this machine and the manifest has never said the word. One of those two is
  wrong about what is on the device, and it is not the one doing the walking.

## The manifest, an apply and a deploy

- **The disk's room answers on the device, and only the green half of it has
  ever been read.** `console room` says what is left and what an apply wants of
  it, `console apply` refuses below that line, and `console well` cards it
  before the refusal would arrive. Asked over ssh on 2026-09-08 the device says
  *1735 GB left, and an apply wants 6 GB of it*, so the engine standing there
  knows the question and the arithmetic is the machine's own.

  What no green answer can give is the sentence when there is no room, on a
  device with most of a terabyte free. Point the arithmetic at a small
  filesystem to see it -- `console room --root` somewhere on a tmpfs -- and read
  the walk while it happens, because naming where the room went is a `du` over
  the person's home and it takes over a minute on this laptop. If it is a minute
  on the device as well, the hourly card wants a cheaper answer than the true
  one.

- **The standing-up-to-a-fault work is deployed, and only the readings under it
  have been taken.** All of it is laptop-settled -- the suite is green and
  clippy is clean -- and all of it is on the device. Every one of them is about
  what happens when something goes wrong on a machine that is not here, so what
  a reading can answer is answered below and what is left in each is the
  breaking:

  **The restart drop-in.** Half of this is answered: asked on 2026-09-08 the
  device says `StartLimitIntervalUSec=0` and names
  `console-.service.d/restarting.conf` among the drop-ins it read, so systemd
  there reads the prefix file the way systemd here does and the units did not
  quietly keep the old limit. What is left is the behaviour rather than the
  reading -- break one on purpose, point `console-bar.service` at a program that
  exits at once, and watch it go on retrying past five falls, at a widening
  interval, instead of going `failed` and staying there.

  **`console well`.** It runs three minutes after the desktop comes up and
  hourly after. On a machine with nothing wrong it must say nothing at all: a
  card at every boot is a card nobody reads. Then give it something to find --
  edit a file the manifest claims and do not apply -- and check the card names
  it and says `console check`.

  **The apply's two new guards.** `console apply` should refuse on a battery
  below the protect step plus fifteen and say so in a sentence with the reading,
  the level and *plug it in*. And while one is running, `systemd-inhibit --list`
  should show `console apply` holding `shutdown:sleep:idle`; kill the apply and
  the lock should be gone with it, because the pipe closes when the process
  does. A lock left behind is a device that has quietly stopped being able to
  suspend, so this is the one to actually watch.

  **The processors.** Press something, and inside the three-quarters of a second
  after it `$XDG_RUNTIME_DIR/console/hurried` should exist and name every core.
  Then the real test: press something and kill the controller daemon inside that
  window. Every core is left at `balance_performance`; the daemon restarts; the
  note is what puts them back. `cat /sys/devices/system/cpu/cpu0/cpufreq/energy_performance_preference`
  should read `power` again a moment later. Before this it read
  `balance_performance` until a reboot.

  **The plan on disk.** An apply now writes `/var/lib/console/laying` before the
  first rename and removes it when the release stands up. Watch it appear and go
  during an ordinary apply. Then the case it is for, which needs nerve: pull the
  power during the swap. The machine should come up and `console well` should
  say an apply stopped partway through and name the files that were in flight.

- **The hourly card is quiet about the file it was loud about, and loud about
  another.** The `/etc/sudoers.d` reading it could not take is answered: nothing
  in the card names it now, and `console check`, typed, should still name that
  file and call it *cannot read*. What it says instead, asked on the device on
  2026-09-08, is *Changed since the last update:
  /home/@user@/.librewolf/profiles.ini* -- every hour, about a file the browser
  writes itself the way `console-scale` writes `bar.css`. Either that path wears
  `theirs` or the card goes on telling somebody about drift they did not cause
  and cannot act on, which is the entry further down about who this check is
  for, arriving as a live example.

- **A file a package owns is a regression that comes back on a schedule nobody
  chose.** Pacman owns `/usr/share/inputplumber/devices/50-legion_go.yaml` and
  takes it back at every upgrade of that package. The manifest names the file
  and has no answer but an apply somebody thought to run, so the touchpad goes
  to `blocked: true` again on a morning chosen by whoever cut the release. It
  will not be the last one: anything under `/usr/share` that `[files]` carries
  is the same arrangement waiting for its own upgrade.

  The half of this that is done is the opposite question. A path in `[files]`
  can now carry the word `theirs`, which says the tree ships what the file
  starts as and nothing after -- installed when it is not there, never compared,
  and not swept back by a `console save` that was not given its name.
  `bar.css` and `zz-steamos-autologin.conf` wear it, because
  `console-scale apply` and `steamos-session-select` write them and are supposed
  to, and the card had been naming both on every boot in the same sentence and
  the same colour as the inputplumber regression that cost a morning.
  `50-legion_go.yaml` is deliberately not marked: a package taking a file back
  is the thing to be told about.

  What would settle this one is the manifest holding a package to what it
  reinstalls. `[packages]` and `[files]` know nothing of each other, and the
  arrangement wants either a hook that reapplies a path pacman has just written
  or an apply that says which of its files a package now claims. Both are more
  than a mark on a line, which is why this waits.

  Also unpressed: the card itself. That two files stopped being named is
  arithmetic and is tested here; what a person sees on the device is one look
  after the next apply.

- **A file dropped from the manifest stays on the device.** `console apply`
  writes what the manifest names and never asks what it wrote last time, so a
  path that leaves the manifest goes on living where it was installed. Tonight
  that was the file pinning which session logs in: it had stopped being ours,
  it still sorted after the one the switcher writes, and it quietly overruled
  every attempt to leave for Game Mode. Removed by hand. Settled by the apply
  keeping a record of what it laid down and taking away what the manifest no
  longer claims.

  Sighted again on 31 August: `/etc/inputplumber/profiles/menu.yaml`, in no
  manifest and named by no word `controller-profile` takes. That one is gone
  from the device now, taken by a hand rather than by anything here, which is
  the shape of the fault rather than the end of it. `asking.yaml` is the same
  file's replacement: the claim crate took it out of this tree, the device still
  holds it, and it lies in exactly the voice the last one did -- anybody reading
  `/etc` to find out what happens while a program is asking a question is told
  about a profile nothing loads any more.

  **A record now exists and it is not this one.** 406ec0e gives an apply a list
  of what it laid down, so a release that will not run can be put back. It was
  tempting to settle this entry with the same list, and it does not: that record
  lives for the length of one apply and is swept at the start of the next,
  because its whole job is undoing the run it belongs to. What this wants is the
  opposite -- what some earlier apply laid down that this one no longer claims
  -- which has to outlive every apply and be kept on the device across them.

  Two mechanisms, and the second one deletes things. Building it inside a rework
  that was already changing what "landed" means is how you get the one release
  where nobody can tell whether a deletion was meant, so it was left out on
  purpose rather than missed. The sweep that clears a half-finished apply has
  the same edge and the same answer: it looks beside every file the manifest
  claims, so a leftover beside a file the manifest has stopped claiming stays
  too.

- **Only an apply asks whether there is room, and everything else that writes
  still does not.** The machine builds what it runs, so an apply is the longest
  write on the device and the one that lands on the same disk as the games, the
  videos and whatever the download panel last fetched. `room` in the engine is
  what stands in front of it: how much is left is one cheap reading taken every
  time, and below what a build wants the apply refuses before it starts, in a
  sentence that says the size, what it wanted and where the room went. `console
  well` asks the same arithmetic with the evening added to the apply, so the
  card arrives while there is still something to do about it rather than on the
  evening an update will not run, and `console-deploy` asks the device that
  question over ssh before it spends `just ready` here. The policy is a size and
  not a share of the disk, because a tenth of this device is room for twenty
  builds and a tenth of a smaller one is room for none, and it lives in the
  engine alone so the deploy and the apply are asking one question rather than
  two that agree today.

  Where the room went is measured and not guessed, and it costs a walk over
  every file in the person's home -- over a minute on this laptop. So it is only
  walked once the cheap reading has already come back short, which is the moment
  somebody has to be told where to look, and between the two lines the card says
  the size and names nothing.

  What is still asked by nobody is every other write. A download that ends in a
  file that will not play, a thumbnail that is half a picture, a panel that
  opens having forgotten what it remembered: none of them asks first, and
  `console_core_atomic_writes` only means the old file survives the attempt.
  Each of those is small enough that running out reads as the feature being
  broken, which is the argument for asking rather than against.

  And the sweep is still a decision nobody has made. `target/` on the device is
  rebuilt by the next apply and is the obvious thing to take away, but an apply
  that has to compile the workspace from nothing is a long one, and nobody has
  measured which of the two costs more. What is known is that the handheld is
  nowhere near the line: asked on 2026-09-07, most of its disk is free and what
  has been spent is the games, with the checkout's `target/` small enough beside
  them to be a rounding error. The urgency this entry was written with was
  borrowed from the laptop it was written on.

- **The build is one stretch that moves on a curve, and could be a stretch that
  moves on the truth.** Building is most of an apply. It already moves the strip
  per crate rather than once at the end -- `building.rs` reads cargo's own
  `Compiling` lines -- but it moves along `steps / (steps + PACE)`, which is a
  curve that never arrives, because there is no honest total to divide by:
  how many crates a build compiles depends on what changed, and the apply that
  matters is the one after somebody edited one file. The front-loading is
  deliberate rather than a guess badly made: the question early on is whether
  anything is happening at all, and a bar that has not moved cannot answer it.

  The wish is for the same thing the device tier now has: the longest work
  first, so an apply feels like it starts slow and then runs. Two halves, and
  only one of them is easy.

  The order is the hard half. Everything is built by one `cargo build` with a
  `--bin` per program, and inside that cargo owns the schedule: it walks its own
  graph and runs what is ready, in parallel. Taking the order over means one
  cargo invocation per program, which serialises what is currently parallel and
  makes the apply genuinely longer to make it feel shorter. Worth trying first
  is the cheap version: order the `--bin` arguments longest first and leave the
  scheduling alone. Cargo takes ready units roughly in the order it was asked
  for them, so the heavy targets start early at no cost at all -- which is the
  classic longest-processing-time-first heuristic, and it usually shortens the
  wall clock rather than lengthening it.

  The measurement is the other half, and neither half works without it. Nothing
  knows which program is the long one. Per-crate times cannot be read off the
  `Compiling` lines, because a parallel build starts six at once and finishes
  them in another order. `cargo build --timings` knows, and writes a report;
  whether an apply can afford to ask for one, and where the answer would be
  kept, is the question to settle before any of the above is written.
  `lasting` is the shape it would take, and the store would live beside
  `checked` for the same reason.

- **Deploying from a tree somebody else is working in should be a flag, not a
  recipe.** Several sessions share this checkout, so a tree with somebody's
  uncommitted work in it is the ordinary state rather than the exception, and
  every deploy from one is a handful of manual steps done from memory.

  The refusal itself is right and should stay. The push sends committed history
  and never the working tree, so what is deployed is already a copy of the
  branch -- but `just ready` runs *in the tree*, so a deploy from a dirty one
  would have the suite vouching for something other than what ships. That is the
  fault being prevented, and it is worth preventing.

  What is missing is that the way around it is manual. `console-deploy` prints
  the recipe when it refuses -- clone the history somewhere nobody is working,
  run from there -- and it works exactly as advertised: the gates run against
  precisely what goes to the device, and other people's edits are irrelevant
  rather than dangerous.

  So it should be a flag that does it. Clone `HEAD`, run the gates in the clone,
  deploy from it, clear it up afterwards. Two things it has to get right that
  the printed recipe does not say: the clone cannot go under the temp directory,
  which is a small tmpfs here and where a cargo build dies partway with errors
  that read like a broken shell rather than a full disk; and a fresh clone has
  no build behind it, so without a target directory kept beside it every deploy
  of this kind pays for the whole workspace from nothing.

- **`console well` says its piece on a timer, to whoever is holding the
  device.** Telling somebody using a desktop about drift they did not cause and
  cannot act on, on an interval, is the wrong audience for the right check.

  Two faults that made this much worse are fixed: the card is now replaced
  rather than added to, so a machine left alone no longer collects one identical
  notice per run of the timer, and a run that finds the machine clean now takes
  the standing card down instead of leaving it reporting a drift that has since
  been deployed away. What is left is the question those were hiding.

  The check is worth keeping -- what is in question is who it is for. The shape
  to consider is that it goes on running and goes on recording, and what reaches
  the screen is only what the person can act on and only while they can still
  act on it, with the rest readable when somebody goes looking.

- **A binary put on the device by hand leaves no trace of being ahead.** The
  ordinary road is an apply, which writes what it sent into `/etc/console`, so
  the machine can say which commit it is running. A binary copied into
  `/usr/local/bin` by hand -- which is what happens whenever the apply road is
  blocked and somebody needs the fix on the glass tonight -- says nothing at
  all. The device is then partly ahead of what it claims to be, and the only
  record of that is whichever session did it, which is the least durable place
  it could live.

  It converges the moment a real apply runs, so this is not about drift that
  lasts. It is about the window: two evenings running, the state of the machine
  has had to be carried in a message between sessions rather than read off the
  machine. What would answer it is the device saying what is actually on it
  rather than what was last sent to it -- a hand-installed binary noting itself
  beside `/etc/console`, or `console-check` reading the binaries it is about to
  press and saying when they are not the ones the commit describes.

  The awkward half is that a hand-install is by definition somebody working
  around the tooling, so the note has to be something the tooling can find
  rather than something the person has to remember to write.

- **What this links against wants the same list the programs it runs now
  have.** Every external program is a variant of
  `console_core_external_programs::Program` carrying where it comes from, and
  `desktop.conf`'s `[packages]` is held against it by a test. The system
  libraries have nothing of the kind: what a machine must have on it before
  this will build is a fact spread over a dozen `Cargo.toml` files and a
  manifest, with nothing crossing the two, so a library that is only there by
  accident is exactly the fault the programs no longer have.

- **Somebody else's crate is published without a way to say it is theirs.**
  `crates/console-resume` is carried in the public copy now, which is right --
  holding it back left a `Cargo.lock` naming a member the copy did not have --
  and `VENDORED` keeps its `LICENSE` travelling with it. What nothing checks is
  the other half of GPL-3.0's ask: that a modified file says it was modified.
  The crate's `authors` names upstream and `papers/forks.md` names the release,
  which is what a person would look for, but neither is a thing a copy carries
  into somebody's hands with the code. Deciding what would be enough is small
  and worth doing once rather than being asked about it.

## Checks, stages and flows

- **The device says it was handed back, and once it said so untruthfully.**
  `putting_back` has run on the machine now -- twice on 2026-09-08 -- and both
  runs ended on *the device is as it was found*. What that sentence has actually
  covered is the workspace and what the run opened: a device run presses only
  what nothing else can answer, so `brightness_to`, `volume_to` and
  `close_window` were skipped along with the checks that would have disturbed
  them, and not one of the three has been carried out on the machine yet. Each
  is the reason this entry stays. `brightness_to` writes the backlight through
  `tee` and a glob, where every other write to that file goes through
  `console-brightness` and its range; `volume_to` sets a percentage back through
  `pactl`, and a level that lands a point off what was read would be reported as
  stuck rather than put back; `close_window` ends a window by killing the pid
  `hyprctl clients` names for it, because nothing in this tree has ever asked
  the compositor to close a window it does not have in front -- if
  `hl.dsp.window.close` turns out to take an address, that is the better call
  and this becomes one line.

  **What it does not put back is the home screen's arrangement, and it said so
  in the same breath.** On the 2026-09-08 run `290` went red at its own far
  corner and returned before its carry-back, so LibreWolf was left in the middle
  of the grid rather than in the corner it started in -- and the run's last line
  still read *the device is as it was found*. A reading the run did not take is
  a reading it cannot put back, so either the arrangement is read before the
  first press like the levels and the workspace are, or a check that carries
  something puts it down where it found it on the way out of a failure as well
  as on the way out of a pass. The second is the smaller change and the first is
  the one that survives the next check that arranges something.

  What settles the rest is a run with something deliberately out of place first
  -- on another workspace, at a brightness nobody would choose, with a window
  open that the run will close.

- **The device could ask, instead of being asked.** The checks are run at the
  device from a laptop, which means the person holding it never decides that
  they are about to happen -- they find out because the menus start opening by
  themselves. The card and the strip say what is going on now, and that is the
  small half of it. The other half is the deploy: an apply ends, and the one
  question anybody has afterwards is whether it worked, and the machine that
  could answer it is the one in somebody's hands.

  So: when an apply finishes, the device asks on its own screen whether to
  check that it went well, and the person answers with a button. Yes runs the
  device tier there and fills the same strip; no goes away and does not ask
  again about that apply. That inverts who decides, which is the rule this
  repository keeps everywhere else about that machine -- a clean tree is not
  permission, and neither is a finished deploy.

  What it needs that does not exist: `console-check` is not in `[build]`, so
  the binary is not on the device at all. Everything else is: the checks are a
  library, `lasting` already keeps its table on the device rather than on the
  laptop for exactly this reason, and the surface to ask on is the ordinary
  panel every other question here is asked with. The piece to think about first
  is what a run is allowed to do to a desktop somebody is using -- the tier
  opens menus, moves workspaces and closes windows, and consent to a question
  is not consent to that. Probably the answer is a smaller tier: the checks
  that touch nothing.

- **Nothing has pressed the session being put back on the machine it is for.**
  `370` and `380` are written and they are the nested desktop's answer: a
  terminal saved in one session and open again in the next, and a second start
  leaving a window nobody saved where it was. Both go red when the feature is
  broken -- that was pressed rather than assumed, by breaking each half on
  purpose and reading what they said.

  What they cannot answer is where a window lands. It is moved onto the
  workspace it was saved from by the loop that listens for windows opening, and
  what that listens to is this desktop's event pool, which a nested session does
  not run; on one workspace there is nothing for that half to get wrong in any
  case. So the device's own `Body::Device` half of `370` is still owed: save
  with windows on two workspaces, restart the session, and assert each came back
  where it was, at the size it was if it floats. That is also the only stage
  that can press the two things this port changed for upstream's sake -- a
  special workspace that is not the first one, and a floating window's size.

  The other one only the device can answer is the unit rather than the program:
  `systemctl --user restart console-session.service` with windows on the screen,
  and the windows still there afterwards. `380` presses the same decision by
  starting the program twice, which is the fault's shape; what it cannot press
  is `Restart=always` and `ExecStopPost=console-fell`, which is how the fault
  arrived on a screen.

- **The scenarios open real applications through the launcher, and the launcher
  counts it.** `scenarios/open-something.txt` opens the menu, walks down it and
  presses A, which goes through `found::run` and writes to
  `~/.local/state/console/menu-counts` like any other opening. So the order the
  menu and the home screen are built from is partly a record of the checks
  running rather than of anybody using the machine -- and it feeds itself,
  because what the scenario lands on climbs, and climbing changes what it lands
  on next time. A terminal reached the top of the launcher that way without once
  being opened on purpose. Its count was put back to nought on the device by
  hand; the next device run starts it again.

  What would settle it is the checks opening things by a path that does not
  count -- `Device::open` already dispatches to the compositor and writes
  nothing -- or a counting that ignores what a check started. The first is
  smaller and does not put a notion of "a check" inside the menu.

- **The flows past the second are still prose.** `docs/flows.md` names the
  long walks across crates, and `crates/console-test-flows` runs two of them at the
  fast stage in `just test`: making the buttons your own, and getting around
  without being lied to. Pictures then a film, the evening of music, the home
  screen's rearranging, and being interrupted are still only written down.
  Each wants what the lines below it are owed first; settled when every flow
  on that page names the test or the stage run that walks it.

  The home screen's is the next one that can be walked here, and it is the
  only one of the four that wants nothing new: the daemon's half of waking,
  standing, opening and sleeping is already visible at the fast stage, and
  `making_it_yours` walks a corner of it in passing. The other three each wait
  on a line below -- a fixture folder, a player that answers off the device, a
  restart that can be watched.

- **`Here` sees the daemon's decisions and no surface.** A flow at the fast
  stage can say the viewer was asked for, not what the viewer drew, so the
  drawn half of every step waits for the desktop stage. `docs/programs.md` is
  the seam already planned -- a program as a pure function of what it has
  heard. Settled when one program (the viewer's reel or the music panel's
  playing tab is the natural first) answers a flow step headless through
  that shape.

- **A scenario can press and wait but cannot expect.** The files under
  `scenarios/` are recordings, and the only thing asserted about one is that
  it still parses and still presses. Either flows own their assertions in
  Rust and scenarios stay recordings, or the scenario language grows a line
  that says what should now be true. Decide once, write the decision into
  `docs/flows.md`, and settle it when one flow does whichever it is.

- **The desktop stage can look but cannot press.** The nested desktop is
  photographed and the emulator presses, and no flow can do both in one
  run. Settled when a thumb-script plays against the nested desktop and a
  photograph is taken at a named step of it, in the same invocation.

- **The music flow has no player to talk to off the device.** It could have
  one now, which is the part that changed: the player is a crate in this
  workspace rather than a fork installed on the handheld, and it answers
  `OpenUri` and position on any machine with ffmpeg and pipewire, which is every
  machine this is developed on. What is still missing is the flow itself.
  Settled when the playing tab's two promises -- the song survives the panel
  closing, and the reopened panel agrees with the ears -- are asserted in
  `just test`.

- **The viewer flow has no folder to walk.** Stepping from a photograph
  onto a film wants a fixture folder of things that weigh nothing and decode
  everywhere, kept in the tree. Settled when the reel steps through it in a
  test and the step onto the film is the same test's next line.

## The shape of the code

Written down after a pass that measured the workspace rather than read it. The
tools are on this machine and nowhere in the manifest: they are somebody's
laptop, not the device, and none of them is wanted by a build.

    tokei crates --sort lines                   what is big
    cargo machete                               dependencies nothing asks for
    cargo modules structure -p <crate>          what a crate is made of
    cargo clippy --workspace --all-features -- \
      -W clippy::cognitive_complexity \
      -W clippy::too_many_lines                 what is long or knotted

Two of them lie in ways worth knowing before believing an answer.
`cargo machete` matches on the package name, so `cairo-rs` reads as unused in
every crate that writes `use cairo::`; the finding is false and there is no
config that fixes it. `similarity-rs` compares functions within one file and
never across two, so it says nothing at all about the question below, and the
cross-crate work was measured with a script instead.
`rust-code-analysis-cli` does not build on a current rustc and is a dead end.

Neither `clippy --message-format=short` nor `cargo dylint` prints a lint's name
in its short form, so a summary that greps for the name reports a clean
workspace when nothing was clean. Grep the message text.

- **A panel cannot carry out `Doing::Listen` at all, and the reason it has not
  been given a way to is that nothing would ask.** Every card matches
  `Doing::Listen(_) | Doing::Deafen(_) => {}` and no card has ever emitted one:
  there is no glib loop translating either into a poll, which is what this
  looked like from a distance. What is owed is real but it is second -- a
  `Listening` held by the panel with its words pumped onto the main context --
  and it is blocked behind a card that wants a topic. The music card wants
  `Player` and the notifications card wants `Notices`, and neither has a source
  yet, so building the panel side first would be a mechanism with no consumer.
  The source comes first, the card that asks for it comes with it, and the
  panel plumbing lands under both.

- **The runtime is on the pool and no panel is, and `docs/programs.md` says
  the rest.** `Wants::Words` reaches `console_events::listening` now,
  `Doing::Deafen` takes a topic back, and the runtime's wait is one
  `recv_timeout` on the pool's channel bounded by the next round -- so `due()`
  and its EXPLICIT021 allow are gone. What is left of it: do it again where a
  panel's glib loop lives, because every card still matches
  `Doing::Listen(_) | Doing::Deafen(_) => {}`; make something actually hang up
  when it stops being looked at, which nothing yet does; and write the sources
  the other topics promise, one at a time, each watched in a nested desktop
  before it is believed. None of it has been near the device: the pool is a
  daemon and what a daemon does when it is restarted underneath a panel is not
  readable from a laptop.

- **The four compositor watchers are one now, and nothing has pressed it on a
  screen.** `music-bar`, `bar-door`, `stick-scroll` and the status bar's
  `watch` each opened Hyprland's socket; all four ask `console-events` through
  `console_events::layers::watching`, and `console_onscreen::watching_layers`
  is gone rather than left standing beside them. What they get back is the same
  `Sender<()>` and the deciding of which lines are about a layer stayed in
  `console_onscreen`, because the bar keeps different ones from the wallpaper.

  What this buys is one subscription where there were four. What it costs is
  that the bar's two door icons now depend on a second daemon, and `bar-door`
  is the one with no tick underneath it: it draws when it is told and never
  otherwise. `Heard::GotIn` is what stops a gap leaving it wrong -- the pool
  says *you are in* on every reconnection as well as the first, and that word
  means *ask again* to a watch whose words already mean that. It is asserted in
  `the_deafening`, and it has never been watched happen: what is owed is
  `console-events` stopped and started under a running bar, on the device, with
  the launcher opened during the gap. There is no check for the door icons at
  all yet, which is the other half of why this cannot be believed from here.

- **Every crate that speaks writes the same accessor by hand.** `word`, `tag`,
  `name`, `written`, `said`, `asking` -- `console-onscreen/src/homeward.rs` and
  all three in `console-input-keyboard/src/keymap.rs` are the identical shape, and
  `console-manifest-engine`, `console-input-gamepad` and `console-wallpaper` carry it too. This is the
  one worth doing: not a generic function but a derive over the `words` enum,
  because it is boilerplate the crate layout guarantees will be written again
  every time a crate learns to say something.

- **A few functions run past a hundred lines, and one past clippy's cognitive
  threshold.** The clippy line above lists them; `console-input-keyboard/src/bin/keyboard.rs`
  is the worst on both counts and is the only one that trips both. Nothing here
  is a bug, so this is a line about reading rather than about correctness.

- **`//!` was left standing and has not been held to the same rule.** Module
  heads survived both passes untouched, and EXPLICIT020 does not judge them:
  whether a head earns its place is a reading, and a lint cannot do a reading.
  By the rule the gate now keeps, most of what they carry is prose that belongs
  in `docs/` or nowhere. Whoever takes this should read them rather than strip
  them: with the `///` gone the heads are the only prose left in the tree, and
  some of them are the only remaining record of why something is the way it is.
  Those want moving before they want deleting. What the second sweep took out
  is not lost either -- it is in the commit that took it, and a head being
  written now is the right moment to go and read what that file used to say.

  The crate renames of 2026-09-04 gave this a second half and made it the next
  thing to do. Most crates here are named for what they are now, and the first
  line of the head is the sentence that has to agree with the name: somebody
  reading `console-core-reconnect` should not be met with *Reaching again for
  something that has gone*. Every head's opening line says what the thing is,
  plainly, to somebody who has never opened this repository -- the same test
  the names were held to -- and what is under it is either a decision worth
  keeping or prose that goes.

## Where this is going

_A direction rather than work. Every entry below is larger than this file's own
rule allows and each one wants a page in `docs/` before anybody starts it; what
is written here is the argument for the order, so a piece picked up out of turn
is picked up knowing what it is standing on. Nothing below is a plan to rewrite
what is here. It is what stands between a desktop that works for the person who
built it and one that works for somebody who did not._

**What this already is, said plainly, because the direction only makes sense
against it.** `desktop.conf` is a declarative description of a whole machine
applied by a compiled engine under a lock, and `migrations/` is the half that
takes back what the manifest stops naming: between them a device is a statement
rather than an accumulation, which is the thing most desktops never get and the
thing every other item here is built on. `means.rs` is one table that the daemon
carries out, the setup screen writes and the guide reads, so what a button does
cannot drift from what a person is told it does. The surfaces a handheld cannot
borrow from a desktop -- the keyboard, the files, the viewer, the menu, the
settings, the notifications -- are written rather than wished for, and the
browser add-on proves the grammar can be pushed into a window this repository
did not write. And the emulator, the three stages, the flows, the panel's own
telling and the wait store mean a change can be disbelieved before it reaches a
thumb. That is a great deal more than a themed Hyprland, and it is why the list
below is about reach and safety rather than about features.

**What it is not is a thing anybody else can have.** It is one device, deployed
by a push from one laptop that knows its address, built from a rolling base with
no way back, configured before first light by whoever built it, with no answer
at all for the morning it does not come up. Each of those is a separate piece of
work and they have an order.

- **A program has to be a function before anything else is worth standing on
  it.** `docs/programs.md` argues this at length and is right; what is worth
  adding here is that it is first rather than merely wanted. Everything below --
  a rollback that can be trusted, a first run that cannot half-happen, a
  recovery surface that has to work on the worst day the machine has -- is a
  promise about behaviour under conditions nobody can reproduce by hand. A
  program whose state is a file six others write, whose subscriptions are its
  own, and whose decisions can only be observed by letting them happen, cannot
  be promised anything about. Stages 1 and 2 of that document are the spine, and
  the input reader is what makes the spine worth having.

  **Stage 1 is in.** `console-program-contract` is the trait, the words and the
  doings; `console-program-runtime` is the loop that carries them out for a
  program with no toolkit of its own. The names in that document were
  `console-turn` and `console-said`, and they were changed on the way in for the
  reason the tree already keeps: a crate is named for what it does, and a name
  that has to be learned before it can be read is the wrong one for the crate
  every other program depends on. The pool is `console-events` for the
  same reason.

  **Stages 3, 5 and 7 are in, and stage 4 is most of the way.** The thirteen
  shell scripts are gone: the last two, `console-pull` and `allow-uinput`, are
  programs whose step chains are `set -e` said in a way a transcript can press,
  and `console-deploy` and `console-migrate` are `console-device`. Every one of
  the seven panels holds its state in a `Program` beside it -- `notices`,
  `pressing`, `standing`, `pressing`, `watching`, `choosing`, `standing` -- with
  the toolkit, the disk and the machine left in the binary. Of the daemons,
  `game-return`, `controller-profile`, `keyboard-toggle`, `keyboard-show` and
  `console-sky` are on it.

  **What each panel bought is the composition rather than the pieces.** The
  pieces were mostly testable already; what was not was what a press does to
  several of them at once, and that is where the faults were. Stepping to the
  next thing in the viewer forgets five settings and keeps one. Backing out of a
  thing in the files panel lands on the row that opened it, and which row that
  is depends on what else is drawn above the things. The music library is read
  once a run and only when something is unread. None of those had a test and all
  of them are a line somebody would get wrong while reading it.

  **`offers()` was not built, because nothing wants it.** The document's own
  rule is that it waits until two programs want it, and the tree was read for
  the second. There is exactly one place a program reaches across to another:
  the music panel opens the files panel standing on the song, and it does that
  by starting it, which `Doing::Start` already says. `console-files` knows
  nothing about the download panel, `console-status-bar` knows nothing about
  the music page, and no program asks another for an answer. A registry written
  now would be a shape guessed from one example, and the one example does not
  need it.

  **Two programs asked for something the contract does not have, and one of them
  got it the other way round.** `console-sky` needs a wait that is a different
  length every time -- the end of a settling, two seconds after a wallpaper
  would not take, five minutes for the sun -- and `Wants::Round` is a fixed
  stretch. Rather than grow the contract, it keeps its own loop the way a panel
  keeps GTK's and says how long to sleep as a doing, which is the better answer:
  the decision is testable and the loop stays where it can see the channel.
  `voice-compare` is the one still owed something, and what it wants is how long
  a `Doing::Ask` took, which nothing can currently be told.

  **Stage 2 is begun.** `console-events` holds the compositor and the sound and
  hands the lines to whoever asked, replaying the last one to whoever has just
  arrived. `console-sky`, the four compositor watchers and the status bar's
  sound reading have all stopped holding their own. What is owed is the other
  five sources -- `nmcli monitor`, mako's bus name, systemd's unit changes, the
  player, and a watched path -- one at a time, each with the program that
  wanted it moved over in the same commit. A source with no consumer is a
  subscription nobody asked for, which is the thing this crate exists to stop.

  `nmcli monitor` and mako's bus name are the two with a consumer waiting:
  `console_status_bar::watch` still opens both itself, and `Topic::Network` and
  `Topic::Notices` are what they would become. The player is the music bar's,
  and it is the one that wants watching hardest, because what a player does
  when it is restarted underneath is the least predictable of the lot.

  Each of them wants watching before it is believed, and the sound is the
  shape of that: it was run under the unit's own confinement rather than in a
  shell, killed underneath a listener, and the pool killed with `-9` to see
  whether it left a `pactl subscribe` behind. What a program of somebody else's
  does when it is restarted underneath is not readable from its manual.

  What was owed here -- one daemon and one panel on the runtime, with a
  transcript that fails when either changes its mind -- has been paid twice
  over: `game-return` is a daemon that stays up and whose sleep rule could not
  be tested at all before, and every panel has one.

  **`stick-scroll` is what is left, and it is deliberately last.** Its decisions
  are already a `Doing` and its tests are already transcripts, so the wrapping
  is small; what is not small is that it is the whole front of the machine, it
  publishes a virtual device, and it reads its pad *during* the turn through
  `Plugged` rather than being handed what arrived. Moving it means `came()`
  reading the devices, which is the same change as the input reader below, so
  the two should land together and be pressed on the device rather than
  reasoned about. `console-home` and the keyboard daemon are the other two, and
  both are toolkit programs of the panel kind rather than loops.

- **A surface should not be paid for twice, and a single program drawing all of
  them is the wrong way to stop paying.** The measurement is already in this
  file: on every surface the two long stretches are getting a process running and
  getting GTK onto the screen, and everything a panel's own code does is a
  fraction of either. So the cost is not in what a panel decides, it is in the
  fact that a panel is born every time somebody asks for one, and that is worth
  fixing before another surface is written.

  **One resident program holding every chooser was the first answer here and it
  is written down as rejected.** Three things are wrong with it. A process
  exec'd per opening is *stateless by construction* -- a panel cannot carry
  yesterday's mistake into today, because there was no yesterday -- and that is
  the strongest reliability property the panels have and the only one that costs
  nothing to keep. A host loses it, and it loses it into exactly the fault class
  this whole section is about: something that works all day and is wrong after a
  week. Second, `Rows::Asked` asks the machine at the moment of drawing, so a
  panel whose rows are slow to build blocks the loop it is drawn on; today that
  blocks the panel a person is looking at and nothing else, and in a host it
  blocks the host, which is the thing that has to answer *open the settings*.
  That is not a fear about crashes, it is liveness, and it is a regression a
  person would feel. Third, the shared-fate objection stands even though it is
  smaller than it looks -- only one chooser is on the screen at a time, so a
  crash loses the same one surface either way -- because a supervised host that
  restarts is still a restart somebody watches, where a dead one-shot is a button
  pressed again.

**The state argument that was used to justify a host does not need one.**
`console-events` is a daemon, not a drawing program: one subscription per
source, handed to whoever asks, replayed to whoever has just arrived. That is
stage 2 of `docs/programs.md` and it removes the per-panel `pactl subscribe`
without merging a single panel into another.

  **What is left is exec and toolkit start, and a warm spare answers both without
  shared fate.** A process that has already started and already brought GTK up,
  sitting idle, waiting to be told which panel it is; being told, it becomes one,
  and another is warmed behind it for the next press. One process per surface,
  fresh state per opening, no shared loop, and the two measured stretches mostly
  gone. It is the zygote every phone ships and the spare renderer a browser
  keeps, and the reason to write it down here rather than reach for it later is
  that it is a shape rather than a tuning: it decides how a panel is entered, and
  every panel written before it will have to be re-entered afterwards.

  **What it does not obviously answer, and the boundary that was stamped to
  settle it.** A layer surface is created with its namespace, and the namespace
  is which panel it is, so the part of the opening that is the compositor
  granting a surface and painting it first cannot begin until the spare knows
  what it has become. How much of an opening is before that point and how much
  is after is the number that decides between one generic spare, a spare per
  panel kind, and a panel that merely unmaps rather than exits.

  `shown` is now three stretches rather than one. `surface` is up to the layer
  surface existing with its name on it, stamped from the window's realize;
  `mapped` is the compositor being asked to put it up; `shown` is what is left
  of `present`. Both new stamps are in `laid_over_everything`, where the
  namespace is set, and the argument is written beside them.

  **What a nested desktop on the laptop says, which is the shape and not the
  number.** Three openings of the menu, under load and with the rows still
  being read, came out at roughly `exec` 28, `gtk` 11, `built` 13, `surface` 39,
  `mapped` 0.6, `shown` 0.8, `frame` 136. Nearly the whole of what used to be
  `shown` is `surface` -- GTK realizing the window and making the layer surface
  -- and what follows it inside `present` is under two milliseconds together.
  The compositor's part is not in `present` at all; it is in `frame`.

  **It does not hold on the device, and the device is the machine the decision
  is about.** Read there on 2026-09-08, a panel the host draws is `exec` 42,
  `placed` 10, `frame` 5, `built` 3, `surface` 2.5, `shown` 0.7, `mapped` 0.2 --
  `surface` is nothing at all, and `frame` is single figures rather than the
  laptop's 136. The stretch that begins once the panel knows what it is has
  almost vanished on the machine that matters, and what is left in front of it
  is `exec`. So the reading argues *for* the generic spare rather than against
  it: what a spare with no name can do in advance is now nearly the whole of an
  opening. The stand-in is the other half of the same reading -- `home-square`,
  which the host does not draw, still carries `gtk` 9 and `surface` 44, which is
  what an opening costs when nothing was warm.

This is the exception to *optimisation is last*, and the reason it is an
  exception is that a person does not experience a wait as slowness, they
  experience it as the machine not having heard them. A surface that is up or is
  not there yet, which `console-ui.md` already asks for, is not reachable by
  making a slow thing faster.

- **An apply is not a transaction, and on a handheld it has to be.** `console
  apply` walks the sections in order and installs what each names. A failure
  partway leaves a machine that is neither what it was nor what it was asked to
  be, and nothing on the device can say which. On a laptop that is an afternoon;
  here it is a person holding a thing that will not come up, with no keyboard,
  no terminal and no idea what the word "manifest" means.

  What every appliance-shaped system arrived at is the same shape, and it is
  worth naming because we should take the shape and not the implementation.
  SteamOS keeps two root partitions and swaps between them, and the Steam Deck
  cannot install a package as the price. Bazzite and the rpm-ostree family get
  the same rollback from a composed image and pay the same price in a different
  currency: a host change means a container or a layer. The embedded world's
  version is older and blunter -- the bootloader counts boots, and a slot that
  does not confirm itself is rolled back without asking anybody.

  **We should not become immutable, and this is the entry that says why.** The
  reason those systems are immutable is that they had no description of the
  machine, so the only way to know what a root filesystem contains was to ship
  it whole. We have the description. What immutability buys -- *this machine is
  exactly what was intended and can be put back* -- is what `desktop.conf` and
  `migrations/` already claim, and the missing piece is not a read-only mount, it
  is that an apply has no identity and no previous. So: an apply becomes a
  **generation**, numbered, recorded on the machine with the commit it came
  from; a snapshot is taken before it and is what the previous generation means;
  and an apply that dies partway is a generation that never confirmed rather than
  a machine in an unnamed state.

**The cheap half of that is in, and it was cheaper than anyone had checked.**
The device's root filesystem is btrfs with the `@` layout, snapper already has a
configuration for `/` and one for `/home`, `snap-pac` already brackets every
pacman transaction with a pair, and `limine-snapper-sync` already writes a boot
entry for each root snapshot -- all of it from the CachyOS base and none of it
ever named here. So `console apply` now takes a `pre` on both configurations
before it touches anything, before even the sweeps, and a `post` when it reaches
the end, both with `--cleanup-algorithm number` so the cleanup timer that is
already running prunes them like everything else. The description is the commit.
`crates/console-manifest-engine/src/previous.rs` is the whole of it and the
argument is in its header; `snapper` is in `[packages]` because the apply runs
it.

  Two configurations rather than one, because the apply writes both subvolumes:
  a snapshot of `/` alone would put back a machine whose home is still holding
  what the apply left, which is the half-and-half state this entry is about. A
  machine that cannot hold a snapshot prints `no snapshot` with snapper's own
  reason and the apply goes on -- a rollback nobody can take is worse than one
  nobody was promised, so it is said out loud rather than swallowed.

  **What is still owed, and none of it is the snapshot.** An apply has no
  identity: a snapshot is described by a commit, which is not a generation
  number and nothing on the machine records which generation is running. Nothing
  confirms one. Nothing counts a failed boot. And the boot menu that makes a
  root snapshot reachable is `limine-snapper-sync`'s, which is on the device
  because CachyOS put it there and is not named in `[packages]` -- so a device
  rebuilt from this manifest would take the snapshots and have no way to boot
  one. Naming it is a claim about the bootloader that has not been thought
  through yet, and it is the next thing here.

  What would settle the rest: an apply interrupted on purpose, at each section,
  on a device that then comes up as the generation before it.

- **A generation confirms itself, and the thing that confirms it is already
  written.** `console well` asks every hour whether this machine is still what
  the manifest says and whether every piece of it is up, and `console-fell` is
  how a daemon that died says so. Those are health checks in the sense the
  atomic-update world means it, and they are wired to a notification rather than
  to a decision. The missing line is short: a generation applied and not yet
  confirmed is confirmed by the first `console well` that passes after a boot,
  and a boot that reaches neither the check nor the desktop is counted, and a
  count that runs out goes back.

  The failure this guards against is the one the OTA writing is unanimous about
  and the one no test here can currently reach: an update that boots and then is
  not usable. A device that comes up to a compositor with no bar, no controller
  daemon and no way to press anything is `active` all the way down -- which is
  the fault `210` was written for, arriving from a direction that does not have a
  person nearby to notice it.

- **Nothing can say what the machine does when it does not come up.** This is the
  gap that most deserves the word *operating system*. There is no surface below
  the compositor. Hyprland failing, `console.target` failing, a disk that filled,
  a generation that will not confirm: every one of those is the same black screen
  and the same silence, on a machine whose only input is a pad and whose only
  output is a panel someone is holding.

  What is owed is a small program that needs nothing this desktop provides --
  no compositor, no toolkit, no session -- and draws on whatever the kernel has
  left. It says what fell, in the words `console-say` already uses, and it
  offers exactly the choices a person can act on: go back to the generation
  before this one, or turn it off. It is reached from a button held at boot,
  because a person who cannot get to a desktop cannot get to a menu on it.

  It is small, it is the least fun thing in this file, and it is the difference
  between a device somebody can own and a device that has an owner on call.

- **The manifest has never once been built from nothing, and three times it has
  been wrong in the same way.** `libpulse`, `grim` and `xkeyboard-config` were
  each on the device because the base install or something else pulled them in,
  each was reached for by something here, and each was found by accident rather
  than by a check. Every one of them would have been found in a minute by a
  machine built from `desktop.conf` alone and then asked to do the thing.

  So there is a stage this repository does not have: a body below `Here` that is
  a fresh machine -- a container or a virtual one -- brought up from the manifest
  and nothing else, and run against. It cannot answer what a screen answers and
  it will never see a pad, and that is fine; what it answers is the one question
  nothing else asks, which is whether the file that claims to be the whole truth
  about this device is the whole truth.

  What would settle it: `xkeyboard-config` removed from `[packages]` in a branch,
  and the stage going red because a keyboard with no keymap to compose says so
  and stops.

- **This is one device, and an operating system is a thing other people can
  get.** The deploy path is a push into `/etc/console` from a laptop holding an
  address in its environment, which is exactly right for the person who wrote it
  and is not a distribution. Three things stand between here and somebody else
  having this, and they are separable.

  **An image**, built from the manifest the same way an apply is, so that the
  first thing a machine runs is the same statement every later apply is made
  against. **A channel**, so a device updates itself from something signed rather
  than from a git remote it trusts absolutely -- see the entry on that below.
  And **a first run**, which is the one below this.

  The order matters and the temptation is to do the image first because it is
  the visible one. It is last of the three. An image of a machine that cannot
  roll back is a way to give a stranger a brick.

- **The desktop begins before anybody has seen it, and there is nowhere to say
  who you are.** The network and which of the applications a person actually
  wants are still decided by whoever built the device, in this repository, for
  one person.

  Four of them have stopped being. The Language tab is which words the machine
  is in, which alphabets the keyboard types and which one the paddle listens
  for; Setup holds where the hour is kept and what the machine answers to.
  `console-machine` is the one door to the four files under /etc those write,
  and `console_input_alphabets` is the list the keyboard and the panel agree
  through. Thai was the case that proved the point -- the keyboard composes its
  layers from what `xkeyboard-config` has, so a second alphabet was a word on a
  command line, and there was no moment at which anybody was asked for it.

  What is missing now is the moment rather than the settings. A setting somebody
  has to go looking for is one that a person who does not know it exists never
  finds, and the whole argument for the tab was that nobody should have to know.
  A first run is that moment. It is the same card as every other surface here,
  driven by the same buttons, and it is the first thing a person will ever use
  this device for, which makes it the one surface where being slow or unclear
  costs the most.

  The trap, and every desktop has fallen into it: a first run that asks for
  things the machine could work out, or that cannot be gone back through, or
  that has to be finished before anything works. It asks for what only a person
  knows, it can be left, and what it does not get it asks for again at the
  moment it matters.

- **The applications this desktop did not write are where the promise breaks,
  and they cannot all be rewritten.** The contract is that the d-pad reaches
  everything, A takes it and B leaves. It holds across every surface in
  `[build]` and it stops at the edge of them. `pamac` is where a person installs
  things; `pavucontrol`, `blueman` and the network applet are what the bar's
  icons open; Signal, FreeTube and a browser are what somebody actually came to
  the device for. Each is a pointer program, and `docs/files.md` already says
  exactly what that costs: the row a thumb moved to and the thing the button
  acts on are two different objects.

  This is the seam Steam Deck users complain about in their own words -- desktop
  mode needing a mouse, the keyboard being janky, the whole thing being an
  escape hatch rather than a place. This repository's answer to that has been to
  write its own, and that answer is right for the ones it has taken and does not
  scale to the ones left. **Firefox OS and Ubuntu Touch are the warning and they
  are not distant.** Both had better ideas than the thing they were replacing,
  both decided the way to a usable device was for the platform to supply the
  applications, and both died of the gap between what they had written and what
  a person wanted to open. Nobody here is going to write a Signal.

  So the direction is a policy rather than a program, and it has three tiers.
  **Ours**, which keeps the contract. **Dressed**, which is a foreign program the
  desktop makes reachable from outside -- the browser add-on is the proof this is
  possible, and the pointer is not the only lever: a program can be given the
  pad by something that knows what its widgets are, exactly as a page is. And
  **pointer**, which is a program nobody has dressed, entered deliberately with
  the machine saying so, rather than arrived at by pressing A on a row and
  finding the rules have changed.

Two things fall out of it. The tier belongs beside the program in
`console-core-external-programs` and `[packages]`, where the rest of the truth
about a program lives, so *what happens when I open this* is a fact about it
rather than a thing a person discovers. And `offers()` -- stage 6, deliberately
left until two programs wanted it -- is what lets somebody else dress an
application without changing this repository, which is the only version of an
ecosystem that a device with one author can have.

- **A program should reach nothing that belongs to a program a person can see,
  and on this machine every one of them reaches all of it.** This is the entry
  the one above is standing on, and it is written second only because the tiers
  are what make it concrete.

  What is true today, said without softening it. Anything that can open
  `/dev/input/event*` sees every button and every letter typed on the device,
  because the on-screen keyboard is the only keyboard and it is an input device
  like any other -- a keylogger here is a file open. The compositor's socket is
  reachable by everything, so anything can ask what windows exist, what is
  focused, and send a keystroke into a window it does not own. `grim` is on the
  machine and nothing mediates it, so anything can photograph the screen at any
  moment and say nothing. The notification name is a name, so anything can speak
  in the desktop's voice, which is the voice faults arrive in. Everything runs as
  the one user with the whole of a home directory, so a browser extension, a
  thing downloaded into Videos and a Flathub application installed from the shop
  all read the syncthing store, the browser profile and each other. And
  `$XDG_RUNTIME_DIR/console/` holds the ownerless variables `docs/programs.md`
  is about, writable by anything, which makes them an integrity question and not
  only a correctness one.

  **The tension is real and has to be said before the answer.** This desktop
  works by crossing exactly these lines. The add-on drives a page from outside
  it. Dictation types into whatever holds the focus. The keyboard takes the pad
  away from everything. A panel asks the compositor what is in front of a person
  to know what a button means. Dressing a foreign application, which the entry
  above asks for, *is* one program driving another's interface. An isolation
  rule that forbids those forbids the machine.

**So the rule is not that nothing crosses, it is that nothing crosses
ambiently.** Every crossing is declared, named, narrow and revocable, and
anything not declared is refused. That is not imported doctrine; it is the
fourth time this repository has reached for the same move. `[packages]` is what
may be installed. `console_core_external_programs::Program` is what may be run.
`means.rs` is what a button may do. And `files/etc/sudoers.d/console` is the
purest example already in the tree: two programs, each taking one thing it
understands, each refusing a name it does not know, each with the argument for
why written above it. The manifest is a capability list that has never been
called one.

  **`console-program-contract` is what turns that from a hope into a boundary.**
  A program that says what it wants done rather than doing it is a program whose
  effects can be held against a declaration before they are carried out, which
  makes the loop the one thing that has to be trusted rather than every program
  being trusted. This is the second reason to build the contract and nobody
  wrote it down: it is the reference monitor, and it is free once the programs
  are functions.

  The contract is in and the monitor is not: `console_program_runtime::run`
  carries out every `Doing` it is handed and holds none of them against
  anything. What it does have is the one place to put the check, which is what
  was missing. `Doing::Start` is already the shape that would let the bar be
  confined -- it starts a chosen program in a scope of its own through the
  manager rather than exec'ing it inside the caller's control group -- and that
  is also the settling of the open item where everything opened from the menu
  lives in the controller's.

  **Ours and theirs are different jobs and only one of them is expensive.**
  Confining what this repository wrote was nearly free and is in:
  `console-.service.d/confining.conf` is the floor every console unit stands on
  -- no new privileges, /usr and /etc read-only, a private /tmp, no modules, no
  clock, no cgroup writes, native syscalls only -- and each unit adds the two or
  three doors its own program needs. Most of them take `PrivateDevices=yes`, the
  ones that only read their config take `ProtectHome=read-only`, and the ones
  that can take `RestrictAddressFamilies=AF_UNIX AF_NETLINK`, which is a socket
  to the compositor and to the bus and no way off this machine. `console-sky`
  and syncthing cannot, because they are network programs, and the two that
  cross to Game Mode cannot either, for a reason nothing said until it had cost
  a day: every line of that kind is seccomp, and a unit of an unprivileged
  manager carrying any seccomp at all is a process with the no-new-privileges
  bit whatever the flag says, so a unit that has to become root can carry none
  of them. The argument is in `confining.conf`. Two lines are deliberately
  absent and say so in the file: `ProtectKernelTunables=` would make /sys
  read-only and the screen's brightness is a file in it, and `ProtectProc=`
  hides other users' processes on a machine that has one user.

  **What the confinement does not cover, which was measured rather than
  assumed.** The units that start a program a person chose carry a drop-in that
  takes all of it back off: the bar, the home screen and the session. They start
  the launcher and every application through it, the panels and the settings
  panel's one `sudo`, and the windows from yesterday -- and
  `systemd-run --user --scope` does not fork through the manager: it execs in
  the caller's own namespaces. A scope started inside a unit with `PrivateTmp`
  sees that unit's empty /tmp, which was tried before any of this was written.
  So a sandbox on the bar is a sandbox on Firefox and on Steam, and the earlier
version of this line -- _the bar does not need `/dev/input`_ -- was wrong for
  the reason that matters: waybar does not, and everything waybar opens does.
  What would have to change first is that a chosen program is started by the
  manager rather than by us, which is a transient unit with the environment
  handed over rather than inherited, and it is not an afternoon.

  Confining what somebody else wrote is the larger half, and the thing that
  makes it possible to confine an application without making it useless is
  portals -- and **`xdg-desktop-portal` is not in `[packages]` at all**, though
  the device is holding it and three backends besides, every one of them there
  because something else pulled it in. That is the worse half of the two: a
  machine rebuilt from this manifest would have no portal, and this one has four
  packages' worth that nothing here chose, asked for or knows the version of.
  The trap that goes with it is worth naming, because half-done containment is
  worse than none: an application confined with the home directory handed to it
  is theatre, and theatre is believed.

  **The threat model, so the work is sized honestly.** One person, one device,
  no untrusted local users, nothing here defending against somebody holding it.
  What is real is what the person installs and opens: an application from the
  shop, an extension in a browser, a file fetched into Videos by a panel written
for fetching files. The unusual one is that this device *compiles its own
operating system* out of a git remote nothing verifies, which is the same edge
  the signing entry below is about, arriving from the other side.

  **What settles the first piece is a check rather than a document**, and it is
  written: `320-only-the-controller-is-reading-the-buttons` walks /proc on the
  device and asks which processes hold a `/dev/input` node with nothing on the
  screen. Five names may, each with its reason beside it -- `systemd` and
  `systemd-logind` have the power button and the lid, `Hyprland` reads
  everything because it is what draws, `inputplumber` publishes the pad, and
  `stick-scroll` is ours. Anything else is either a claim that was taken and
  never handed back or a program nobody meant to be listening. It also fails
  when `stick-scroll` is missing from the list, because an ssh that answered
  nothing would otherwise read as a machine where the buttons are private.

  That single question is most of the difference between a machine where typing
  is private and one where it is a convention, and the device answers it: 320
  passed there on 2026-09-08, so nothing but the five is reading the buttons.

- **Two sessions, and only one of them can own the machine.** The desktop is
  ours and Game Mode is Steam's, and the machine leaves for it entirely. That is
  the right call and the seam is honest, but everything with a life longer than a
  session sits on the wrong side of it: a download that finishes in Game Mode,
  an update that wants a reboot, a notification raised while Steam has the
  screen, a wallpaper that changed with the weather. Today those are simply lost
  or simply late.

  The direction is that the desktop is the machine and Game Mode is a thing it
  runs, in the sense of who owns state rather than who owns the framebuffer. What
  is owed is small and specific: what a person is told when they come back, and
  what was decided while they were away.

- **Nothing here teaches, and a guide somebody has to go and open is a manual.**
  `console-button-guide` is read out of the one table that decides the binds, which is
  the hard half and it is done. What is missing is that it is a place rather than
  an answer. A person meeting a surface for the first time has a question about
  *this* surface -- what Y does here, whether B will lose what they typed -- and
  the guide answers about the machine.

  Y is the lendable button and the guide is what it is lent for on a surface with
  nothing else to offer. A hint at the moment of need, in the surface, in the
  words the table already holds, costs nothing to keep true because it is read
  from the same line the daemon carries out.

  The other half of teaching is that nothing here is ever a first time twice.
  What a person has already done is not written down anywhere, so the machine
  cannot stop explaining, and a machine that keeps explaining is one people learn
  to press past without reading.

- **The interface has one language and one size.** Thai can be typed and cannot
  be read: every word this desktop says is English, written in the source. The
  Language tab makes that visible rather than fixing it -- a person can put the
  machine in Dutch and get a Dutch browser and a Dutch calendar with rows that
  are still English, which is the one thing that tab says out loud about
  itself. The
  size of everything is one number -- the screen scale -- which is a real control
  and a blunt one, because a person who wants larger words is asking for larger
  words rather than for less of the folder. There is no contrast setting, nothing
  is read aloud on a machine that already has a microphone, a hearing and a way
  to type what it heard, and the dictation points in one direction only.

  This is the entry most likely to be deferred forever, so the line worth holding
  is narrower: nothing new writes a sentence into a program. Where the words a
  surface says come from is a decision that costs nothing while there is one
  language and is a rewrite of every panel later.

- **What a person would actually mind losing is not backed up in any way they
  chose.** `syncthing.service` is in `[services]` and has no surface at all, so
  what is being synchronised, to where, and whether it worked are questions with
  no answer on the device. Beside it: the machine has no lock, on a thing whose
  whole nature is being put down on a table, and no answer about encryption at
  all.

  Neither of these is a feature somebody asks for. Both are what an operating
  system is assumed to have already done, and both are noticed exactly once.

- **Nothing knows a build is bad until somebody is holding it.** The wait store,
  `console well` and `console-fell` all write locally and are read by whoever
  thinks to look. For one device that is the correct amount of machinery. For a
  second device it is nothing, and a regression is found by the person it
  happened to, who is not the person who caused it.

  The trap here is the obvious answer, and this repository's stance on it is
  already right and should be written down rather than assumed: **nothing leaves
  the device on its own, ever**. What is owed is the deliberate version -- a
  person can hand over what the machine has already written about itself, in one
  press, having seen it. The store is JSON a line at a time and was built to be
  read by anything, which is most of that work done.

- **The device trusts a git remote absolutely, and builds its own operating
  system out of it.** An apply compiles what `[build]` names, on the machine,
  from whatever history arrived. That is a good design and it has one unguarded
  edge: nothing checks who wrote the history. For a laptop pushing over a private
  network that is fine; for a channel, or for a second device, it is the whole of
  the security model and there is none. Signed commits or signed tags, checked by
  the engine before it builds anything, is the small version and it should land
  before the channel does rather than after.

- **Optimisation is last, and here is the note that says why, so nobody has to
  argue it again.** Everything above is about a machine that behaves the same way
  twice: after a restart, after a bad update, in somebody else's hands, in a
  language nobody here reads. A faster version of a machine that does not do that
  is a faster version of the problem, and every hour spent on a number is an hour
  not spent on the reason the number was being looked at.

  The two exceptions are already named and are exceptions because they are shape
  rather than speed: how a panel is entered, and one pool holding the
  subscriptions. Both are cheaper now than after another surface is written, and
  both stop being available at all once enough of the tree assumes the current
  shape.

  When the time comes, the wait store already knows where to look, which is the
  whole reason it exists.

## More than one machine

_Also a direction rather than work, and it stands on the section above rather
than beside it: the channel, the first run that cannot half-happen, the rollback
that can be trusted and the signed history are named there already, and every
one of them is a prerequisite here rather than a neighbour. What is new below is
the axis this tree has never had. One manifest describes one machine, and there
is no way in it to say `here, but not there`. Everything about handhelds,
consoles, laptops and phones is downstream of that one sentence._

**What is Legion Go by name, so the size of it is not guessed at.**
`files/usr/share/inputplumber/devices/50-legion_go.yaml` and the profiles beside
it, `console-input-gamepad`'s `go.rs` and the capture under `fixtures/`, the
monitor block in `hyprland.lua` that says a panel is mounted portrait and turned
a quarter, the backlight and battery paths, the boost, and the sysfs file the
rumble is silenced through. Each of those is right, and each is written as though
there could only ever be one machine, which until now was true.

- **A second machine before a second kind of machine.** The laptop this
  checkout is edited on is the cheapest possible second, and the point of it is
  not the laptop: it is that `desktop.conf` learns to vary at all, while the
  person holding both can see what broke. The thing to protect while it happens
  is the migrations arithmetic. It is four sets over the manifest's own history,
  and it is the reason nothing has to be remembered; a name that can leave one
  variant while staying in another is the first thing that could quietly make it
  answer a question nobody asked.

  What settles it: a second machine that applies this manifest and comes up, and
  `cargo test -p console-manifest-migrations` still going red when a name leaves
  either variant unswept.

- **Variance is selection, and must never become a config language.** The
  manifest is a statement about a machine. A variant is another statement, and
  the engine picks between them by asking the machine what it is -- what the
  firmware calls the board, what the device tree says -- rather than by a
  conditional somebody writes in a file. The moment the manifest can compute, the
  thing that makes this tree worth copying is gone: a machine stops being
  readable as a description and goes back to being a program whose output is a
  machine.

  Whether that is a section heading per shape, or a manifest per device that
  names a common one, is a page in `docs/` before it is a line of code. Whichever
  it is, `console check` has to keep meaning *this machine has drifted from what
  it says it is*, and not *one of several things it might be*.

- **Input shape, rather than input device.** `docs/button-contract.md` describes
  a pad, and every surface in the tree was written against it. A laptop is a
  keyboard and a pointer, a television box is a remote, a phone is a thumb and
  nothing else. The asset is already in place and was built for another reason:
  the panels keep their arithmetic in modules that have never heard of GTK, and
  `console-program-contract` takes a word in and hands back doings, so what a
  surface *means* is already separate from what pressed it.

  What is owed is that the contract has peers -- a keyboard contract, a touch
  contract -- and that each surface answers all of them, with the checks pressing
  each. The trap is the cheap answer: a layer that turns a keyboard into a
  pretend pad. It would work, it would be finished in a week, and every machine
  that is not a handheld would spend the rest of its life pretending to have
  buttons it does not have, in a menu shaped around a d-pad nobody is holding.

  One step of it is done and it is worth saying which, because the rest reads
  differently afterwards. A binding carries the input it is on, so a keyboard is
  rows in the same table the pad is in rather than thirty `hl.bind` lines in the
  compositor's file; the guide and the setup screen resolve against whichever
  hand was last used; and switching language means one thing on both keyboards.
  That is the *bound* half. What is still owed is the other half and it is the
  larger one: what a surface offers is still shaped by a d-pad and an A button,
  so a keyboard reaches every job and still walks a menu by pretending to be
  one. The trap named below is exactly that layer, and it is now half-built by
  accident rather than on purpose.

  This is the largest piece here and it is what decides whether anything but a
  handheld is real.

- **Stop building the operating system on the machine.** `[build]` compiles what
  the manifest names, on the device, out of the history that arrived, and that
  decision is a good one: nothing compiled travels, and what is on the machine is
  what somebody can read afterwards. It survives exactly as long as the machine
  is a handheld with a fan and a wall socket. On a fanless mini-pc it is a first
  boot measured in a battery, and on a phone it is not a design at all.

  The answer is to build once per architecture into a signed repository that the
  apply installs from, and it buys three things beyond the time. A compiler comes
  off a machine somebody carries. Architecture becomes a fact the build knows
  rather than a surprise the device meets. And the trust question stops being
  rhetorical: the engine currently believes a git remote absolutely, which is
  named above as the one unguarded edge, and a package that is signed is a
  signature something actually checks rather than an intention.

  What settles it: an apply on a machine with no rust installed.

- **Battery is a hardware fact and not an optimisation, which is why it is here
  and not under the note that says optimisation is last.** That note is right and
  nothing below contradicts it. What changes with a second kind of machine is
  that power stops being a number to chase and becomes part of what a device
  profile *is*: the platform profile and the governor it wants, whether the panel
  can refresh itself, which audio path keeps the machine awake, how deep a
  suspend the firmware really reaches and what is allowed to wake it. This
  device's own idle draw was never a slow program; it was a machine awake with
  its audio alive, and it took measurement on the machine to say so.

  So each profile declares its power answers, and a check reads them back on
  whatever machine it is standing on, in the same words on all of them: what the
  governor is, what suspend state was reached, what woke it. A kernel update that
  takes one away should turn a check red on the machine it happened to, rather
  than becoming a rumour about battery life.

  The comparison the wait store has been waiting for arrives here too. The same
  opening, timed on two machines, is the first evidence anybody has had that a
  slow surface is the code rather than the silicon -- which is exactly the
  question a number on one machine has never been able to answer.

- **What a check means when the machine is not in the room.** `--stage device` is
  one handheld over ssh, and `putting_back` is what makes that a borrowing rather
  than a taking. Neither generalises by itself: a supported machine with nobody
  holding one is a machine the checks make claims about. Either each shape has an
  owner who runs the device stage before a release, or the release says plainly
  which machines were pressed and which were only compiled for. The second is
  honest and cheap and should be written before the first release rather than
  after the first complaint.

- **A machine that skipped releases, and a machine that fails halfway.** Both are
  somebody else's morning. The sweeps are derived from the manifest's history,
  which is what makes them survive a machine that is several releases behind --
  but nothing has ever run a sweep for a name that left two releases ago, and
  that is a test, here, before it is a promise. And `previous.rs` says of itself
  that it is not a rollback and does not pretend to be one. For one device that
  is the right amount of machinery; for a channel it is the whole of what stands
  between a bad release and a machine that does not come up in a house nobody
  here can reach.

- **The security model is currently one private network and a remote nobody
  checks, and each thing that changes about that is a separate piece of work.**
  Signed history before the channel is already owed above and is the first.
  After it: the lock the machine has never had, on a thing whose nature is being
  put down on a table; encryption, which on a device with no keyboard means a way
  to unlock it with a thumb, and that is the on-screen keyboard running before
  the desktop does rather than a preference in a panel. The per-unit confinement
  is written and wants an evening on a machine to say whether the promises in it
  are true. And the stance that nothing leaves the device on its own is a
  security decision as much as a courtesy, so it belongs in the same page rather
  than only in the telemetry line above.

  The one that is new with a second machine: **an apply has never removed
  anything.** That is safe while one person remembers what a device is holding.
  On a stranger's machine an unswept name is a program still running that nobody
  believes is installed, which is the shape of most of the interesting ones. The
  gate makes the need derivable, which is why this can scale at all -- but the
  arithmetic has only ever been asked about a machine that took every release.

- **The public copy becomes the source, rather than a scrubbed picture of it.**
  `console-manifest-publish` builds a clean tree because there is a private one
  behind it. A distribution has no private one: the forks it carries as binaries
  have to be a package built from source somebody can read, the leak check
  becomes a property of the only tree there is, and the host that is deliberately
  checked for less when it is unset stops being a laptop's convenience.

- **The phone is a different program sharing a spine, and it goes last for a
  reason that is not difficulty.** Assume a handset with a mainline kernel, which
  means one somebody else already did that work for; anything else is a vendor
  tree and a second full-time job. Then the parts that have no analogue here: a
  modem, and telephony as an interruption. A call preempts everything on the
  screen, and nothing in the contract can say that -- a doing is carried out
  because a word arrived, and there is no urgency in the vocabulary at all.
  Adding one is a change to the thing every program on every machine stands on,
  which is the argument for doing the phone after two other shapes rather than
  before: by then the contract has been bent twice by machines that could not
  break it, and a third bend can be believed. A phone first would be the thing
  that proves the abstraction, with no way to tell which half was wrong.

  What it shares when it arrives: the manifest, the migrations, the palette, the
  program contract, the panel arithmetic and every check that does not need a
  thumb. What it does not share is the surfaces, and pretending otherwise is the
  same mistake as the pretend pad.

**What must not change while all of that happens, written here so it is not
argued again.** No config language in the manifest. No crate that is a shelf. An
apply still never removes, and the gate still derives what must be swept. Nothing
leaves a machine on its own. A check is still pressed rather than asked, and a
green one nobody doubts is still worse than a red one. The whole of this section
is more machines saying the same sentence, and not more sentences.

- write a library or find one for gtk4paintablesink so we don't need to rely on gstreamer, it's one dependency less that's sort of big
