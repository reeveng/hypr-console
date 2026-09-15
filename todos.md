# What is still owed

Small work, written down so it is not carried in somebody's head. Anything that
grows past a few lines and a reason belongs in `docs/` or in a check; anything
finished leaves this file rather than gathering a tick, and what a thing already
does is read from the code rather than restated here.

Say who owns a line if anyone does, and say what would settle it. A line nobody
can act on without the device says so, because the device is one machine and
there is usually somebody holding it.

The headings below are areas rather than states, so that picking one thing up
shows what else is in reach of the same afternoon. What each line is waiting for
it says itself: a thumb, a screen, an argument nobody has had. The two sections
at the end are a direction rather than work and keep their own order.

What the device is standing on is not written down here. It went stale on every
apply and was wrong more often than it was right, which made it a thing to fall
over rather than a thing to read: ask the machine, because it knows and this file
cannot. So a line that opens with `just deploy` says to deploy, and whether that
is already true is a question for the device rather than for this paragraph.

## The pad, the keyboard and the guide

- **Nobody has pressed the new keys on the device.** The alphabet walk is on
  Super, Shift and Space and on Super, Ctrl and Space; moving between windows is
  on HJKL as well as the arrows; the on-screen keyboard has left Super and K for
  Super and T. All of it is held by tests against the table and the rendering,
  and none of it has been pressed on a machine with a board attached -- which is
  the half that answers whether a chord is swallowed before it reaches the
  window somebody is typing in. Needs a thumb, a keyboard and the device: a
  check in `console-test-checks`, on `Body::Device`, that presses the pair and
  asks the compositor what each keyboard is wearing afterwards.

- **R2 and both triggers together are layers with nothing on them.** Putting
  something on one from Settings, **Buttons** is the other half of the same
  press, and the guide should grow a section headed **R2** the moment it has one
  row.

- **Two buttons on this device have no name, and the setup screen is what will
  give them one.** InputPlumber reports `Gamepad:Button:QuickAccess2` and
  `Gamepad:Button:RightPaddle3` off this machine's hidraw driver, and nothing
  here knows where on the machine either of them is; they read on a row as
  *quick access 2* and *right paddle 3*, which is a placeholder in
  InputPlumber's words rather than a name.

  Settings, **Buttons**, A on any row, and press whichever buttons on the
  machine you cannot account for. The card says what it was. Then the spoken
  names here become what is written on the device -- or what a hand would call
  it, the way `left-paddle-top` is -- and this line goes.

  While you are there: press A on the menu's row, press Y, and check that the
  card says two lines, *The menu is y* and under it *what else can be done with
  this has no button*. Moving a part onto a button another part is on used to be
  refused, which on this machine meant nearly every press. Then **Put every
  button back**, the first row, and check that everything is where it was.

- **InputPlumber cannot be asked to hold anything down, and nobody has written
  it up.** `SendEvent` on `org.shadowblip.Input.CompositeDevice` reaches
  `blocking_write_send_event` in `src/input/composite_device/client.rs`, which
  calls `blocking_send` on a channel from inside a tokio worker; the worker
  panics with `Cannot block the current thread from within a runtime`, the
  caller waits out the method timeout, and nothing is ever emitted. One line on
  the device shows it:

      busctl --system call org.shadowblip.InputPlumber \
        /org/shadowblip/InputPlumber/CompositeDevice0 \
        org.shadowblip.Input.CompositeDevice SendEvent sv \
        'Gamepad:Button:DPadRight' b true

  It is not a regression and the version it arrived in is not worth looking for:
  it does the same on 0.78.1 and on 0.79.0, `client.rs` is untouched between the
  two, and 0.79.1 does not go near it. What it costs is that a chord will not
  press a trigger at all -- `The event 'Gamepad:Trigger:LeftTrigger' is not a
  Button capability` -- and L2 held is the whole of how this desktop's second
  layer is reached, so every check under `carry`, `screenshot`, `brightness` and
  `volume` has its emulator half as the only one answering. A fix upstream takes
  the hold back for free, and the entry under this one is the only other way to
  it.

- **The way to the trigger layer is a device rather than a better call.**
  `SendButtonChord` is a mapping before it is a method, and what it cannot say
  is a key-down: it is press these together and let go, so `hold` and `release`
  saying they cannot is not the daemon refusing this desktop, it is this desktop
  asking a tap to be a hold. `SendEvent` is the call that means what we mean and
  it is the one that panics, so the wrong call is in use because the right one
  is broken.

  What would answer it is a device. InputPlumber's shape is sources in, a
  profile, one target out, and a source is matched by config, which this
  repository already owns: one more entry under `source_devices` in
  `50-legion_go.yaml`, matching a uinput pad by a name nothing else has, and a
  check's press goes through the composite device and the loaded profile and out
  the same target a thumb's does, without the hardware and its driver. It would
  also be a pad this end can hold down.

  What it costs is a config on every machine naming a pad that exists only while
  a check runs, and a program in `[build]` to make it and unmake it, with a
  `Lifetime` for the run somebody interrupts -- which is the objection
  `console-test-stages`' own head raises against a second pad, and it is a real
  one. Whichever way it goes, `LET_GO` stays where it is: it is what pressing
  through the front door costs today.

- **Nothing has pressed the claim by hand.** The half a machine can answer is
  green: the keyboard comes back across twenty restarts, and it types into a
  page with the pad grabbed beside it. What needs a hand, and has not had one:

  - Both sticks walk the keyboard's highlight, and the d-pad still walks it. No
    check presses a stick at a keyboard.
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

  One avenue was closed without being tried, and it is here so it does not look
  untested: InputPlumber's composite device carries a `dbus0` target with an
  `InputEvent(s,d)` signal, and a reader of it would get presses without opening
  the pad at all. That is the wrong shape -- what these programs need is not to
  *see* presses but for nothing else to see them, and a signal cannot take a
  device away from anybody. It would still be the better way to *watch* the
  front of the machine, if something here ever wants that.

- **The stick scrolls slower.** Slower was a first guess. On the device it is
  either enough, or it wants a curve rather than a speed -- slow near the middle
  and quick at the edge, which is what a stick is for.

- **The paddle on the back scrolls down.** A notch a press, and it keeps turning
  while it is held. Whether the notch is the right size and whether holding it
  runs away are both things a finger says at once and a test cannot.

- **Thai reaches the numbers.** The rule is that the numbers key sits beside the
  language key on every arrangement, and a test holds each of them to it. What a
  test cannot say is whether the thumb finds it there.

- **Per-key latency is not measured, and a line per keystroke is not the way to
  get it.** The number somebody means by *the keyboard is slow* is per key, ten
  a second while a person types, and writing it the way everything else here is
  written would fill the store with the one surface it says least about. It
  wants sampling -- one key in fifty -- or a mode somebody turns on while they
  are looking. Deciding which is the work; the writing is an afternoon.

- **The guide opens on the wrong tab.** `opens_on` reads which hand was last
  used and answers Anywhere for a pad and Typed for a keyboard, so a guide
  raised over a chooser opens on Anywhere, where A is a click and R1 is a
  workspace. Both are false of the screen it was raised over and the true
  answers are one tab along under Menus. `panel::show` already takes the tab to
  start on; what is missing is the reading behind it, because the daemon knows
  what is in front on every press and the guide asks nobody. Settled when the
  guide raised with a chooser up opens on Menus, and the sweep in that flow asks
  the guide the way a person reads it rather than ordering the sections itself.

- **Touch and buttons, both, everywhere.** The rule this desktop is now held to:
  everything it offers has to be reachable with the screen alone and with the
  pad alone. Neither is a fallback for the other -- it is a machine that is
  held, so a thumb on the glass and a thumb on the d-pad are both the ordinary
  way to use it, and anything only one of them can ask for is a thing half the
  device cannot do. The home screen is written to that rule and the rest of the
  desktop has not been read against it. What is left is whatever a sweep turns
  up: every surface asked twice, once with the screen covered and once with the
  pad unplugged.

- **Nothing but the bar has been asked whether it answers a finger.** There is a
  way to press a place on the screen -- `console-tap`, and `Device::touch` on
  top of it -- and one check that uses it, on one icon. Every other surface is
  still only asked the questions that were asked of the bar the whole time it
  could not be pressed: is it there, is it the right size, is it on the right
  layer.

  The ones worth pressing are the ones where a finger is the only way in: the
  rest of the bar's icons, the panels' own rows and tabs, the keyboard's keys,
  the notification cards. The audit that goes with it is every surface that asks
  for `KeyboardMode::Exclusive` -- correct for a panel that is meant to be
  modal, and a surface that takes the whole screen's input away from everything
  else for one that is not.

## Panels and what they draw

- **Nothing measures what the panel host costs while it sits there.** The
  promise the unit's own comment makes is that a GTK loop with no surface mapped
  sits in `poll` and wakes for nothing, and RAPL on the device can say whether
  it does. Nobody has asked it.

  One press goes with it that a laptop cannot make: stop the unit and press the
  menu. It should still open, slower, and say on the journal that it drew
  itself.

  The open question underneath is whether a window is worth keeping. It is
  destroyed and built again on every opening rather than hidden and shown, which
  is deliberate -- a surface that survived is a surface holding a reading nobody
  refreshed -- but it means the layer surface and the first frame are still paid
  per opening. Worth deciding once there is a reading to decide it with.

- **What else a row offers is a button beside it, and no thumb has held it.**
  What a laptop cannot answer is the two questions a hand answers. Whether the
  button is the size a thumb wants at the device's own scale, where the card is
  narrower than it is here and the row lost that much of its width to make room.
  And whether right off a row reads as somewhere to go: on the rows that carry a
  level right has always set the level, and the mark at the end of the row is
  the whole of what says which of the two this row is.

  `just deploy`, then Files in the menu. Walk down to a file, press right -- the
  row should keep its ground and the **⋯** beside it should take the highlight
  -- then A, and the list of what can be done with that file should come up.
  Left, and B, should both put the highlight back on the row without leaving the
  folder. Then Music, **Playing**: the button there is in the corner of the card
  head, which is a finger's, and Y from the scrub or the transport is the
  thumb's, because those rows spend right on their level.

- **Tap the × on a panel with the keyboard up.** It did nothing when the panel
  was a stopped process for as long as the keyboard was up. Nothing is stopped
  now, so the × should answer a finger, and one tap is what says so. It needs a
  finger, because touch is not InputPlumber's to send: open a panel, press X,
  tap the ×.

  The × is also a place along the top now, and that half needs no finger, so it
  is one run of the pad: R1 to the end of the strip, one more onto the ×, then
  A. It should take the highlight while the tab in front goes quiet in mint, and
  the d-pad should step back off it into the list.

- **A GTK 4 popover has no background.** Seen in gnome-weather before it was
  swapped out: the search popover drew its border and its entry, and the page
  behind it read straight through where its own ground should have been. That
  reproduction is gone with the application, and kweather is Qt, so this needs a
  new one; `console-panel` and `console-files` are both GTK 4 and are the place
  to look. The palette does define `popover_bg_color`, as a reference to
  `@panel` like every other name, so either libadwaita no longer reads
  `@define-color` for this one or it cannot follow a reference. Worth knowing
  which, because every menu in every GTK 4 application on this machine is the
  same widget. Settled by a GTK 4 application with a menu in the nested desktop,
  and either a custom property beside the named colour or a rule for `popover >
  contents`.

## The home screen

- **The home screen holds the keyboard, and lets go of it.** The one thing on it
  a laptop cannot answer is the beat between: a panel opened from a button asks
  for the focus as it maps, which is before the socket has said it exists. If
  Hyprland does not hand the focus over when the home screen lets go a moment
  later, a menu opened from the paddle comes up and answers nothing. Press the
  menu paddle from the home screen twenty times and watch. Falling back is
  `KeyboardMode::OnDemand`, which costs a tap on the screen before the d-pad
  moves anything and cannot break a panel.

- **Nothing on the home screen says how to put something on it.** Somebody
  holding the machine could not find how to add an application, how to take one
  off, or how to move one from the first pane to the second. All three work, so
  this is not a feature that is missing, it is a feature nothing announces. The
  guide has it, and the guide is behind a button somebody has to know about
  first, which is the same shape as the fault the home screen was written to
  fix. What would settle it is the home screen saying it where it is being
  looked at: an empty square already draws as an offer while the highlight is on
  it, and that is the place a word belongs.

- **What folded onto a later pane has no easy way back, and nothing else about
  the arrangement is quick either.** Narrowing the grid folds the squares that
  no longer fit round onto the end, which is right -- dropping them was a way to
  lose applications. What has no answer is afterwards: grow the grid again and
  they stay where they folded to, with the first pane holding empty room.
  Staying put may even be desired -- a hand-arranged pane should not shuffle
  itself -- so the miss is not the folding, it is that gathering everything back
  together, or onto one pane, is a walk of pick-up-and-carry presses per square.
  Said while resizing on the device on 2026-09-04.

  The same session said the wider thing: moving one square between two others,
  or swapping a handful into a new order, is all holding A and carrying, one at
  a time, and it reads as work. What would settle the first half is one press
  that gathers -- the squares in their reading order, packed from the first pane
  -- offered where the shape is set, so it is a choice and never a surprise. The
  second half wants a shape nobody has designed yet, and it should wait for one
  rather than grow options.

- **An icon on a pale photograph is a shape with no edge.** The names have a
  shadow behind them and the pictures have nothing, and the plate under each
  square helps without finishing the job, because the plate is deliberately thin
  enough to read the picture through. What would settle it is the icon carrying
  its own edge rather than the square carrying more darkness -- a shadow under
  the picture the way there is one under the name, which costs nothing on a dark
  wallpaper and is what makes a light one work. Worth doing at the same time as
  deciding whether an empty square shows a plate at all.

- **The one Qt program on this device is a weather window, and the weather is
  already on the machine.** The argument in `desktop.conf` for `kweather` is an
  argument against gnome-weather and is still true of it: a GTK popover holds a
  grab, and the first key tapped on the on-screen keyboard closes the field
  being typed into. What it is not is an argument for Qt.
  `console-wallpaper::weather` already asks what it is doing outside, with
  `curl`, because the picture on the screen changes with it -- so the reading is
  here, and a window somebody else drew is what puts it in front of anybody.

  A weather card is the ordinary card, over a reading this tree already takes.
  What leaves with it is `plasma-integration`, which is on the machine only so
  that a Qt program opens `kdeglobals`, and `/home/@user@/.config/kdeglobals`
  itself, which `console-palette` writes for that one program. One surface takes
  two packages and a colour path with it.

  What to decide first is what a person wants of it -- the hours of today, or
  the week -- because what the wallpaper asks for is what a picture needs and is
  probably less.

- **The box that asks for the password is KDE's, on a machine with no KDE.**
  `console-polkit.service` starts `/usr/lib/polkit-kde-authentication-agent-1`,
  and the reason it is there is written in the unit: polkitd asks the session a
  question and gives up when nothing answers, so without an agent installing
  something is a button that does nothing and says nothing about why. It is also
  a pointer program with a text field in it, standing between a person and every
  install they will ever make, on a device whose only keyboard is a surface this
  desktop draws over the top of whatever is asking.

  An agent is a name on the session bus and a conversation: polkit hands over
  the identities it will accept, something asks the person, and the answer goes
  back. The surface is the ordinary card. What it wants that nothing here has
  wanted is a field that holds what was typed into it and hands it to one place
  -- which is the same requirement as unlocking an encrypted disk with a thumb,
  named under **More than one machine**, and the reason to write it here first
  is that it is the same field.

## Music

- **The two music modes and the search have not been pressed on the device.**
  The panel counts its presses of `Shuffle` and `LoopStatus` because kew took a
  set as a press of its own key without reading the value. The player this
  desktop owns reads the value it is set to, once, so the counting is a wrong
  answer to a question nobody asks -- worth taking out, and worth checking on
  the device first that it has stopped mattering rather than assuming it.

  Playing at all has to be watched again. The panel used to press A into silence
  because kew had never been told where the music is; there is nobody to tell
  now, the player is handed the folder on its command line, and the folder is
  read from a file of this desktop's own. So the first press of A after this
  deploy is the check: it either plays or it is silent for a new reason.

  `just deploy`, then Music in the menu. On **Playing**, take **Play them in any
  order** with something on: the row should turn into *Play them in the order
  they are in* with **any order** beside it, and the song after this one should
  not be the song after it in the folder. Then take **Play this one over** and
  let a song end. Both of them again to put them back, and once more from the
  player left repeating the whole list, which is the state the panel offers no
  way into and has to be able to come out of in one press.

  Y over a song in the folder is worth a press -- the files panel should open on
  Music, standing on that file. Y over the song *playing* is not, and will not
  be again: it read `xesam:url`, the player does not answer it, and the
  now-playing card offers nothing. That was asked for and is not a regression.

  Then **Music**. Arriving on the tab is what sends the library to be read, and
  the corner says how many songs that is. What is worth measuring is how long it
  takes on the device: it is minutes of ffprobe and it is the one thing here a
  handheld will feel. Press X and type an artist nothing is named after, and
  check the songs by them arrive -- until the reading has finished only the
  filenames answer, which is the difference to watch for.

- **Three things about the player want a hand on the device.** The whole chain
  was pressed here -- a song opened, position moving at the rate of the clock,
  next walking a library of three and coming round, pause, seek, shuffle,
  repeat, and a song reaching its own end and the next one starting -- but this
  machine is not the handheld and this machine is not the speaker.

  **Position runs ahead of the speaker.** Samples are counted as they are
  written, and what has been written is not what has been heard: the pipe holds
  what a pipe holds and pw-cat holds its latency. `sounding::AHEAD` takes off a
  bounded guess at that. Watch the scrub bar against the ears and see whether
  the number is right there.

  **Nothing is gapless.** A song ends, the pipe closes, and the next ffmpeg is
  started. Between two tracks of one album that gap is audible and it was not on
  kew. The shape that fixes it is the next song decoding while this one plays,
  which this design allows and does not do.

  **A seek opens the file again.** There is no telling a running ffmpeg to go
  elsewhere, so a scrub ends it and starts another at the offset. Whether that
  reads as a scrub or as a stutter is a question for a thumb on the bar.

  Then the ordinary pass: Music, a song, next five times -- five different
  songs, none twice, and the sixth press comes round to the one you started on.

- **The now playing screen wants the thumb again.** The card is three rows now
  rather than five -- the sleeve with the title and whose it is beside it, the
  bar with a clock under each end, and the strip -- because five things down the
  middle made a card taller than the screen it opens on, and what hung off the
  bottom was the strip of five, the one thing a hand came for. What a thumb has
  to say is whether the sleeve is still worth its half of the card beside the
  words rather than over them.

  One look goes with it: the strip says what is switched on in mint alone now,
  which took out `media-playlist-no-repeat-symbolic`, a name neither Adwaita nor
  breeze has. If that is what Papirus was drawing as a broken square, *the
  buttons look wrong* is settled with it.

- **The transport is reachable only by walking to it.** With the music panel
  open, next, previous, shuffle, repeat and play/pause should each have a button
  of their own: a hand carrying the machine wants the next song without reading
  the screen for where the highlight is.

  Nothing here is against it -- the presses are five MPRIS calls that already
  exist -- and the reason it is not written is that the router is one table of
  what a button means, keyed by what is in front, and *what is in front* is a
  panel rather than which panel. So this is either a meaning the daemon takes
  while the music panel is the surface in front, which wants the door to say
  which panel that is, or it is the media keys the pad already has going
  somewhere that answers them. The second is smaller and is the one to price
  first.

## The wallpaper

- **What the wallpaper does on the device is unconfirmed.** `just deploy`, then
  `wallpaper-press`, which fetches about two hundred megabytes and takes six
  minutes; then four things a thumb has to confirm.

  Open a window, and check the picture stops moving. Then open the settings and
  check it stops for that too: a menu is a layer surface rather than a window,
  and counting it is new. Close them and the movement should come back within a
  frame or two.

  While a window is open, `ps -o rss= -C awww-daemon` should read about 66 MiB
  whatever picture is up. That is the whole argument for pausing, and it is the
  one measurement that has only ever been taken by switching pictures by hand.

  Then the Wallpaper tab: turn following the weather off, pick a picture, and
  check it arrives at once rather than in five minutes. Three things to watch,
  because all three are new. The picture, or its still at least, should be up in
  about a second, because a pinned picture no longer waits on the weather
  service. The corner should say so while it happens. And the panel should
  answer the d-pad through the whole of it, because the pass is handed to
  `Showing::later` rather than waited for where the panel is drawn.

  Then drop something into `~/Pictures/Wallpapers` and take it up from the same
  tab. That press runs on the device and takes tens of seconds a picture, so the
  thing to watch is that the panel keeps answering the buttons while it happens,
  and that the corner says how long it is going to be.

## Files, downloads and what opens what

- **A kind of thing is a family of types, and the families have not been set on
  the device.** Each of the three settings used to take effect for exactly one
  type while the rest of the family scattered -- flac and ogg to Firefox, jpeg
  and webp and gif to Chromium, webm to Firefox, opus and mkv claimed by nothing
  at all. The panel writes the whole family now, so what is owed is one press
  each.

  After a deploy: Settings, **Configuration**, **Pictures**, choose Gwenview
  again -- the same answer that is already on the row -- and jpeg, webp, gif and
  the rest follow it. Then **Video**, choose mpv again, and webm and mkv follow.
  Music needs no press, because this desktop ships its own answer for it in
  `/etc/xdg/mimeapps.list`; the other two point at programs that are not ours
  and this tree should not be choosing them.

  Then the press that started it: a `.opus` file in the files panel, opened. It
  should be the music panel and not a browser window with a scrubber in it.
  `.flac` and `.ogg` are the same question and were the same fault.

  Worth knowing while in there: nothing on this machine runs
  `update-desktop-database` after an apply, so `mimeinfo.cache` is whatever it
  was. It does not matter for any of the above -- a named default in
  `mimeapps.list` beats the cache -- and it is why a browser was winning types
  nobody had ever chosen it for.

- **Nothing on the Download panel has been pressed on the device.** `just
  deploy`, then Download in the menu. Type a song with X, press A to walk off
  the line onto **Look for**, and press A again: the row should say **Looking
  for** it and the list should arrive with pictures. Press A on one, and the
  corner should say it is on its way into Music; a notification should say it
  has landed, and the row should say **have it** the next time that search is
  made. Then Y on a row, which should offer the video of the same thing and the
  browser.

  The one thing worth measuring while it is open is how long a search takes over
  the device's own network. Ten pictures are ten curls and ten ffmpegs; if that
  is slow enough to notice on the handheld, fetching them at the same time
  rather than one after another is where to start.

- **A screenshot is the one thing this desktop asks somebody else's program to
  do over Wayland.** `console-screenshot` shells to `grim`, and what grim
  contributes is `zwlr_screencopy`, a buffer and a file. The tree already speaks
  the rest: `console-input-pointer` binds the wlr globals, the keyboard draws
  its own surface against them, and `cairo-rs` is built here with `png`.

  Worth a package, and worth more for the second caller. A capture that is a
  crate can be asked for a region, for a window, or for a pixel, and the answer
  is a buffer rather than a file somebody then has to read back -- which is what
  the media viewer, a thumbnail and anything that ever shows a person their own
  screen each need and none of them can ask `grim` for.

- **What opens what is settled by shell scripts written for desktops that are
  not on this machine.** `xdg-open`, `xdg-mime` and `xdg-settings` are how a
  link, a type and a default browser are asked about here. Each is a walk
  through the desktop environments it might be running under, and this one is
  none of them, so each ends in the fallback. What this desktop actually uses of
  them is a `.desktop` file and a `mimeapps.list`: `console-applications` reads
  the first already, and `console-core-ini-files` reads the shape of the second.

  The reason to take it is not the package. It is that *what opens this* becomes
  a decision this tree makes and a check can press, which is what the tiers
  under **Where this is going** need it to be -- a program entered deliberately,
  with the machine saying so, is not an arrangement a script can be asked to
  make. The families entry at the top of this section is the same fault from the
  other end: a cache and a script disagreeing about a `.opus`, and nothing here
  owning the answer.

## The browser

- **The profile is chosen by a file that belongs to the person, not to this
  desktop.** `~/.librewolf/profiles.ini` is the list of every profile somebody
  has, and this desktop declared the `console` profile in it and marked it the
  default. That is how the browser arrives wearing the palette and holding the
  add-on, and on a machine with a browser somebody already uses it replaces
  their list with a list of one. So it moved to `machines.conf`, and the
  consequence is that a machine where this is a session beside another desktop
  installs the profile and then opens the person's own.

  What it wants instead is `--profile`. `console-browser` starts the browser
  through `gio launch` on the package's own `.desktop` entry, which is what
  makes this desktop's browser the one a person already has rather than a
  second copy of it, and that entry passes no profile. A Firefox-family browser
  started with `--profile <dir>` reads no profiles.ini at all, which also ends
  `MOZ_LEGACY_PROFILES` -- the switch in `hyprland.lua` that exists because an
  `[Install<hash>]` heading in that file outranks `Default=1` and the browser
  had quietly made itself another profile. Two workarounds for one fact, and the
  fact is that nothing was naming the directory.

  The part to decide is what it means for a browser that is not one of that
  family, since the Web tab offers several: chromium takes
  `--user-data-dir` and the rest take neither, so the answer is either a profile
  flag per browser in `console_default_applications::browsers` or a desktop that
  only carries its own profile for the ones that can be told.

- **Nothing the browser's add-on does has been pressed on the device.** `just
  deploy`, then start the browser -- and if it was already running, close it
  first, because a policy is read when a browser starts. **Console** should be
  listed on `about:addons` after that start. If it is not, the answer is in the
  Browser Console and it is almost certainly the signature: LibreWolf is built
  to install an add-on nobody has signed and `user.js` asks it to, and neither
  half of that has been watched here.

  Then a page with links on it. Press Y: every link should wear a label of
  arrows and a bar should appear along the bottom. Press the arrows written on
  one, and the page should follow it. Y twice, and the bar should say the labels
  open a new tab instead; take one and the page should say afterwards that it
  went behind this one. Then the d-pad with no labels up: a pink box should walk
  from link to link, A should take the one it is standing on, and pushing the
  stick should take the box away and give the pointer back. Then B, which should
  go back a page -- that is the promise this was written for and the first press
  worth making.

  Then the bar itself. **Look for something**, X, type, and take the row at the
  top: it should search in whatever engine the settings panel's Web tab last
  chose, which is the one thing here that is read off the browser rather than
  written down. **On this page** should count what it found and the d-pad should
  walk between the matches. **The tabs** should list this window's tabs with the
  one you are on in mint. And a new tab should open with the line to type into
  already there and already holding the keys, which is the surface nothing on a
  laptop has drawn at all.

  Two that are answers rather than presses. A finger held on a page should still
  get Firefox's own menu, because the pad is a mouse and a finger is a touch and
  the add-on tells them apart by what the event says about itself. A page inside
  a page -- a comment box, an embedded player -- should have no labels on it at
  all, which is the top-frame limit written down in `docs/browser.md` rather
  than a fault. If either of those is wrong on the device, it is the sentence in
  that file that is wrong.

- **The add-on's bars want a finger, and one decision nobody has taken.** Both
  bars carry a **×** at the right end now and the find's carries **↑** and **↓**
  beside its count, and all four answer `elementFromPoint` headless. What is
  left is the tap itself: on the device, tap the × on the labels bar, and with a
  find up tap ↑ and ↓ and then its ×.

  The decision is the bar's own ground, which passes a tap through to the page
  underneath it because `.bar` never claims the point and only the controls on
  it do. That is either right -- the bar is a thing to read, and the page goes
  on taking what is not aimed at one of its presses -- or it is a panel covering
  something that answers taps meant for it. Nobody has taken it deliberately
  either way.

  Written down so nobody reaches for it again: a highlight walked along the bar
  with the d-pad, the way the pink box walks links, is unbuildable. The bar
  exists only while the labels are up, and while they are up an arrow press *is*
  a label keystroke -- it either finishes a label or narrows the list. One press
  cannot both narrow the labels and move a highlight, so walking the bar means
  giving up the labels, which is the quick thing this was written for. The two
  are alternatives, not layers.

- **The add-on is at the top of what an add-on can do, and the rung above it is
  a browser this desktop draws.** What the pad reaches in a page it reaches
  because a WebExtension puts labels on the links from inside the page, and
  everything that is not the page is out of reach by construction: the tab
  strip, the address bar, the find bar and the menu are the browser's own chrome
  and no extension is allowed to draw there. The frame limit in
  `docs/browser.md` is the same wall from underneath. And the install rests on a
  browser being willing to take an add-on nobody signed, which is a LibreWolf
  build decision rather than a promise anybody made this desktop.

  The rung above is our own chrome around somebody else's engine, and for a tree
  that is already GTK4 the reachable one is WebKitGTK. A view in an ordinary
  panel; the tabs, the find and the address line drawn the way every other
  surface here is drawn and coloured from `theme/palette.toml`; the link labels
  moved out of an add-on and into a script this desktop injects, where nothing
  can refuse to install them and a frame is a decision rather than a limit. It
  answers the signature, the chrome and the frames at once, and it is the same
  card as everything else.

  **What it is not is an engine, and nothing here is going to write one.** Servo
  is the Rust answer to that sentence and is not ready for the sites a person
  actually opens; it is worth asking again in a year and is not worth a plan.
  LibreWolf stays the browser somebody chooses for the whole web and chromium
  stays the second engine for the site that will not work in it. What a browser
  of this desktop's own would be for is the reading a person does with a thumb,
  which is most of what this device is picked up to do.

  What settles whether it is worth starting is the pressing already owed above,
  on the device. If the labels and the pink box are right on the handheld, this
  is an improvement on something that works. If they are not, it is the answer.

## Voice

- **The comparison that decides the dictation model has not been run, and none
  of the three fixes has been near a microphone.** This is a morning of work on
  the device and nothing here can advance it. `voice-compare` runs as the person
  whose session it is -- not as root over ssh, because the microphone belongs to
  a PipeWire that belongs to a session:

      ssh <the user>@$CONSOLE_HOST
      cd /etc/console
      cargo run --bin voice-compare -- --record     # sixteen clips, one at a time
      cargo run --bin voice-compare                 # every clip through every model

  `~/.local/share/console/voice` on the device already holds `whisper-cli` and
  all four models, so `--models` and `--build` have nothing left to do. What is
  left is a voice.

  What decides it is the Thai, read by somebody who speaks Thai. Turbo is
  large-v3 with the decoder cut from thirty-two layers to four, and OpenAI names
  Thai as one of the languages that costs; meanwhile the saving is in the
  decoder, which on a two-second sentence is a hundredth of the work on this
  machine. So the model in use may be paying for Thai and getting nothing back.
  If the full one costs little and hears Thai better it is the one to keep, and
  the line in `docs/voice.md` calling turbo both the accurate one and the fast
  one needs a clause about which language it is not.

  Three things the recording answers that nothing here can:

  - `th-tones` -- one syllable at three tones, with gaps. If the models differ
    anywhere, they differ here.
  - `nl-mixed` -- Dutch with the English words left in, said the way it is
    really said. This is what pinning the language *costs*, and it is the one
    result that could argue for leaving the setting on *Whichever is spoken*.
  - Whether whisper writes `'s ochtends` with the apostrophe at all. The fix is
    right either way -- it is a no-op if whisper never emits one -- but nobody
    has seen its Dutch output.

  Also there while the directory is open, and still there on 2026-09-08:
  `ggml-small-q5_1.bin`, 190 MB, left over from the few hours the small model
  was in use. Nothing reads it.

- **Dictation types through a program that fires nothing a binding can be held
  against, and the keyboard beside it does not need one.**
  `console-input-dictation` hands what it heard to `wtype`, which is the last
  caller of it in the tree. `console-input-keyboard::typing` is already a
  `zwp_virtual_keyboard_v1` client, because
  `console-input-gamepad::vocabulary` had written down that `wtype` fires
  nothing by keysym -- so there are two ways to put a letter into whatever holds
  the focus on this machine, and the one that works is ours.

  What is owed is that typing into the focus is one place, which is the crate
  that already holds the protocol and the keymap, and that `wtype` leaves
  `[packages]` behind it. The keyboard uploads a keymap per alphabet, so this is
  also the only path on which dictation in a second language ever types what was
  heard.

  What would settle it: a sentence spoken into a field that is not the on-screen
  keyboard's own, on the device.

## The bar, and the row under it

- **Nobody has looked at the bell while notifications are held back.** Take
  **Keep them off the screen**, and check the icon is a bell with a line through
  it. Then take **Let them back on the screen** and check it goes back. The
  codepoint and the font are answered; the look is not.

- **Nothing crosses the glyphs the bar draws against the font the bar draws them
  in.** `300-every-icon-is-one-the-theme-has` does exactly this for the icon
  names a panel asks GTK for, because a name nobody has is a broken square. The
  bar's readings are not names, they are codepoints written into
  `console-status-bar::reading`, and a codepoint the font has nothing at is the
  same fault with the same ending -- the bell cost an afternoon of it, found by
  counting along an alphabetical run and landing on a bunk bed.

  What settles it is the check the bell was settled by hand: `fc-list
  :charset=<codepoint>` on the device, for every glyph the bar can draw, against
  the family the bar names. What it needs first is for those codepoints to be
  named in one place instead of written into the middle of the four readings --
  which is worth doing anyway, because `\u{f0084}` in an expression is not a
  thing anybody can read. `PLUGGED` is the one that made this worth writing
  down: asked of the device by hand, against the family the stylesheet actually
  names, and by nothing that would notice the day it stopped being true.

- **The strip is a general row that only one thing knows how to use.** That it
  is always there is settled and should not be revisited: a row that came and
  went would reflow everything under it twice per use, and a row that is always
  there is a place anything can draw a length. What is apply-shaped about it is
  only the naming and the path -- the file is `updating`, the module is
  `bar-updating`, the layer is `updating` -- and nothing about a bar that fills
  from the left is about applies.

  The second caller arrived without the renaming: `console-test-stages`'
  `watching` fills the same row while a device run walks the checks. So the
  protocol is already shared and only the words are not, and a row that says
  *updating* while something else is being watched is the fault this entry was
  written before. What would settle it is a name and a protocol that are not an
  apply's: one place a length and a word are written, one module that draws
  whatever is there, and the apply as the first caller rather than the only one.

  The callers after it are already written and are not hypothetical. The volume,
  the brightness and the battery each raise a notice carrying a value, and the
  card is filled to that proportion in the same colour the strip fills in -- so
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

  What was blocking it is gone. The card is `console-notify` now, so whoever
  draws it is also the one deciding whether to draw one, and *instead of the
  cards* is a decision this tree takes rather than a mode it asks a package for.
  `console_notifications::serving` is where a raise is turned into a card, and
  it already reads the value out of the hint -- a reading is a shape it can
  recognise and hand to the strip instead.

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

- **The panel and the bell read a copy, not the daemon.** `console-notify` holds
  what has been raised; `$XDG_RUNTIME_DIR/console/notices.json` is what it writes
  after every change, and the bar, the panel and the settings page all read that.
  It is the strip's own argument -- the bell wakes every time a notification
  moves, and a reading that costs a subprocess and a round trip is one the bar
  cannot take that often -- and it is why `makoctl list -j` is gone rather than
  translated.

  What it buys with that is a second copy of the truth. The daemon answers a call
  before it writes the file, so there is a moment where what is held and what is
  readable disagree; nothing on the screen has been seen to show it, and *has not
  been seen* is the shape of every reading this tree has had to go back and fix.
  `ExecStopPost` deleting the file on the way out is the same fault from the other
  end: a promise about a copy, kept by a unit file rather than by whatever made
  the copy.

  The interface of our own is the answer and the daemon already has half of it.
  `console.Notices` takes `ClearAll` and `Quieten`; what it has no word for is
  *what are you holding*, and there is no signal saying that changed. With those
  two, the panel and the settings page ask and the bell subscribes -- and it
  already watches this bus for the raising and the closing, so the subscription
  costs nothing new -- and the file stops being a thing that can be stale.

  What settles the order is whether the bell can afford a call per change on the
  same argument the strip makes about polling. If it can, the file goes. If it
  cannot, the file stays and the bus is what the *panel* uses, which is two
  readings of one truth again and worse than either alone.

- **`console-bus` says what `Notify` needs and no more.** Strings, the integers,
  arrays, `a{sv}` and variants: exactly `susssasa{sv}i` and the calls a
  notification daemon answers. That floor is deliberate. The alternative was
  zbus, and an async runtime and a proc-macro layer for one interface on a bus
  this desktop speaks to nobody else on is the trade this tree keeps refusing.

  A second caller is what would move it, and the second caller is not
  hypothetical: logind is asked whether this machine can hibernate, systemd is
  asked what units changed, the player is asked what is playing, and every one of
  those is a `busctl` fork today. What each would want past what is here --
  dictionaries keyed on something other than a string, structs read rather than
  walked over, `h` and the descriptors that ride beside a message rather than in
  it, properties as `Get`, `GetAll` and `PropertiesChanged` rather than as calls,
  and a client half, which does not exist at all: today there is the half that
  answers a call and no half that makes one.

  Signals first, because that is the piece that pays. `console-events` watches
  this bus by running `busctl monitor` and reading what it prints, for the bell
  and for the player both -- a program per pool and a format nobody promised. A
  match rule and a reader would replace it, and the crate that reads a message
  is already written.

  What it must not become is a bus library. The line is the one the daemon is
  on: what a caller in this tree actually asks for, spelled once, with the thing
  that asks it in the same commit.

- **Nothing that raises a notification here is a stranger, and the daemon is
  written as though that will hold.** No actions, no icon data, no fd passing,
  no hints past urgency and progress: `console_notifications::saying` names
  everything that raises one, so a card is this desktop talking to itself and
  can be trusted the way a panel's own text is.

  The day a program nobody wrote raises one, that stops. A summary is then
  somebody else's string on a surface that comes up over whatever is on the
  screen, and the questions are the tiers under **Where this is going**: what it
  may say, how long it may stand there, whether it may carry a button, and
  whether a card from outside is drawn to look like one from inside or is marked
  as somebody else's. The daemon truncates and escapes today because the text is
  ours and short; neither is a policy. Worth answering before the first stranger
  rather than after, because the answer is a shape and not a patch.

## Bluetooth

- **Nothing has yet been paired from the tab.** All of it is unit tested and
  none of it has met a radio. What settles it is a device that has never been
  seen from this machine, put into pairing mode and taken all the way from the
  row to typing, and then a reboot: the trusting is the half nothing on the
  screen shows, and a keyboard that comes back a stranger is how it would say it
  did not happen. Forgetting wants pressing too, and wants the row to be gone
  afterwards rather than standing there saying Connect.

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
  session switch is not one of them. `controller-desktop` holds the only
  `Hurrying` and does not run in Game Mode, so whether the hint goes back before
  Steam has the screen depends entirely on how that daemon is ended, and nobody
  has looked. The note in the runtime directory is what should catch it on the way
  back -- a session switch is not a reboot and the directory survives one.

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

  The argument is already in the tree once, in the other direction.
  `console-cpu-boost` raises the hint for about as long as an opening takes,
  because the whole of a panel opening is over before load-based scaling has
  noticed the load it made. Sustained work wants the opposite answer and the two
  do not contradict each other: hurry when nothing is sustained, hold when
  something is. What is missing is anything at all that tells those two cases
  apart.

  The choice stays where it already is. Three power profiles, the same three
  whatever `powerprofilesctl` says, and no fourth switch asking somebody to opt
  into the machine behaving well -- a list of options is a way of making a
  person responsible for arithmetic they cannot do from where they are sitting.
  Somebody picks how much machine they want; making that a number the machine
  can keep is not a preference. `More than one machine` reaches the same shape
  from the other end.

- **Nothing on this machine reads a temperature.** Not a crate, not a doc, not a
  line of the manifest. Heat is the one hardware fact this desktop has never
  asked for, and every decision above wants it.

  This is not a tidying job. The sysfs this tree already reads has one owner
  each and they have not drifted, so there is no second reading to go and
  collapse -- there is a subject nobody has written. Which also settles what it
  must not become: a general layer over the operating system, a `console-sysfs`
  every caller depends on growing a name each time something new is read, is the
  shelf `docs/crates.md` argues against at length. The shape of sysfs is a file
  holding a word, which `console_core_atomic_writes::read` already is. So this
  is named for what it does -- how warm the machine is, how much of its envelope
  has been spent, and what it could hold from here -- and it reads whatever it
  has to in order to say that.

  RAPL and the amdgpu hwmon both answer on this device and were measured on it;
  there is no `powertop` or `turbostat` here to lean on. Which of the two says
  what, and what a reading is worth on a machine whose fan curve nothing can
  read back, is the first thing to write down rather than the first thing to
  code.

  What settles it: a number, on the device, that moves when a game does.

- **The boost hurries with no idea what the machine is already doing.**
  `Hurrying` has one caller -- `controller-desktop`, in the daemon that reads
  the pad -- and it raises the hint on every core for three quarters of a second
  per press and puts it back. On an idle desktop that is the whole point of it. In
  the desktop session with a game in front, it is a person's own thumb on the
  stick making the clock swing underneath the thing they are playing, which is
  the boost and the fall this desktop should be the one preventing.

  The signal that tells the cases apart is already read, and already in this
  desktop's own words: `console-compositor` reads a window's `fullscreen` into
  `Filling`, and `fullscreen>>` arrives as `Stirred::WindowFilled`. The boost
  asking before it hurries is the second caller that reading wants, and the
  first one somebody would feel.

  What settles it: the same stick, in the same game, with the hint watched.

- **The manifest has never said the word Steam.** No `[packages]` line names it,
  and the manifest mentions it only where something else is carried along for
  it, while `console-applications` walks the Steam library roots to find what is
  installed and what its icons are. So the desktop already knows about the games
  on this machine and the manifest does not. One of those two is wrong about
  what is on the device, and it is not the one doing the walking.

  The reason it matters beyond the inventory: Game Mode is one road to a game
  rather than what a game is. Steam runs in the desktop session like anything
  else, and when it does this tree is the compositor, holds the pad and draws
  its panels over it -- so every question above belongs there, because there is
  where this desktop is the one deciding.

## The manifest, an apply and a deploy

- **Only the green half of the disk's room has ever been read.** Asked over ssh
  on 2026-09-08 the device says *1735 GB left, and an apply wants 6 GB of it*.
  What no green answer can give is the sentence when there is no room, on a
  device with most of a terabyte free. Point the arithmetic at a small
  filesystem to see it -- `console room --root` somewhere on a tmpfs -- and read
  the walk while it happens, because naming where the room went is a `du` over
  the person's home and it takes over a minute on this laptop. If it is a minute
  on the device as well, the hourly card wants a cheaper answer than the true
  one.

- **The standing-up-to-a-fault work has had its readings and none of its
  breaking.** Every one of these is about what happens when something goes wrong
  on a machine that is not here, so what is left in each is the breaking:

  **The restart drop-in.** Break one on purpose -- point `console-bar.service`
  at a program that exits at once -- and watch it go on retrying past five
  falls, at a widening interval, instead of going `failed` and staying there.

  **`console well`.** On a machine with nothing wrong it must say nothing at
  all: a card at every boot is a card nobody reads. Then give it something to
  find -- edit a file the manifest claims and do not apply -- and check the card
  names it and says `console check`.

  **The apply's two guards.** `console apply` should refuse on a battery below
  the protect step plus fifteen and say so in a sentence with the reading, the
  level and *plug it in*. And while one is running, `systemd-inhibit --list`
  should show `console apply` holding `shutdown:sleep:idle`; kill the apply and
  the lock should be gone with it, because the pipe closes when the process
  does. A lock left behind is a device that has quietly stopped being able to
  suspend, so this is the one to actually watch.

  **The processors.** Press something, and inside the three-quarters of a second
  after it `$XDG_RUNTIME_DIR/console/hurried` should exist and name every core.
  Then the real test: press something and kill the controller daemon inside that
  window. Every core is left at `balance_performance`; the daemon restarts; the
  note is what puts them back. `cat
  /sys/devices/system/cpu/cpu0/cpufreq/energy_performance_preference` should
  read `power` again a moment later.

  **The plan on disk.** Watch `/var/lib/console/laying` appear and go during an
  ordinary apply. Then the case it is for, which needs nerve: pull the power
  during the swap. The machine should come up and `console well` should say an
  apply stopped partway through and name the files that were in flight.

- **The hourly card is loud about a file the browser writes itself.** Asked on
  the device on 2026-09-08 it says *Changed since the last update:
  /home/@user@/.librewolf/profiles.ini*, every hour, about a file LibreWolf
  rewrites the way `console-scale` rewrites `bar.css`. Either that path wears
  `theirs` or the card goes on telling somebody about drift they did not cause
  and cannot act on, which is the entry further down about who this check is
  for, arriving as a live example.

- **A file a package owns is a regression that comes back on a schedule nobody
  chose.** Pacman owns `/usr/share/inputplumber/devices/50-legion_go.yaml` and
  takes it back at every upgrade of that package. The manifest names the file
  and has no answer but an apply somebody thought to run, so the touchpad goes
  to `blocked: true` again on a morning chosen by whoever cut the release. It
  will not be the last one: anything under `/usr/share` that `[files]` carries
  is the same arrangement waiting for its own upgrade. `50-legion_go.yaml` is
  deliberately not marked `theirs` -- a package taking a file back is the thing
  to be told about.

  What would settle it is the manifest holding a package to what it reinstalls.
  `[packages]` and `[files]` know nothing of each other, and the arrangement
  wants either a hook that reapplies a path pacman has just written or an apply
  that says which of its files a package now claims. Both are more than a mark
  on a line, which is why this waits.

  Also unpressed: the card itself, one look after the next apply.

- **A file dropped from the manifest stays on the device.** `console apply`
  writes what the manifest names and never asks what it wrote last time, so a
  path that leaves the manifest goes on living where it was installed. It has
  cost a session already: the file pinning which session logs in had stopped
  being ours, still sorted after the one the switcher writes, and quietly
  overruled every attempt to leave for Game Mode.

  Standing now: `asking.yaml`, which the claim crate took out of this tree and
  the device still holds, lying in exactly the voice the last one did -- anybody
  reading `/etc` to find out what happens while a program is asking a question
  is told about a profile nothing loads any more.

  **The record an apply keeps is not the one this wants.** 406ec0e gives an
  apply a list of what it laid down, so a release that will not run can be put
  back; that record lives for the length of one apply and is swept at the start
  of the next, because its whole job is undoing the run it belongs to. What this
  wants is the opposite -- what some earlier apply laid down that this one no
  longer claims -- which has to outlive every apply and be kept on the device
  across them. It deletes things, which is why it was left out of a rework that
  was already changing what "landed" means rather than built inside one.

- **Everything that writes except an apply still writes without asking whether
  there is room.** A download that ends in a file that will not play, a
  thumbnail that is half a picture, a panel that opens having forgotten what it
  remembered: none of them asks first, and `console_core_atomic_writes` only
  means the old file survives the attempt. Each is small enough that running out
  reads as the feature being broken, which is the argument for asking rather
  than against.

  And the sweep is a decision nobody has made. `target/` on the device is
  rebuilt by the next apply and is the obvious thing to take away, but an apply
  that has to compile the workspace from nothing is a long one, and nobody has
  measured which of the two costs more. What is known is that the handheld is
  nowhere near the line: asked on 2026-09-07, most of its disk is free and what
  has been spent is the games, with the checkout's `target/` a rounding error
  beside them. The urgency this entry was written with was borrowed from the
  laptop it was written on.

- **The build is one stretch that moves on a curve, and could be a stretch that
  moves on the truth.** Building is most of an apply. It moves the strip per
  crate along `steps / (steps + PACE)`, a curve that never arrives, because
  there is no honest total to divide by: how many crates a build compiles
  depends on what changed, and the apply that matters is the one after somebody
  edited one file. The front-loading is deliberate -- the question early on is
  whether anything is happening at all, and a bar that has not moved cannot
  answer it.

  The wish is for what the device tier now has: the longest work first, so an
  apply feels like it starts slow and then runs. Two halves, and only one of
  them is easy.

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
  kept, is the question to settle before any of the above is written. `lasting`
  is the shape it would take, and the store would live beside `checked` for the
  same reason.

- **The card a deploy raises has never been seen on the device's screen.**
  `exec` in front of the whole card meant the probe before it was what the shell
  tried to become, so every deploy asked at the terminal instead and the run said
  so. That is fixed and the composing is tested, and what is untested is the
  thing itself: the card coming up on the handheld, the deploy waiting for it,
  and a no there sending nothing. It is a `--stage device` check, because a
  laptop cannot see that screen, and until one is written the way to know is to
  deploy while holding the machine.

- **Deploying from a tree somebody else is working in should be a flag, not a
  recipe.** Several sessions share this checkout, so a tree with somebody's
  uncommitted work in it is the ordinary state rather than the exception, and
  every deploy from one is a handful of manual steps done from memory.

  The refusal itself is right and should stay: `just ready` runs *in the tree*,
  so a deploy from a dirty one would have the suite vouching for something other
  than what ships. What is missing is that the way around it is manual --
  `console-deploy` prints the recipe when it refuses, and it works exactly as
  advertised.

  So it should be a flag that does it. Clone `HEAD`, run the gates in the clone,
  deploy from it, clear it up afterwards. Two things it has to get right that
  the printed recipe does not say: the clone cannot go under the temp directory,
  which is a small tmpfs here and where a cargo build dies partway with errors
  that read like a broken shell rather than a full disk; and a fresh clone has
  no build behind it, so without a target directory kept beside it every deploy
  of this kind pays for the whole workspace from nothing.

- **`console well` says its piece on a timer, to whoever is holding the
  device.** Telling somebody using a desktop about drift they did not cause and
  cannot act on, on an interval, is the wrong audience for the right check. The
  check is worth keeping -- what is in question is who it is for. The shape to
  consider is that it goes on running and goes on recording, and what reaches
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
  press and saying when they are not the ones the commit describes. The awkward
  half is that a hand-install is by definition somebody working around the
  tooling, so the note has to be something the tooling can find rather than
  something the person has to remember to write.

- **What this links against wants the same list the programs it runs now have.**
  Every external program is a variant of
  `console_core_external_programs::Program` carrying where it comes from, and
  `desktop.conf`'s `[packages]` is held against it by a test. The system
  libraries have nothing of the kind: what a machine must have on it before this
  will build is a fact spread over a dozen `Cargo.toml` files and a manifest,
  with nothing crossing the two, so a library that is only there by accident is
  exactly the fault the programs no longer have.

- **Somebody else's crate is published without a way to say it is theirs.**
  `crates/console-resume` is carried in the public copy and `VENDORED` keeps its
  `LICENSE` travelling with it. What nothing checks is the other half of
  GPL-3.0's ask: that a modified file says it was modified. The crate's
  `authors` names upstream and `papers/forks.md` names the release, which is
  what a person would look for, but neither is a thing a copy carries into
  somebody's hands with the code. Deciding what would be enough is small and
  worth doing once rather than being asked about it.

- **A package is on this machine for a reason, and `[packages]` has never said
  which.** The enum says it from the other end --
  `console_core_external_programs::Origin::Package` names what a program comes
  from, and `every_package_a_program_comes_from_is_in_the_manifest` crosses that
  against the manifest -- and the same test file already asks the opposite
  question of the enum: `nothing_named_here_has_stopped_being_run` goes red when
  a variant stops being spelled anywhere. Nothing asks either question of a
  package.

  So `pavucontrol`, `blueman` and `network-manager-applet` sat in `[packages]`
  after the bar's icons stopped opening them and started opening `settings-panel
  Sound`, `Bluetooth` and `Wi-Fi`, and `brightnessctl` sat there after
  `console-brightness` took the rocker and the dimming. Nothing in `files/` or
  `crates/` had named any of them for a while. They have left the manifest now,
  and they were found by reading it, which is how `libpulse`, `grim` and
  `xkeyboard-config` were found going the other way.

  What is owed is the word that makes the question answerable, said on the entry
  the way `theirs` is said after a path under `[files]`. A package is here
  because a program runs it, because something links it, because it is data
  something reads, or because it is a program a person opens -- and only the
  first of those is derivable. With it said, the check is the one already
  written: a package whose reason is *a program runs it* and which no `Program`
  variant names is dead, and nothing else is guessed at. It is also the same
  word the tiers under **Where this is going** want beside a program, and *what
  happens when I open this* and *why is this installed* are one fact that will
  drift the day it is written twice.

  **The device is still holding every one of them, and nothing derives that
  either.** `console_manifest_migrations::holds` answers for `[build]`,
  `[files]`, `[services]` and `[masked]`, and `None` for a package, so a name
  leaving `[packages]` sweeps nothing and the gate cannot say so. That is the
  right answer today and not a permanent one: every sweep here is a move into an
  attic somebody can read afterwards, and taking a package off is `pacman -Rns`,
  which is a deletion. It is the one place where *an apply has never removed
  anything*, under **More than one machine**, is already true rather than a
  plan.

  What would settle the check: `brightnessctl` put back in a branch, and it
  going red.

## Checks, stages and flows

- **The viewer's finger tests are red in a workspace run and green on their
  own.** Six of them stand a nested compositor up each, and `cargo test
  --workspace` starts them beside every other nested session in the tree: the
  same six pass alone in twenty-four seconds and four of them fail in a
  forty-five-second run, saying the card drew once when it should have drawn
  twice. Nothing about the feature is in that difference -- it is contention for
  the machine, and the assertions that go are the ones that wait for a surface to
  be drawn. So `just test` is a gate that is sometimes red for a reason that is
  not the code, which is the thing this tree says a check must never be.

  What it probably wants is for a nested session to be a resource a run holds one
  of at a time rather than one per test, which is a change to `console-test-desktop`
  and not to the tests. Until then, a red one of these is worth rerunning alone
  before believing it.

- **The device says it was handed back, and three of the ways it puts things
  back have never run.** A device run presses only what nothing else can answer,
  so `brightness_to`, `volume_to` and `close_window` were skipped along with the
  checks that would have disturbed them. Each is the reason this entry stays.
  `brightness_to` writes the backlight through `tee` and a glob, where every
  other write to that file goes through `console-brightness` and its range;
  `volume_to` sets a percentage back through `pactl`, and a level that lands a
  point off what was read would be reported as stuck rather than put back;
  `close_window` ends a window by killing the pid `hyprctl clients` names for
  it, because nothing in this tree has ever asked the compositor to close a
  window it does not have in front -- if `hl.dsp.window.close` turns out to take
  an address, that is the better call and this becomes one line.

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
  themselves. The other half is the deploy: an apply ends, the one question
  anybody has afterwards is whether it worked, and the machine that could answer
  it is the one in somebody's hands.

  So: when an apply finishes, the device asks on its own screen whether to check
  that it went well, and the person answers with a button. Yes runs the device
  tier there and fills the same strip; no goes away and does not ask again about
  that apply. That inverts who decides, which is the rule this repository keeps
  everywhere else about that machine -- a clean tree is not permission, and
  neither is a finished deploy.

  What it needs that does not exist: `console-check` is not in `[build]`, so the
  binary is not on the device at all. The piece to think about first is what a
  run is allowed to do to a desktop somebody is using -- the tier opens menus,
  moves workspaces and closes windows, and consent to a question is not consent
  to that. Probably the answer is a smaller tier: the checks that touch nothing.

- **Nothing has pressed the session being put back on the machine it is for.**
  `370` and `380` are the nested desktop's answer and cannot say where a window
  lands: it is moved onto the workspace it was saved from by the loop that
  listens for windows opening, and what that listens to is this desktop's event
  pool, which a nested session does not run. So the device's own `Body::Device`
  half of `370` is still owed: save with windows on two workspaces, restart the
  session, and assert each came back where it was, at the size it was if it
  floats. That is also the only stage that can press the two things this port
  changed for upstream's sake -- a special workspace that is not the first one,
  and a floating window's size.

  The other one only the device can answer is the unit rather than the program:
  `systemctl --user restart console-session.service` with windows on the screen,
  and the windows still there afterwards. `380` presses the same decision by
  starting the program twice; what it cannot press is `Restart=always` and
  `ExecStopPost=console-fell`, which is how the fault arrived on a screen.

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

- **The flows past the second are still prose.** Pictures then a film, the
  evening of music, the home screen's rearranging, and being interrupted are
  written down in `docs/flows.md` and walked by nothing. Settled when every flow
  on that page names the test or the stage run that walks it.

  The home screen's is the next one that can be walked here, and it is the only
  one of the four that wants nothing new: the daemon's half of waking, standing,
  opening and sleeping is already visible at the fast stage. The other three
  each wait on a line below -- a fixture folder, a player that answers off the
  device, a restart that can be watched.

- **`Here` sees the daemon's decisions and no surface.** A flow at the fast
  stage can say the viewer was asked for, not what the viewer drew, so the drawn
  half of every step waits for the desktop stage. `docs/programs.md` is the seam
  already planned -- a program as a pure function of what it has heard. Settled
  when one program (the viewer's reel or the music panel's playing tab is the
  natural first) answers a flow step headless through that shape.

- **A scenario can press and wait but cannot expect.** The files under
  `scenarios/` are recordings, and the only thing asserted about one is that it
  still parses and still presses. Either flows own their assertions in Rust and
  scenarios stay recordings, or the scenario language grows a line that says
  what should now be true. Decide once, write the decision into `docs/flows.md`,
  and settle it when one flow does whichever it is.

- **The desktop stage can look but cannot press.** The nested desktop is
  photographed and the emulator presses, and no flow can do both in one run.
  Settled when a thumb-script plays against the nested desktop and a photograph
  is taken at a named step of it, in the same invocation.

- **The music flow has no player to talk to off the device.** It could have one
  now: the player is a crate in this workspace rather than a fork installed on
  the handheld, and it answers `OpenUri` and position on any machine with ffmpeg
  and pipewire. What is missing is the flow itself. Settled when the playing
  tab's two promises -- the song survives the panel closing, and the reopened
  panel agrees with the ears -- are asserted in `just test`.

- **The viewer flow has no folder to walk.** Stepping from a photograph onto a
  film wants a fixture folder of things that weigh nothing and decode
  everywhere, kept in the tree. Settled when the reel steps through it in a test
  and the step onto the film is the same test's next line.

## The shape of the code

How the workspace is measured rather than read. The tools are on this machine
and nowhere in the manifest: they are somebody's laptop, not the device, and
none of them is wanted by a build.

    tokei crates --sort lines                   what is big
    cargo machete                               dependencies nothing asks for
    cargo modules structure -p <crate>          what a crate is made of
    cargo clippy --workspace --all-features -- \
      -W clippy::cognitive_complexity \
      -W clippy::too_many_lines                 what is long or knotted

Two of them lie in ways worth knowing before believing an answer. `cargo
machete` matches on the package name, so `cairo-rs` reads as unused in every
crate that writes `use cairo::`; the finding is false and there is no config
that fixes it. `similarity-rs` compares functions within one file and never
across two, so it says nothing about the question below, and the cross-crate
work was measured with a script instead. `rust-code-analysis-cli` does not build
on a current rustc and is a dead end.

Neither `clippy --message-format=short` nor `cargo dylint` prints a lint's name
in its short form, so a summary that greps for the name reports a clean
workspace when nothing was clean. Grep the message text.

- **Two crates here differ by one letter, and the one with the `s` is about to
  lose its reason.** `console-panel` is the card that gets drawn and
  `console-panels` is the program that stays up and draws whichever one is
  asked for, and neither name says which is which. It is the one name in this
  tree a reader cannot recover from by thinking harder: a wrong `use` compiles,
  and what the compiler complains about afterwards is a missing item rather
  than the wrong crate.

  The rename is the smaller half. `console-panels` exists because the two
  largest stretches of any opening were `exec` and the toolkit coming up, so it
  opens the display once, parses the stylesheet once and reads the icon theme
  once on behalf of all of them -- and the entry under **Where this is going**
  about taking GTK out is aimed at exactly that number. What a resident program
  is still saving once a surface is a connection and a buffer is not known, and
  the timing file is what will say. So this is not a rename to schedule: it is
  a question that work answers, and the name depends on the answer, because a
  program that is already up is a different thing from one that is holding a
  toolkit.

- **The rule for a binary name reaches `[build]` and stops, and four tools here
  break it.** `the_binaries.rs` asks it of the programs the manifest installs,
  which is where a stale name costs a person something. The binaries a crate
  builds for this laptop are not in that list and were never held to anything:
  `capture-devices` and `allow-uinput` out of `console-input-gamepad`,
  `voice-compare` out of `console-input-dictation`, and `second-chooser` out of
  `console-panel` each open with a word their crate does not hold.

  What is not obvious is whether they should. A tool run once a year from the
  repository root is reached for by a person reading `justfile` or `ls
  crates/`, not by a unit, so the argument that a part says which crate it came
  out of is weaker here -- and `console-emulate`, `console-desktop` and
  `console-rename` already take the command shape without anybody having asked
  them to. Either the rule widens to every `[[bin]]` in the tree and those four
  are renamed, or it says out loud that it covers what is installed and why
  that is the line. Whoever takes it decides which, and the test says so either
  way.

- **A panel cannot carry out `Doing::Listen`, and the card that would ask has
  nowhere to ask about.** Every card matches `Doing::Listen(_) |
  Doing::Deafen(_) => {}` and no card has ever emitted one. What is owed is a
  `Listening` held by the panel with its words pumped onto the main context, and
  it is second rather than first: the music card wants `Player` and the
  notifications card wants `Notices`, so the card that asks comes with the topic
  it asks for and the panel plumbing lands under both.

  Two more go with it. Nothing yet hangs up when it stops being looked at. And
  none of this has been near the device: the pool is a daemon, and what a daemon
  does when it is restarted underneath a panel is not readable from a laptop.

- **The one subscription under the four compositor watchers has never been
  broken on a screen.** What it costs to have one is that the bar's two door
  icons now depend on a second daemon, and `bar-door` is the one with no tick
  underneath it: it draws when it is told and never otherwise. `Heard::GotIn` is
  what stops a gap leaving it wrong -- the pool says *you are in* on every
  reconnection as well as the first, and that word means *ask again* to a watch
  whose words already mean that. It is asserted in `the_deafening` and it has
  never been watched happen: what is owed is `console-events` stopped and
  started under a running bar, on the device, with the launcher opened during
  the gap. There is no check for the door icons at all yet, which is the other
  half of why this cannot be believed from here.

  The nested desktop can now hold half of that. It started waybar and the
  keyboard and no pool, so every module that reads through one was up, correct
  about what it said at startup, and never told anything after -- a bar that
  looks right in a picture and is not. It starts the pool before the bar now,
  which is how `bar-door` was first seen to write a line at all. What the stage
  still cannot do is stop the pool and start it again under a bar somebody is
  looking at, because a staged session's daemons are backgrounded rather than
  units, so the reconnection stays the device's question.

- **A few functions run past a hundred lines, and one past clippy's cognitive
  threshold.** The clippy line above lists them;
  `console-input-keyboard/src/bin/keyboard.rs` is the worst on both counts and
  is the only one that trips both. Nothing here is a bug, so this is a line
  about reading rather than about correctness.

- **`//!` was left standing and has not been held to the same rule.** Module
  heads survived both passes untouched, and EXPLICIT020 does not judge them:
  whether a head earns its place is a reading, and a lint cannot do a reading.
  By the rule the gate now keeps, most of what they carry is prose that belongs
  in `docs/` or nowhere. Whoever takes this should read them rather than strip
  them: with the `///` gone the heads are the only prose left in the tree, and
  some of them are the only remaining record of why something is the way it is.
  Those want moving before they want deleting. What the second sweep took out is
  not lost either -- it is in the commit that took it, and a head being written
  now is the right moment to go and read what that file used to say.

  The crate renames of 2026-09-04 gave this a second half, and that half is
  done at the crate roots: every `lib.rs` and every `main.rs` now opens with a
  sentence that says what the thing is and agrees with the name it has, which
  is the same test the names were held to. Two crates opened with *The menu*
  and one of them was the list rather than the surface; three opened with an
  instruction rather than a thing.

  What is left is the modules under those roots, one head at a time, deciding
  whether what is there is a decision worth keeping or prose that goes to
  `docs/`. There is a mechanical part of it worth doing on the way past: a
  reflow somewhere in the tree's history took the blank line out from between
  the paragraphs of a good many heads, leaving a wall of text with two spaces
  where each break used to be and a `##` heading welded to the sentence above
  it. `console-cpu-boost` and `console-input-focus` were put back by hand; the
  rest are found by looking for two spaces after a full stop inside a `//!`.

- **The film half is four packages of somebody else's pipeline for one
  paintable.** GTK is packaged here with no media backend, so
  `console-media-viewer` builds a GStreamer pipeline that ends in
  gst-plugin-gtk4's sink, and `[packages]` carries `gst-plugins-base`,
  `gst-plugins-good`, `gst-libav` and `gst-plugin-gtk4` to make it draw. What is
  wanted out of all of that is one thing: decoded frames handed back as a
  paintable a picture widget can take.

  Whether that is a library this tree writes over ffmpeg -- which is on the
  device already, and already decodes for the music player -- or a smaller one
  somebody else wrote, is the question, and it wants pricing rather than
  assuming: a decoder is where the hard parts of playing a film actually live,
  and the pipeline is also what holds the film to the clock.

- **The `beside us` walk is written out in four places and now has a home.**
  `console_core_our_programs::Ours::at` looks for a program of ours beside the
  binary that is running before it looks on `PATH`, which is what
  `console-input-keyboard::asked::keyboard`, `console-test-stages` and
  `console-test-desktop::staging` each do for themselves with their own
  `current_exe` match. Folding those onto it is small and wants doing when
  somebody is next in one of them; what it needs first is a variant per program
  they reach for, and the `[build]` cross-check in
  `console-manifest-engine/tests/the_programs.rs` is what keeps that list honest.

- **A number with a unit is the third of the three, and it is still unwritten.**
  024 and 025 were the first two: a representation doing a type's work, said
  about two parameters the compiler would swap and about a `MAX` standing in for
  a case. The third is the one `os_design.md` needs. Bytes, pages, ticks,
  nanoseconds, budget shares, queue slots -- and further out a virtual address,
  a physical address and a device address, which are one machine word and three
  different things. Here it is smaller and already on screen: geometry is
  pixels, scale factors and margins, and they meet in
  `console-core-number-conversion` because it reaches every screen this desktop
  draws. That crate is where a unit type would land and it is also the one whose
  call sites would all move. There is no rule to write until somebody decides
  which quantities this tree actually has -- that decision is the work, and the
  lint after it is small. 024's sweep is most of the answer already: what a
  newtype was written for there is a quantity somebody has named, and the ones
  it named are on the shelf waiting to be told what they are counted in.

- **`Alongside`'s drop is an effect another process can observe, and it has no
  name.** Two rules pull opposite ways here and the tree has already picked one
  without the argument being had. *A resource's lifetime should be structurally
  apparent* is why `Alongside` exists at all: a child dies with whoever started
  it, by a death signal and by drop, so neither a killed parent nor a path
  nobody thought about leaves it behind. *A destructor does local cleanup and
  nothing another context can see* would say the opposite -- that killing a
  process is an operation and wants naming at the place that means it.

  The second is the rule `console-program-contract` is built on: a program says
  what it wants done and nothing in it touches a machine, which makes a drop
  that reaches out of the process the one place this tree does what it forbids
  everywhere else. The answer may still be `Alongside`. What it does is undo
  what its own scope made, which is the only effect a destructor is allowed
  under any reading of the second rule, and the paths nobody thought about are
  the whole reason it was written. It is worth settling in words rather than by
  omission, because it is `os_design.md`'s I6 arriving early: an outcome is
  delivered or withdrawn, and a withdrawal that happened because a value went
  out of scope is still one somebody has to be able to name.

- **A softer comment rule was proposed, and taking it means 020 moves back.**
  What was asked for: comments and documentation only at declaration
  boundaries, no inline comment at all, and a comment on a declaration that
  explains the contract, the invariants, what it does to ownership, how it
  fails or why it is the way it is -- rather than one that narrates what the
  body does. If a fragment needs explaining, the fragment gets a name and
  becomes a function.

  It is not a rule to add. 020 already keeps the whole of the first half and
  keeps it harder: the `//!` head stays, a `// SAFETY:` stays, and everything
  else goes, including the `///` on a declaration that this one would hand
  back. So what is on the table is 020 softened, which is the move the suite
  has said it does not make.

  Whether it is a good relaxation is worth having out rather than settling by
  the ratchet alone, because what it has going for it is real. 020's answer to
  an argument that belongs to one function is the module head, and a head is
  the whole file; the entry above about the heads is that answer going wrong,
  prose collecting at the top of a file because there is nowhere the size of a
  function to put it. A rule that let an argument sit where its subject is
  would empty those heads back down to what a head is for.

  Two things cost more. The exception is a place rather than a kind, and every
  comment can be moved into it by going up a line: three lines above a `match`
  become three lines above the function the `match` is in, legal, saying the
  same thing about the same code -- and 020 exists because that is exactly what
  happened while the rule was prose. Then the second clause, which is the half
  that would stop the first from rotting, is a reading and not a lint. No gate
  can tell a contract from a narration; 020's own head says as much about the
  heads it declines to judge. So what would actually be enforced is the
  permission, and what would be left to memory is the discipline -- which is
  the arrangement the rule was written to end.

  The half worth having is the half 020 already gets by being absolute: with no
  comment on offer, a fragment that needs explaining has to be given a name,
  which is what the proposal wants. A function whose argument will not fit in
  its name is usually a module, and a module has a head. Leave 020 where it is,
  and read a head carrying function-sized prose as an argument for smaller
  modules rather than for `///`.

## Where this is going

_A direction rather than work. Every entry below is larger than this file's own
rule allows and each one wants a page in `docs/` before anybody starts it; what
is written here is the argument for the order, so a piece picked up out of turn
is picked up knowing what it is standing on. Nothing below is a plan to rewrite
what is here. It is what stands between a desktop that works for the person who
built it and one that works for somebody who did not._

**What this is not is a thing anybody else can have.** It is one device,
deployed by a push from one laptop that knows its address, built from a rolling
base with no way back, configured before first light by whoever built it, with
no answer at all for the morning it does not come up. Each of those is a
separate piece of work and they have an order.

- **A program has to be a function, and the front of the machine is what is
  left.** `docs/programs.md` argues this at length and is right; everything
  below -- a rollback that can be trusted, a first run that cannot half-happen,
  a recovery surface that has to work on the worst day the machine has -- is a
  promise about behaviour under conditions nobody can reproduce by hand, and a
  program whose state is a file six others write cannot be promised anything
  about.

  **`controller-desktop` is what is left, and it is deliberately last.** Its
  decisions are already a `Doing` and its tests are already transcripts, so the
  wrapping is small; what is not small is that it is the whole front of the machine, it
  publishes a virtual device, and it reads its pad *during* the turn through
  `Plugged` rather than being handed what arrived. Moving it means `came()`
  reading the devices, which is the same change as the input reader, so the two
  should land together and be pressed on the device rather than reasoned about.
  `console-home` and the keyboard daemon are the other two, and both are toolkit
  programs of the panel kind rather than loops.

  `voice-compare` is the one program still owed something by the contract, and
  what it wants is how long a `Doing::Ask` took, which nothing can be told.

  `offers()` is stage 6 and is deliberately unbuilt: the document's rule is that
  it waits until two programs want it, and there is one place a program reaches
  across to another -- the music panel opens the files panel standing on the
  song, which `Doing::Start` already says. A registry written now would be a
  shape guessed from one example.

- **The bar is the one surface on this device that somebody else draws.** What
  waybar contributes to the things this desktop says about itself is a layer
  surface, a stylesheet, and a loop that reads a pipe; every word on it is
  already ours.

  **What that arrangement costs is paid in three places, all of them above in
  this file.** What an icon means crosses out of Rust as a word in JSON and is
  matched again in a stylesheet, and a stylesheet that names a colour nobody
  defined paints nothing while every other answer says it worked. The stylesheet
  is read once at startup, so writing the palette does not reach a bar that is
  up. And nothing on the bar can say how long it took, because the moment a
  label appears happens inside a loop this repository does not compile.

  What a bar of our own would be is the readings this tree already returns drawn
  on a layer surface, coloured out of `console-palette` at the moment of
  drawing, and woken by `console-events`. One process instead of one per module,
  each with its own watcher, is most of the reason to do it at all. It is not
  what a panel's statelessness argues against: a panel is asked for and should
  be born when it is asked for, and a bar is always there by definition, so one
  resident program drawing it loses no state property because there is no
  opening to be stateless across.

  **Two modules on that bar are waybar's own and would have to be answered
  first.** The workspaces are a question `console-compositor` already knows how
  to ask and a row of buttons; that is an afternoon. The tray is a
  StatusNotifierItem host -- another program's menu, drawn in our surface, over
  a protocol nothing else here speaks -- and it is the whole of the work in this
  entry. So the first thing to find out is whether anything on this device ever
  puts an icon in it. If nothing does, the tray is dropped rather than written.

  **What the bar's own answer costs is written down, and has now been read.**
  `bar-door` times the stretch it is responsible for -- a word on the
  compositor's socket, a question asked, a line printed -- with the settle
  deliberately outside it, and writes a line only when the icon actually
  changed. Nothing had ever produced one: the nested desktop ran no event pool,
  so the watch had nothing to hear and the door was silent on the one stage that
  could have shown it. With the pool started before the bar, opening the
  launcher writes `who: bar, what: door` beside the launcher's own opening, and
  the two can be read off one file. What is still unmeasured is the half in
  front of it -- waybar reading that line and drawing it, in a loop this
  repository does not compile -- so the number is the floor rather than the
  whole, and it wants a run on the device before anybody argues from it.

  **The bar of our own is written, and is not yet what the manifest starts.**
  `console-bar` is one process: a layer surface along the top with a reserved
  zone, the readings this crate already returned, the workspaces the compositor
  already knew how to be asked for, and a strip in the last rows of the same
  surface that fills while an apply runs. Every measurement in it is a share of
  the screen's height rather than a number of points, which is what the
  gradient ladder and the `min-width` `console-scale` rewrote at every login
  were standing in for. What is left is the swap: `[build]`, the unit's
  `ExecStart`, waybar out of `[packages]`, the bar's three files out of
  `[files]`, and the sweep in `migrations/` in the same commit as the removal.

  **Two of waybar's gestures are dropped rather than carried over.** Right-click
  to mute and scroll to change the volume, both on the sound icon. Neither is
  reachable by a thumb on a handheld with no mouse, and a tap on that icon opens
  the sound tab, where both of them are a row.

  **The bar still links GTK, and nothing on it uses GTK.**
  `console-status-bar` depends on `console-panel` for `running::said`, so the
  binary pulls libgtk-4 in and never initialises it. It is the same question as
  the panels' own and is owed the same answer at the same time; what it is doing
  here is making the one process that is always resident carry a toolkit for one
  sentence.

- **An apply is not a transaction, and on a handheld it has to be.** A failure
  partway leaves a machine that is neither what it was nor what it was asked to
  be, and nothing on the device can say which. On a laptop that is an afternoon;
  here it is a person holding a thing that will not come up, with no keyboard,
  no terminal and no idea what the word "manifest" means.

  **We should not become immutable, and this is the entry that says why.** The
  reason SteamOS, the rpm-ostree family and the embedded slot-swappers are
  immutable is that they had no description of the machine, so the only way to
  know what a root filesystem contains was to ship it whole -- and each pays for
  it in a currency this device cannot afford, a package that cannot be installed
  or a host change that means a container. We have the description. What
  immutability buys -- *this machine is exactly what was intended and can be put
  back* -- is what `desktop.conf` and `migrations/` already claim, and the
  missing piece is not a read-only mount, it is that an apply has no identity
  and no previous.

  So: an apply becomes a **generation**, numbered, recorded on the machine with
  the commit it came from, and an apply that dies partway is a generation that
  never confirmed rather than a machine in an unnamed state. The snapshot half
  is in. What is owed is the identity: nothing on the machine records which
  generation is running, nothing confirms one, and nothing counts a failed boot.

  And the boot menu that makes a root snapshot reachable is
  `limine-snapper-sync`'s, which is on the device because CachyOS put it there
  and is not named in `[packages]` -- so a device rebuilt from this manifest
  would take the snapshots and have no way to boot one. Naming it is a claim
  about the bootloader that has not been thought through, and it is the next
  thing here.

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
  the fault `210` was written for, arriving from a direction that does not have
  a person nearby to notice it.

- **Nothing can say what the machine does when it does not come up.** This is
  the gap that most deserves the word *operating system*. There is no surface
  below the compositor. Hyprland failing, `console.target` failing, a disk that
  filled, a generation that will not confirm: every one of those is the same
  black screen and the same silence, on a machine whose only input is a pad and
  whose only output is a panel someone is holding.

  What is owed is a small program that needs nothing this desktop provides -- no
  compositor, no toolkit, no session -- and draws on whatever the kernel has
  left. It says what fell, in the words `console-say` already uses, and it
  offers exactly the choices a person can act on: go back to the generation
  before this one, or turn it off. It is reached from a button held at boot,
  because a person who cannot get to a desktop cannot get to a menu on it.

  It is small, it is the least fun thing in this file, and it is the difference
  between a device somebody can own and a device that has an owner on call.

  **What it would be drawn with is half in the tree already.**
  `console-input-keyboard` binds its own Wayland surface and draws itself with
  cairo and pango -- no GTK anywhere in it -- so a surface made of a buffer and
  a loop is a shape this repository has built once and can build again. What is
  missing is underneath it: with no compositor there is no Wayland at all, and
  what is left is the kernel's own -- a DRM device, a mode set, a buffer handed
  to it. That is the one part of this nobody here has written, and it is also
  the only path to any surface that is not GTK, which is what every entry about
  the toolkit eventually arrives at. The least fun thing in this file is the one
  that buys the most.

- **GTK is the last thing on this device that decides something this tree has
  already decided better, and taking it out is subtraction rather than a
  rewrite.** What a toolkit decides is what fits, what order a press moves in,
  what a press means and what colour a thing is, and every one of those is
  answered here already, in a place a screen cannot reach. `fitting` and
  `shape` are a share of the room and a whole number of rows, asserted against
  the device either way up without opening a display. `page` says what a page
  is -- the tab and the rows under it, and what each row says, does and has
  beside it -- which is the description a renderer would otherwise have to
  invent. `Ring` is the order a highlight moves in, written so a step cannot
  fail. `Meaning` is what a press does, an enum, exhaustive by 016. The toolkit
  sits on top of all of it holding a second copy in widgets, and it is named in
  one file of `console-panel` and in a handful of lines in each panel crate;
  everything else in them has never heard of it.

  **The contract already asked for this and was refused by GTK by name.**
  `console-program-contract`'s head says the runtime redraws from the state,
  which is the whole reason a state is asked to be `PartialEq`, and then gives
  up the one loop a paragraph later: GTK draws on one thread and will not be
  driven from somebody else's, so half the programs here keep their own. That
  concession is what this costs today. Taking the toolkit out is what lets a
  panel be pressed by `transcript`, on a laptop, with no screen and no
  compositor -- which is the one thing the contract promises and the panels
  cannot do.

  **What is not being built is a toolkit.** The tempting shape is elements with
  an environment -- measure, layout, draw, an affordance named per interaction
  -- and it is a second general layout engine written in this tree's own words.
  It was looked at and put down: this device has one arrangement, a page of
  rows of a settled height, and an engine for arbitrary arrangement is a larger
  thing than the one it replaces. An element holding its own mutable state is
  also what `docs/programs.md` exists to forbid, arriving dressed as a widget.

  **What replaces it is the list of shapes this machine can ever draw**, closed
  and matched over: a tab strip, a row, a letter standing over a run of them, a
  field to type in, the strip along the bottom, a card that asks, a picture, a
  film. Whether that list actually closes is the first question and is asked of
  every `Showing` there is, before anything is drawn. If it closes, drawing is
  a match, and 016 makes a shape added later announce itself everywhere it
  matters.

  **Pango is an input to layout and not part of it, and that boundary is the
  design.** How tall a wrapped line is, is the one thing placement cannot work
  out for itself. Measure into the description first and place purely
  afterwards, and the placing stays something `Body::Here` can check against
  stated sizes while the measuring is checked where there is a screen. A
  measurement kept in the state is the first thing that would undo it.

  **What gets answered before any of the rest is 2.5.** Every number in this
  tree is logical points at `DRAWN_AT`, and this device's panel is screwed in
  sideways at a scale that is not a whole number. GTK is what currently absorbs
  that. The one `set_buffer_scale` here is the keyboard's and it takes an
  integer, which is the one thing that cannot say 2.5, and neither
  `wp_viewporter` nor the fractional-scale protocol is anywhere in the tree. So
  the first piece of work is `console-notify` drawn on a surface of our own --
  one window, wrapped words, no rows, no highlight, already ours end to end --
  with the screencopy client written before it rather than after, so the old
  card and the new one are compared in a picture instead of by eye on a laptop
  that is not the device.

  **The order inverts the obvious one, and the entry above is the reason.** The
  panels work today and nobody is held up by them being somebody else's. What
  is missing is the surface for the morning the machine does not come up, and
  that surface is these same shapes drawn on a buffer the kernel gave us
  instead of one a compositor did. So: a surface of our own, the screencopy
  client, the shapes spiked as the notification, then the recovery surface,
  then the bar, and the panels last. Taken that way the promise this whole
  section is about closes early, and the most enjoyable part of the work is the
  part that waits.

  **What is rented stays rented, written here so it is not argued again.**
  Pango, because shaping is not a decision and because Thai is a layer this
  desktop wears unless it is told otherwise: a script with no spaces between
  its words wraps by dictionary, and a text engine of our own would break the
  one feature the on-screen keyboard exists for. Wayland, because it is not a
  dependency but the way to every program nobody here is going to write, which
  is the whole of the **Dressed** tier below. The decoders, by the rule
  further down this section. Hyprland is the one that is genuinely takeable --
  it holds the input and the window list, which is where the entry below about
  crossing ambiently has to be enforced if it is ever enforced at all -- and it
  comes after the generations, the recovery surface and the first run, and it
  is a Smithay compositor rather than one written from nothing.

  This wants a page in `docs/` before anybody starts, and settling the shape
  list and the measuring boundary is what the page is for.

- **The manifest has never once been built from nothing, and three times it has
  been wrong in the same way.** `libpulse`, `grim` and `xkeyboard-config` were
  each on the device because the base install or something else pulled them in,
  each was reached for by something here, and each was found by accident rather
  than by a check. Every one of them would have been found in a minute by a
  machine built from `desktop.conf` alone and then asked to do the thing.

  So there is a stage this repository does not have: a body below `Here` that is
  a fresh machine -- a container or a virtual one -- brought up from the
  manifest and nothing else, and run against. It cannot answer what a screen
  answers and it will never see a pad, and that is fine; what it answers is the
  one question nothing else asks, which is whether the file that claims to be
  the whole truth about this device is the whole truth.

  What would settle it: `xkeyboard-config` removed from `[packages]` in a
  branch, and the stage going red because a keyboard with no keymap to compose
  says so and stops.

- **This is one device, and an operating system is a thing other people can
  get.** The deploy path is a push into `/etc/console` from a laptop holding an
  address in its environment, which is exactly right for the person who wrote it
  and is not a distribution. Three things stand between here and somebody else
  having this, and they are separable.

  **An image**, built from the manifest the same way an apply is, so that the
  first thing a machine runs is the same statement every later apply is made
  against. **A channel**, so a device updates itself from something signed
  rather than from a git remote it trusts absolutely -- see the entry on that
  below. And **a first run**, which is the one below this.

  The order matters and the temptation is to do the image first because it is
  the visible one. It is last of the three. An image of a machine that cannot
  roll back is a way to give a stranger a brick.

- **The desktop begins before anybody has seen it, and there is nowhere to say
  who you are.** The network and which of the applications a person actually
  wants are still decided by whoever built the device, in this repository, for
  one person. What is missing is the moment rather than the settings: a setting
  somebody has to go looking for is one that a person who does not know it
  exists never finds, and the whole argument for the Language and Setup tabs was
  that nobody should have to know.

  A first run is that moment. It is the same card as every other surface here,
  driven by the same buttons, and it is the first thing a person will ever use
  this device for, which makes it the one surface where being slow or unclear
  costs the most.

  The trap, and every desktop has fallen into it: a first run that asks for
  things the machine could work out, or that cannot be gone back through, or
  that has to be finished before anything works. It asks for what only a person
  knows, it can be left, and what it does not get it asks for again at the
  moment it matters.

- **Every program here that this desktop did not write is either worth having or
  worth replacing, and nothing says which it is.** The list is short and it
  sorts cleanly. What decodes, hears or fetches -- ffmpeg, whisper-cpp,
  GStreamer, yt-dlp -- is somebody's decade of work against formats nobody here
  will ever see the whole of, and is never worth taking. What is data -- the xkb
  symbols, the locale names, the fonts, the icons -- is not a program at all.
  What the compositor does for itself, hypridle and hyprsunset, is argued for in
  `desktop.conf` and stands.

  What is left is a handful of small programs that each do one thing this tree
  already knows how to do, and the entries above name them one at a time:
  `wtype` against a console-keyboard client that is written and running,
  `grim` against protocols the pointer crate already binds, `kweather` against a
  reading the wallpaper already takes, `xdg-utils` against a table this desktop
  should be keeping itself, and the polkit agent against a field the disk unlock
  wants anyway. None of them is a rewrite of anybody's work; each is a call site
  that stops leaving the tree. `mako` was one of them and is done: the card is
  `console-notify` and the name is ours.

  **The order they are worth doing in is not the order of size.** Each one taken
  moves a program out of the tier that means *nobody dressed this* and into the
  tier that means *ours*, which is the entry below, and each leaves a piece
  behind that something larger needs: a screencopy client, a typing client, the
  table of what opens what. The one already taken left `console-bus` and a bus
  name this desktop owns, which is the entry above about a second caller. The bar is the largest of
  them and has its own entry above. `pamac` is the one to leave alone, because a
  shop is not a small program and being able to install things is the least
  dressed-up promise on the device.

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
  escape hatch rather than a place. This repository's answer has been to write
  its own, and that answer is right for the ones it has taken and does not scale
  to the ones left. **Firefox OS and Ubuntu Touch are the warning and they are
  not distant.** Both had better ideas than the thing they were replacing, both
  decided the way to a usable device was for the platform to supply the
  applications, and both died of the gap between what they had written and what
  a person wanted to open. Nobody here is going to write a Signal.

  So the direction is a policy rather than a program, and it has three tiers.
  **Ours**, which keeps the contract. **Dressed**, which is a foreign program
  the desktop makes reachable from outside -- the browser add-on is the proof
  this is possible, and the pointer is not the only lever: a program can be
  given the pad by something that knows what its widgets are, exactly as a page
  is. And **pointer**, which is a program nobody has dressed, entered
  deliberately with the machine saying so, rather than arrived at by pressing A
  on a row and finding the rules have changed.

  Two things fall out of it. The tier belongs beside the program in
  `console-core-external-programs` and `[packages]`, where the rest of the truth
  about a program lives, so *what happens when I open this* is a fact about it
  rather than a thing a person discovers. And `offers()` is what lets somebody
  else dress an application without changing this repository, which is the only
  version of an ecosystem that a device with one author can have.

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
  machine and nothing mediates it. The notification name is a name, so anything
  can speak in the desktop's voice, which is the voice faults arrive in.
  Everything runs as the one user with the whole of a home directory. And
  `$XDG_RUNTIME_DIR/console/` holds the ownerless variables `docs/programs.md`
  is about, writable by anything, which makes them an integrity question and not
  only a correctness one.

  **The tension is real and has to be said before the answer.** This desktop
  works by crossing exactly these lines. The add-on drives a page from outside
  it. Dictation types into whatever holds the focus. The keyboard takes the pad
  away from everything. Dressing a foreign application, which the entry above
  asks for, *is* one program driving another's interface. An isolation rule that
  forbids those forbids the machine.

  **So the rule is not that nothing crosses, it is that nothing crosses
  ambiently.** Every crossing is declared, named, narrow and revocable, and
  anything not declared is refused. That is not imported doctrine; it is the
  fourth time this repository has reached for the same move. `[packages]` is
  what may be installed. `console_core_external_programs::Program` is what may
  be run. `means.rs` is what a button may do. And `files/etc/sudoers.d/console`
  is the purest example already in the tree. The manifest is a capability list
  that has never been called one.

  **`console-program-contract` is what turns that from a hope into a boundary,
  and the monitor is the part that is missing.** A program that says what it
  wants done rather than doing it is a program whose effects can be held against
  a declaration before they are carried out, which makes the loop the one thing
  that has to be trusted rather than every program.
  `console_program_runtime::run` carries out every `Doing` it is handed and
  holds none of them against anything. What it does have is the one place to put
  the check.

  **Ours is confined and theirs is not, and the drop-in that takes it back off
  is the reason.** The units that start a program a person chose -- the bar, the
  home screen and the session -- carry a drop-in undoing the floor, because
  `systemd-run --user --scope` does not fork through the manager: it execs in
  the caller's own namespaces, so a scope started inside a unit with
  `PrivateTmp` sees that unit's empty /tmp. A sandbox on the bar is therefore a
  sandbox on Firefox and on Steam. What would have to change first is that a
  chosen program is started by the manager rather than by us, which is a
  transient unit with the environment handed over rather than inherited, and it
  is not an afternoon. `Doing::Start` is already that shape, and it is also the
  settling of the open item where everything opened from the menu lives in the
  controller's control group.

  Confining what somebody else wrote is the larger half, and what makes it
  possible without making an application useless is portals -- and
  **`xdg-desktop-portal` is not in `[packages]` at all**, though the device is
  holding it and three backends besides, every one of them there because
  something else pulled it in. A machine rebuilt from this manifest would have
  no portal, and this one has four packages' worth that nothing here chose,
  asked for or knows the version of. The trap that goes with it is worth naming,
  because half-done containment is worse than none: an application confined with
  the home directory handed to it is theatre, and theatre is believed.

  **The threat model, so the work is sized honestly.** One person, one device,
  no untrusted local users, nothing here defending against somebody holding it.
  What is real is what the person installs and opens: an application from the
  shop, an extension in a browser, a file fetched into Videos by a panel written
  for fetching files. The unusual one is that this device *compiles its own
  operating system* out of a git remote nothing verifies, which is the same edge
  the signing entry below is about, arriving from the other side.

- **Two sessions, and only one of them can own the machine.** The desktop is
  ours and Game Mode is Steam's, and the machine leaves for it entirely. That is
  the right call and the seam is honest, but everything with a life longer than
  a session sits on the wrong side of it: a download that finishes in Game Mode,
  an update that wants a reboot, a notification raised while Steam has the
  screen, a wallpaper that changed with the weather. Today those are simply lost
  or simply late.

  The direction is that the desktop is the machine and Game Mode is a thing it
  runs, in the sense of who owns state rather than who owns the framebuffer.
  What is owed is small and specific: what a person is told when they come back,
  and what was decided while they were away.

- **Nothing here teaches, and a guide somebody has to go and open is a manual.**
  What is missing is that `console-button-guide` is a place rather than an
  answer. A person meeting a surface for the first time has a question about
  *this* surface -- what Y does here, whether B will lose what they typed -- and
  the guide answers about the machine.

  Y is the lendable button and the guide is what it is lent for on a surface
  with nothing else to offer. A hint at the moment of need, in the surface, in
  the words the table already holds, costs nothing to keep true because it is
  read from the same line the daemon carries out.

  The other half of teaching is that nothing here is ever a first time twice.
  What a person has already done is not written down anywhere, so the machine
  cannot stop explaining, and a machine that keeps explaining is one people
  learn to press past without reading.

- **The interface has one language and one size.** Thai can be typed and cannot
  be read: every word this desktop says is English, written in the source. The
  Language tab makes that visible rather than fixing it -- a person can put the
  machine in Dutch and get a Dutch browser and a Dutch calendar with rows that
  are still English. The size of everything is one number, the screen scale,
  which is a real control and a blunt one, because a person who wants larger
  words is asking for larger words rather than for less of the folder. There is
  no contrast setting, nothing is read aloud on a machine that already has a
  microphone, a hearing and a way to type what it heard, and the dictation
  points in one direction only.

  This is the entry most likely to be deferred forever, so the line worth
  holding is narrower: nothing new writes a sentence into a program. Where the
  words a surface says come from is a decision that costs nothing while there is
  one language and is a rewrite of every panel later.

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
  edge: nothing checks who wrote the history. For a laptop pushing over a
  private network that is fine; for a channel, or for a second device, it is the
  whole of the security model and there is none. Signed commits or signed tags,
  checked by the engine before it builds anything, is the small version and it
  should land before the channel does rather than after.

- **Optimisation is last, and here is the note that says why, so nobody has to
  argue it again.** Everything above is about a machine that behaves the same
  way twice: after a restart, after a bad update, in somebody else's hands, in a
  language nobody here reads. A faster version of a machine that does not do
  that is a faster version of the problem, and every hour spent on a number is
  an hour not spent on the reason the number was being looked at.

  The two exceptions were exceptions because they are shape rather than speed --
  how a panel is entered, and one pool holding the subscriptions -- and both
  were taken while they were still cheap. Nothing else here is owed that
  argument.

  When the time comes, the wait store already knows where to look, which is the
  whole reason it exists.

- **The doings are two thirds of an effect type, and the rules are standing in
  for the third.** `Doing` is a description of what a program wants done,
  carried out by a runtime and pressed with words by `transcript`, which is the
  shape an effect has: what a call returns, how it fails, and what it needs to
  run. Two of those are already written down here. 001 through 005 say a failure
  met is a failure said, 038 made every crate's faults a type of its own, and
  `Result<T, Never>` is a call that cannot fail saying so out loud.

  The third is not written anywhere, and 026, 039 and 044 are what stands in for
  it: a function that reads the environment, the clock, or a value the process
  shares has an input nothing handed it, and the only way this tree can say so
  today is a rule denying the call. Said as a type it would be a parameter --
  `console-cpu-boost`'s `asked(now)` is that already, and its binary does the
  reading -- and a check could then hand a clock that does not move rather than
  arranging the machine's. 041 is the same thing about the other direction:
  `Doing::Print` is on the list and a `println!` beside it is an effect nobody
  declared.

  043 is the piece that cannot be a rule at all. A topic listened to and never
  deafened is one half of a pair, and the rule asks the question of the crate
  because there is nothing for the obligation to attach to -- `Doing` is a value
  handed across a loop that does not own the connection. A subscription that
  must be given back is a type this tree already knows how to write:
  `console-program-lifetime` holds exactly that shape for a child, where the two
  ways it can end are the two variants and there is no third.

  What this wants before anybody starts is a page in `docs/` arguing which
  doings a program declares and which the runtime does on its behalf, and
  whether the quantities belong in the same place. That last part is the half of
  the suite borrowed from nowhere: 026 says a name is read *once* and 029 says a
  program is not run *per item*, and an effect type as they are usually built
  has no way to count. Those two rules are the argument this tree would be
  making that the others are not.

## Installing it somewhere else

- **A package has no origin, so one name a machine's repositories have not got
  stops the whole apply.** Every missing package goes to pacman in a single
  command, which is right -- one transaction, one set of dependencies resolved
  together -- and it means a name nothing carries is not a package missing but
  "target not found" and an apply that stops before it writes a file. `pamac-aur`
  and `freetube` are that, and they are in `machines.conf` now for a reason that
  is about repositories rather than about the machine.

  What it wants is the shape `[files]` already has with `theirs`: a word beside
  the name saying where it comes from, and a machine with no way to reach that
  origin saying which package it cannot have and going on with the rest.
  `console_core_external_programs::Origin` is already the place that word belongs,
  since it is what a crate shelling out to a program declares. The hard half is
  what an origin that is the AUR means -- no helper is installable from a
  repository, so the first one has to be built from a PKGBUILD, which is a
  machine step this manifest has never had.

  And the half under it, which is the user's own framing from the session this
  came out of: what one person has installed has never been separable here from
  what the desktop needs. freetube is a want; pamac is arguably the desktop's,
  because the menu has a row for it. A list of what *this installation* also
  wants, beside the manifest rather than inside it, is the thing that would let
  the manifest stop carrying either.

- **Whether the person can read the pad at all is still undeclared.** The
  controller daemon opens `/dev/input/event*` itself, and on this laptop those
  are `root:input 0660` with no ACL for the person logged in -- a compositor gets
  its devices through logind rather than through the filesystem, so a desktop
  that reads them directly needs something the manifest does not yet say. On the
  handheld it works, and the likeliest reason is that the account is in the
  `input` group, which nothing in this tree put it there. `/dev/uinput` is the
  same fact one step worse, and `94-console-uinput.rules` now answers that one
  by name.
  
  What is missing is the general form. A rule per device is the shape the
  touchpad and uinput rules already have, and it only works for devices this
  tree can match on; a pad somebody plugs in is claimed by InputPlumber, whose
  own rules decide. The alternative is for the manifest to be able to say *this
  person is in these groups*, which is a section it has not got and the first
  thing it would have to do to a machine that is not made of files. Decide which
  before the next machine, because the symptom is a desktop with no keys and a
  unit restarting every two seconds.

## More than one machine

_Also a direction rather than work, and it stands on the section above rather
than beside it: the channel, the first run that cannot half-happen, the rollback
that can be trusted and the signed history are named there already, and every
one of them is a prerequisite here rather than a neighbour. What is new below is
the axis this tree has never had. One manifest describes one machine, and there
is no way in it to say `here, but not there`. Everything about handhelds,
consoles, laptops and phones is downstream of that one sentence._

**What is Legion Go by name, so the size of it is not guessed at.** The
InputPlumber device file and the profiles beside it, `console-input-gamepad`'s
`go.rs` and the capture under `fixtures/`, the monitor block that says a panel
is mounted portrait and turned a quarter, the backlight and battery paths, the
boost, and the sysfs file the rumble is silenced through. Each of those is
right, and each is written as though there could only ever be one machine, which
until now was true.

- **A second machine before a second kind of machine.** The laptop this checkout
  is edited on is the cheapest possible second, and the point of it is not the
  laptop: it is that `desktop.conf` learns to vary at all, while the person
  holding both can see what broke. The thing to protect while it happens is the
  migrations arithmetic. It is four sets over the manifest's own history, and it
  is the reason nothing has to be remembered; a name that can leave one variant
  while staying in another is the first thing that could quietly make it answer
  a question nobody asked.

  What settles it: a second machine that applies this manifest and comes up, and
  `cargo test -p console-manifest-migrations` still going red when a name leaves
  either variant unswept.

- **Variance is selection, and must never become a config language.** The
  manifest is a statement about a machine. A variant is another statement, and
  the engine picks between them by asking the machine what it is -- what the
  firmware calls the board, what the device tree says -- rather than by a
  conditional somebody writes in a file. The moment the manifest can compute,
  the thing that makes this tree worth copying is gone: a machine stops being
  readable as a description and goes back to being a program whose output is a
  machine.

  Whether that is a section heading per shape, or a manifest per device that
  names a common one, is a page in `docs/` before it is a line of code.
  Whichever it is, `console check` has to keep meaning *this machine has drifted
  from what it says it is*, and not *one of several things it might be*.

- **Somebody else's edit to a file the manifest owns, carried across an update
  rather than overwritten.** A file has two answers here and no third: ours,
  which an apply writes whole and `install::State::Differs` reports until it
  does, and `theirs`, which is never compared at all. That is enough on this
  checkout, because whoever edits a file also holds the tree -- `console save`
  takes the change back and a commit makes it the manifest's own sentence. On a
  machine whose owner has no checkout there is no save to run and no commit to
  make, so a line they changed is a line the next update destroys, and the only
  advice the tree can offer them is not to change it.

  The user's framing, and the reason this is written down: the three values a
  three-way merge wants are already on the device and none of them is new
  storage. The base is `files/<path>` at the commit `/etc/console` stood on when
  the apply ran, ours is the file that is live, theirs is `files/<path>` at the
  commit that just arrived -- and the device holds a clone, so the base is a
  checkout away rather than a thing to start recording. What reaches the machine
  is the history, which is what makes this cheap here and expensive anywhere
  that ships an image.

  `~/Documents/projects/pidvcs` is the shape of the answer and worth reading
  before designing a worse one. `merge(base, ours, theirs)` there hands back a
  state and the questions it could not answer, and its ladder refuses far more
  than git does: it declines when two edits are merely *near* each other rather
  than only when they overlap, which is exactly the bias a config file on a
  machine nobody is sitting at wants. Its `diff.rs` is a dependency-free Myers
  diff, and that is the smaller thing owed either way -- `differs` names a path
  and has never said what in it differs, which is what the hourly card would
  need before it could tell anybody whether the drift was theirs.

  The larger half of that tree is the half this one has no answer to at all.
  Files have identities there and paths are relationships, so a file that moves
  keeps its history and a rename is replayed over content that never saw it.
  Here a manifest that moves a file writes a migration that sweeps the old path,
  and whatever the person had written in it goes with the sweep -- their change
  is lost by the mechanism that exists to keep machines tidy.

  What has to be settled before a line of it is written, because it is the
  sentence the whole tree stands on: after a merge the machine no longer matches
  the tree, and `console check` has to go on meaning *this machine has drifted
  from what it says it is*. The only coherent reading is that a device's state
  is the manifest's commit plus a branch of its own -- which is what `console
  save` already writes -- and that a merge lands there as a commit rather than
  as a file the engine cannot explain afterwards. A page in `docs/` before any
  code, the way variance above is.

  Three things it must not pretend. `[packages]`, `[services]` and `[masked]`
  are not files and merge into nothing. A file a package takes back, like the
  InputPlumber device file above, has a third writer, and a merge that knows two
  will read a package's rewrite as the person's own work. And a question the
  merge cannot answer arrives on a handheld with no keyboard in the middle of an
  update: the apply keeps what the machine has and says so until somebody looks,
  because an update that stops to ask a device nobody is holding does not
  survive a channel.

  What settles it: an update that changes a line in a file the machine has also
  changed, applied on a second machine, coming up with both and `console check`
  green afterwards.

- **Input shape, rather than input device.** `docs/button-contract.md` describes
  a pad, and every surface in the tree was written against it. A laptop is a
  keyboard and a pointer, a television box is a remote, a phone is a thumb and
  nothing else. The asset is already in place and was built for another reason:
  the panels keep their arithmetic in modules that have never heard of GTK, and
  `console-program-contract` takes a word in and hands back doings, so what a
  surface *means* is already separate from what pressed it.

  What is owed is that the contract has peers -- a keyboard contract, a touch
  contract -- and that each surface answers all of them, with the checks
  pressing each. The trap is the cheap answer: a layer that turns a keyboard
  into a pretend pad. It would work, it would be finished in a week, and every
  machine that is not a handheld would spend the rest of its life pretending to
  have buttons it does not have, in a menu shaped around a d-pad nobody is
  holding.

  The *bound* half is done -- a binding carries the input it is on, so a
  keyboard is rows in the same table the pad is in -- and the half that is owed
  is the larger one: what a surface offers is still shaped by a d-pad and an A
  button, so a keyboard reaches every job and still walks a menu by pretending
  to be one. The trap above is exactly that layer, and it is now half-built by
  accident rather than on purpose.

  This is the largest piece here and it is what decides whether anything but a
  handheld is real.

- **Stop building the operating system on the machine.** `[build]` compiles what
  the manifest names, on the device, out of the history that arrived, and that
  decision is a good one: nothing compiled travels, and what is on the machine
  is what somebody can read afterwards. It survives exactly as long as the
  machine is a handheld with a fan and a wall socket. On a fanless mini-pc it is
  a first boot measured in a battery, and on a phone it is not a design at all.

  The answer is to build once per architecture into a signed repository that the
  apply installs from, and it buys three things beyond the time. A compiler
  comes off a machine somebody carries. Architecture becomes a fact the build
  knows rather than a surprise the device meets. And the trust question stops
  being rhetorical: the engine currently believes a git remote absolutely, and a
  package that is signed is a signature something actually checks rather than an
  intention.

  What settles it: an apply on a machine with no rust installed.

- **Battery is a hardware fact and not an optimisation, which is why it is here
  and not under the note that says optimisation is last.** What changes with a
  second kind of machine is that power stops being a number to chase and becomes
  part of what a device profile *is*: the platform profile and the governor it
  wants, whether the panel can refresh itself, which audio path keeps the
  machine awake, how deep a suspend the firmware really reaches and what is
  allowed to wake it. This device's own idle draw was never a slow program; it
  was a machine awake with its audio alive, and it took measurement on the
  machine to say so.

  So each profile declares its power answers, and a check reads them back on
  whatever machine it is standing on, in the same words on all of them: what the
  governor is, what suspend state was reached, what woke it. A kernel update
  that takes one away should turn a check red on the machine it happened to,
  rather than becoming a rumour about battery life.

  The comparison the wait store has been waiting for arrives here too. The same
  opening, timed on two machines, is the first evidence anybody has had that a
  slow surface is the code rather than the silicon -- which is exactly the
  question a number on one machine has never been able to answer.

- **What a check means when the machine is not in the room.** `--stage device`
  is one handheld over ssh, and `putting_back` is what makes that a borrowing
  rather than a taking. Neither generalises by itself: a supported machine with
  nobody holding one is a machine the checks make claims about. Either each
  shape has an owner who runs the device stage before a release, or the release
  says plainly which machines were pressed and which were only compiled for. The
  second is honest and cheap and should be written before the first release
  rather than after the first complaint.

- **A machine that skipped releases, and a machine that fails halfway.** Both
  are somebody else's morning. The sweeps are derived from the manifest's
  history, which is what makes them survive a machine that is several releases
  behind -- but nothing has ever run a sweep for a name that left two releases
  ago, and that is a test, here, before it is a promise. And `previous.rs` says
  of itself that it is not a rollback and does not pretend to be one. For one
  device that is the right amount of machinery; for a channel it is the whole of
  what stands between a bad release and a machine that does not come up in a
  house nobody here can reach.

- **The security model is currently one private network and a remote nobody
  checks, and each thing that changes about that is a separate piece of work.**
  Signed history before the channel is already owed above and is the first.
  After it: the lock the machine has never had, on a thing whose nature is being
  put down on a table; encryption, which on a device with no keyboard means a
  way to unlock it with a thumb, and that is the on-screen keyboard running
  before the desktop does rather than a preference in a panel. The per-unit
  confinement is written and wants an evening on a machine to say whether the
  promises in it are true. And the stance that nothing leaves the device on its
  own is a security decision as much as a courtesy, so it belongs in the same
  page rather than only in the telemetry line above.

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
  becomes a property of the only tree there is, and the host that is
  deliberately checked for less when it is unset stops being a laptop's
  convenience.

- **The phone is a different program sharing a spine, and it goes last for a
  reason that is not difficulty.** Assume a handset with a mainline kernel,
  which means one somebody else already did that work for; anything else is a
  vendor tree and a second full-time job. Then the parts that have no analogue
  here: a modem, and telephony as an interruption. A call preempts everything on
  the screen, and nothing in the contract can say that -- a doing is carried out
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
apply still never removes, and the gate still derives what must be swept.
Nothing leaves a machine on its own. A check is still pressed rather than asked,
and a green one nobody doubts is still worse than a red one. The whole of this
section is more machines saying the same sentence, and not more sentences.
