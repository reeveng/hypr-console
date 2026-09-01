# What is still owed

Small work, written down so it is not carried in somebody's head. Anything
that grows past a few lines and a reason belongs in `docs/` or in a check;
anything finished leaves this file rather than gathering a tick.

Say who owns a line if anyone does, and say what would settle it. A line
nobody can act on without the device says so, because the device is one
machine and there is usually somebody holding it.

## Needs the device

- **The router has not been held.** Every button now arrives at the controller
  daemon as itself and what any of it means is one table there, which is a
  change nothing on a laptop can finish checking: the emulator drives the real
  profile and 1174 tests pass against it, and none of them is a thumb. What
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

  `make deploy`, then Settings, **Buttons**, A on any row, and press whichever
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

- **The two music modes and the search have not been pressed on the device.**
  Both halves are settled here as far as a laptop can settle them. kew answers
  `Shuffle` and `LoopStatus` on MPRIS, and it takes a set of either as a press
  of its own key without ever reading the value: `set_property_callback` in
  `src/sys/mpris.c` calls `toggle_shuffle` and `toggle_repeat`, and the repeat
  key is a round of three. So `player::repeat` asks first and presses as many
  times as the round makes it, which was watched round twice on this machine
  with busctl. The reading was pressed too: 1330 songs read in 79 seconds, and
  the second run over the same library is fifteen milliseconds.

  Playing at all was watched on the device and is settled: the panel used to
  press A into silence because kew had never been told where the music is, and
  it asks that question on a terminal a panel does not have. The folder is
  written into `kewrc` now, and kew was watched playing a song on the device
  with the desktop's own session around it.

  `make deploy`, then Music in the menu. On **Playing**, take **Play them in
  any order** with something on: the row should turn into *Play them in the
  order they are in* with **any order** beside it, and the song after this one
  should not be the song after it in the folder. Then take **Play this one
  over** and let a song end. Both of them again to put them back, and once more
  from kew left repeating the whole list, which is the state the panel offers
  no way into and has to be able to come out of in one press.

  Y is the other half of it, and it needs the fork rebuilt and installed: the
  kew on the device has to be one that answers `xesam:url`, or the row for the
  song playing now will rightly offer nothing. Press Y on a song in the folder
  and the files panel should open on Music, standing on that file; press it
  again from **Playing** with something on and it should open standing on the
  song you are listening to. Both were watched here in the nested desktop, the
  first with a path handed to `files-panel` by hand, the second only as far as
  the property: kew says the file it is playing now, which it did not before.

  Then **Music**. Arriving on the tab is what sends the library to be read, and
  the corner says how many songs that is. What is worth measuring is how long
  it takes on the device: it is minutes of ffprobe and it is the one thing here
  a handheld will feel. Press X and type an artist nothing is named after, and
  check the songs by them arrive -- until the reading has finished only the
  filenames answer, which is the difference to watch for.

- **Nothing on the Download panel has been pressed on the device.** The card
  opens and draws its list in the nested desktop here, and both fetches have
  been run on a laptop: a song arrives in Music as an opus with its cover and
  its title inside it, and a film arrives in Videos as an mkv with the picture
  attached. What no machine here can answer is the half that is a thumb.

  `make deploy`, then Download in the menu. Type a song with X, press A to walk
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

- **The keyboard's layer is named from wvkbd's source and not from the
  machine.** `bar-door` lights the bar icon while what it opened is on the
  screen, and it decides that by looking for the surface in `hyprctl layers
  -j`. The launcher's name is ours and certain: every panel lists itself under
  its own program name now, which `console_panel::panel::namespace` sets from
  argv. wvkbd's is not ours, and `wvkbd` is what its source calls it rather
  than what this machine was seen to answer. It is matched on the front of the
  name so the layout suffix does not matter, and the height is checked because
  wvkbd is started `--hidden` and stays for the session.

  Two things a thumb confirms, both in one go. Open the menu: its icon should
  go pink the way the workspace you are on is pink, and go dark when it
  closes. Then press X: the keyboard icon should do the same. If the menu
  lights and the keyboard does not, the name is the fault, and

      hyprctl layers -j | grep namespace

  with the keyboard up says what it should be.

- **The wallpaper has been pressed on the device; what it does there is still
  unconfirmed.** Eight pictures and their stills are in
  `/usr/share/backgrounds/console`, so the press itself is settled. Everything
  in
  `crates/console-sky` is tested here and one pressed picture has been shown on
  the real panel over ssh, which settled the three things a laptop could not:
  the grade reads correctly against the bar, 2560x1600 through the quarter turn
  needs no resampling, and what a loop costs the wallpaper daemon in memory,
  which is 66 MiB for a picture where a sixteenth moves and 697 MiB for one
  where all of it does. What is left needs a deploy. `make deploy`, then
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

- **Nothing here can hold a trigger, so no chord that needs one can be
  pressed.** `020`, `090` and `091` are the three that are left failing and
  they are one fault, in the harness rather than in the machine. Measured on
  the device:

      console-brightness down, by hand      64000 -> 58000, works
      L2 held and d-pad pressed, injected  58000 -> 58000, never arrives

  The trigger was sent forty times around the press, from one ssh connection
  so the two are milliseconds apart, and the chord still did not land. The
  pad reports its own LeftTrigger at rest a few hundred times a second, and
  the injected value does not outlive the next report. `021` passes for the
  same reason it always did: it asserts the carry does **not** happen.

  So the screen, `console-brightness` and the daemon's `CARRIED_KEYS` are all
  in the clear, and there is no run on record of a held L2 ever reaching the
  daemon.

  InputPlumber's interface has been read and there is no way to hold an axis:
  `SendEvent` and `SendButtonChord` are all there is, and the composite device
  publishes `Gamepad:Button:LeftTrigger` as well as the axis, which was tried
  and does not arrive either. The daemon wants the axis over `CARRY_HELD`, and
  the pad's own reading of a stick nobody is touching wins every time.

  `Device.trigger` now says so, the way `130-the-touchpad` does for touch, so
  `020`, `021`, `090` and `091` skip on the device and give the reason. `021`
  goes with them: it was green because nothing arrived, which is the same
  falseness as `020` wearing the other face.

  What is left is a thumb. Hold L2, press the d-pad left, and say whether the
  screen dims. Then the same with a window and the right shoulder.

- **Nothing the browser's add-on does has been pressed on the device.** What a
  laptop can settle is settled: the archive `console-web` writes is a zip a
  reader opens and finds the six files in, the version in it is read back out
  of the packed bytes, the labels are prefix-free at every size and never
  longer than three presses until a page has more than sixty things on it, and
  no file in the add-on holds a colour of its own. None of that has been near a
  browser.

  `make deploy`, then start the browser -- and if it was already running, close
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

  `tools/voice-compare` is the measurement, and it runs on the device as the
  person whose session it is -- not as root over ssh, because the microphone
  belongs to a PipeWire that belongs to a session. The copy already on the
  device is an older one that knows six clips; send this one first.

      scp tools/voice-compare $CONSOLE_HOST:<the user's home>/voice-compare
      ./voice-compare --models     # about 7.5 GB
      ./voice-compare --build      # llama.cpp, pinned, slow on a handheld
      ./voice-compare --record     # sixteen clips, prompted one at a time
      ./voice-compare              # every clip through every model

  The graphics-card whisper and the turbo it runs today are already there, so
  `--models` is fetching the three it is being compared against: turbo
  unquantised, the full `ggml-large-v3.bin`, and Qwen3-ASR with its mmproj.

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

  Also there while the directory is open: `ggml-small-q5_1.bin`, 190 MB, left
  over from the few hours the small model was in use. Nothing reads it.

- **A kind of thing is a family of types now, and only Music has been given a
  default to fall back on.** The device says the fault plainly. Each of the
  three settings took effect for exactly one type and the rest of each family
  scattered:

  | | set to | and the rest of the family |
  | --- | --- | --- |
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

## Open

- **Nothing in this tree may name the person or the machine again.** The tree
  used to say three things it should not: her name, in `desktop.conf` and in
  every path under `files/home/`; the device's address, in the Makefile, both
  tools and `console-stage`; and the controller's serial, in the captured
  devices and in the scrubber's own dictionary. A fourth, her home to about a
  kilometre, was in `theme/sky.toml` for the wallpaper's sun and weather.

  All four are gone and each was replaced by asking rather than by storing:

      the person     `@user@` in the manifest, filled in by `machine::whoever`
      the device     `CONSOLE_HOST`, required, with no default to fall back on
      the serial     not captured at all; `capture` writes an empty `uniq`
      the place      `console_sky::here`, from `/etc/localtime` and `zone1970.tab`

  So there is nothing left to scrub, and `console-publish` no longer rewrites
  anything on the way out. What it does instead is ask this machine and the
  device what they are called and refuse to build a copy that says any of it.
  That check is only as good as what it can reach: without `CONSOLE_HOST` it
  cannot ask the device, and it says so rather than passing quietly.

  What would undo it, for anybody working here: a path written `/home/<a
  name>/` instead of `/home/@user@/`; a re-run of `capture` whose `uniq` is
  committed, which `the_captured_devices_name_nobodys_controller` refuses; a
  coordinate, a hostname or an address put back as a constant "just for now".
  Two tests stand exactly here — that one and the mark's round trip in
  `install` — and `console-publish` is the last gate before anything is pushed.

- **A deploy needs a still tree, and three sessions share one checkout.**
  `console-deploy` refuses on a dirty tree, and tonight it was beaten twice by
  a file appearing between the status and the push, from sessions that had no
  way to know somebody was mid-deploy. Nothing here is a lock. Raised by 35,
  who is right that it will cost somebody again.

- **A pressed key looks the same as a selected one.** `osk-start` gives
  `--press` pink and `--sel` rose, 1.01:1 apart: the same lightness, differing
  only in hue. It is the one thing that says whether a press registered. The
  cause is the palette's method rather than the keyboard: all ten accents
  declare `lightness = 0.855`, so every pair among them lands between 1.00:1
  and 1.01:1, and coral against rose is closer still. Right for ten terminal
  colours seen side by side, wrong for one element at two moments. Settled by a
  pair that must be told apart declaring a ratio against each other as well as
  against the ground, or by one of them being chosen rather than solved.
  `crates/console-theme/tests/the_keyboard_colours.rs` has
  `no_two_backgrounds_are_the_same_colour`
  standing exactly here and passing, because it asks whether they are the same
  string; widen it to a distance rather than write a new one.
- **A file dropped from the manifest stays on the device.** `console apply`
  writes what the manifest names and never asks what it wrote last time, so a
  path that leaves the manifest goes on living where it was installed. Tonight
  that was the file pinning which session logs in: it had stopped being ours,
  it still sorted after the one the switcher writes, and it quietly overruled
  every attempt to leave for Game Mode. Removed by hand. Settled by the apply
  keeping a record of what it laid down and taking away what the manifest no
  longer claims.

  Sighted again on 31 August: `/etc/inputplumber/profiles/menu.yaml`, six
  kilobytes, in no manifest and named by no word `controller-profile` takes, so
  nothing can load it. It is harmless in the way the first one looked harmless,
  and it lies in the same voice: it still holds the two stick-press clicks that
  were taken out of the profiles that day, so anybody reading `/etc` to find
  out what a button does is told this machine has three clicks. Left there,
  because deleting it is the user's call and one file is not the fix.

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

- **A unit that starts a script is not restarted when the program the script
  runs changes.** `named_by` reads the absolute paths out of a unit's Exec
  lines, and `console-keyboard.service` names `/usr/local/bin/osk-start`, which
  is the script that runs `wvkbd-mobintl` with its colours. So a new keyboard
  was installed tonight and the old one went on running: the machine matched
  the manifest and behaved like the version before it, which is the one state
  this engine exists to make impossible. Restarted by hand. Settled by
  `named_by` following one level into a script the manifest also holds, or by
  a unit naming what it actually depends on.

- **The keyboard reads the pad itself, and that is the thing that rebuilds the
  pad.** Settled between 67, ed and 7c tonight after three wrong answers, and
  the argument belongs to `docs/programs.md` rather than here. What that
  document recommended — stock wvkbd with a program of ours driving it — has no
  mechanism: wvkbd's whole IPC is SIGUSR1, SIGUSR2 and SIGRTMIN, so it can be
  shown, hidden and toggled and nothing else. There is no way to move its
  highlight from outside, and driving it by synthesising pointer events at key
  coordinates needs its layout geometry and moves the real cursor.

  Writing our own was the other answer and is worse. It regresses Thai:
  `hyprland.lua:78` is `kb_layout = "us"` with no second group, so a uinput
  keyboard can emit only what a US layout produces, and reaching Thai any other
  way means hand-deriving through a shared layout group or wiring a raw Wayland
  protocol onto GTK's `wl_display`. `tests/the_keyboard.rs` says why that is not
  costable away, and the eight keymaps that already answer it are 12330 lines we
  would be rewriting to stand still.

  So: **the pad reading comes out of the fork, the fork comes into the tree.**
  `gamepad.c` goes, a small command socket arrives in its place, and
  `console-controller` takes the pad with `EVIOCGRAB` while the keyboard is up,
  decides from `MEANS`, and says "left, down, press". Net less C than there is
  now, and it is C that can be built from what is written down.

  The delta is readable rather than estimated, which is the other thing that
  settled tonight. `~/Documents/projects/wvkbd` is a clone of
  `jjsullivan5196/wvkbd` on a branch `codincod-controller`, clean, seven commits
  above `origin/master`, and `git format-patch origin/master..HEAD` is the whole
  internalisation. Four of the seven are the gamepad reading and come out with
  it — `1212b61` read the pad, `bd1525d` measured to a key's edge, `9af75ce`
  typed on a stick press, and `b38097e "X is the keyboard, and nothing else"`,
  which is where X's binding goes to `MEANS` and stops being a constant in
  somebody else's C. Three stay: the solid selected key, `b3d1627` Thai, and the
  keymap following the keys back to the first layer.

  So the standing claim that this source "is in no repository" is wrong, and it
  is wrong in `docs/programs.md` three times — ed is correcting it. The real
  fault is narrower and worth stating exactly, because the two have different
  fixes: `origin` is *upstream*, which nobody here can push to, and the branch
  tracks nothing. It does not need a remote. It needs one that is ours. Until it
  has one, seven commits nothing in this repository can reach are the whole
  difference between the keyboard on the device and the keyboard anyone else
  would build. The user has said they are not publishing the fork, which settles
  where it may not go and not where it must live.

  The payoff is not the C. It is that a keyboard told what to do needs no
  `keyboard` profile, and a profile load is what destroys and rebuilds the pad
  every time. That rebuild is the X flake, the `After=` on
  `console-keyboard.service`, and the whole of `Mode::profile`. The grab is not
  a guess: `docs/button-contract.md` records the published pad coming back free
  from an `EVIOCGRAB` taken and given back while Game Mode had the screen.

  Three things the plan owes, none of them the interesting part, all of them
  cheaper to write down now than to meet later.

  **Whoever draws owns "is it up".** A command socket makes it tempting to have
  the daemon track visibility so it knows when to grab. It must not: `osk` is
  seven lines and stateless precisely because wvkbd answers that question, and
  its comment records an earlier version that kept the answer in a file and
  guessed wrong every other press. Either the keyboard says so on the socket, or
  it is read off the compositor the way `Mode::seen` already does.

  **The publish machinery does not cover a fork it holds the source of.**
  `console_publish::tree::FORKS` names `/usr/local/bin/wvkbd-mobintl`, a built
  program under `files/`, and `is_fork` matches that path. Source in the tree is
  tracked and not matched, so it would go into the public copy. So
  `console-publish` grows a source exclusion beside the binary one and
  `the_forks_are_not_carried` covers it, or a deploy publishes the fork the
  first time somebody runs it.

  Write that exclusion as what it is. wvkbd is GPL-3 and this workspace is MIT,
  but GPL does not forbid publishing source — it forbids relicensing it, and
  carrying GPL C in an MIT repository is fine as long as that subtree keeps its
  own licence and says so. The exclusion is enforcing a decision not to publish
  the fork, not a rule against it. Written as "we cannot publish this", the next
  person to read it takes the machinery for a compliance gate and is afraid to
  touch it. `papers/forks.md` has the same slip today — "that source belongs to
  the projects below rather than to this one" is not quite true either, since
  the seven patches on top belong to this one.

  **`console apply` runs cargo and nothing else.** An in-tree C fork needs the
  apply to build C as well, and `named_by` above changes shape in the same
  motion: `osk-start` and `osk` both go, so the unit stops naming a program that
  starts the program that matters.

  Settled in the order ed gave, which is the one that keeps a device somebody is
  holding usable: the socket and the grab first with the fork still installed
  and working, then the profile deleted, never all three at once. This is the
  one surface this device cannot be used without.

  Not settled and worth ten minutes with a thumb before the socket is designed:
  7c found that InputPlumber's composite device carries a `dbus0` target with an
  `InputEvent(s,d)` signal and 41 `Capabilities` strings in the same vocabulary
  the profiles are written in. If that signal fires, the daemon reads presses
  without opening the pad at all. Tried twice and got nothing, but neither run
  had a confirmed press behind it, so it is unproven rather than disproven.

  One thing that is wrong today whichever way this goes:
  `console-publish/papers/forks.md` tells a reader to clone upstream and run
  `make wvkbd-mobintl`, which builds a keyboard with no Thai layer and no X
  button. It is the file `the_keyboard.rs` points at, generated into the public
  copy as `docs/forks.md`, and it is the only instruction anybody outside this
  machine gets.

- **Three programs need the pad to themselves, and all three take it a
  different way.** This is the composable piece the keyboard work turns up, and
  it is worth pulling out precisely because it is not speculative: the callers
  exist today and no two of them agree.

  The need is one sentence. While this surface is in front, nothing else acts
  on the pad. `Mode::acts` already answers half of it — the daemon stands down
  for `Keyboard` and `Asking` alike, which is the daemon's own restraint and
  the part that works. The other half is making sure nobody *else* acts, and
  that half is written three times.

  The keyboard opens the pad itself and leans on the `keyboard` profile to hand
  it over untouched. `console-asking` loads an `asking` profile that sends every
  button to a key nothing listens for, then reads the press off the keyboard
  InputPlumber publishes rather than the pad, because the profile has taken the
  pad away from it. Both are profile loads, and a profile load destroys and
  rebuilds the pad — the fault the entry above is about, arrived at twice by
  two people solving the same problem a year apart.

  The third is worse and is a live bug in its own right, below.

  Settled by one small crate with the shape the others here have: a claim taken
  while a surface is up and released when it goes, `EVIOCGRAB` underneath, and
  nothing else. `console-door` is the precedent and the argument — it exists
  because the panel and the daemon both needed to know what was in front, and a
  daemon reading a pad twenty times a second should not carry a toolkit to find
  out. Same shape here: three callers, one answer, and today three copies of it
  that are each wrong differently. Name is open; the crates here are plain words
  and I have not earned the naming of this one.

  What it is *not* is `offers()`. `docs/programs.md` holds that back until two
  programs want it and is right to. This is smaller than a registry and it
  clears that bar already, which is the only reason to write it.

  Not owed yet: a way to *tell* a running program what to do. The keyboard's
  command socket would be the first, and it would be the only one — every
  socket in this tree today is the compositor's, read, never ours, written. One
  caller is not a crate. If `console-asking` ever wants driving rather than
  reading, that is two, and then it is.

- **`console-buttons --identify` stops the daemon with `SIGSTOP`, which this
  repository has twice written down as the wrong way.** `identify()` in
  `console-guide/src/bin/console-buttons.rs` runs `systemctl --user kill
  --signal=STOP console-controller`, reads a press, and sends `CONT`. That is
  `osk-hook`, which `33dcb93` deleted for being exactly this, and the reasons it
  was deleted apply here unchanged.

  Stopped is not deaf. `docs/programs.md` says it plainly: the devices stay
  open, the kernel goes on queueing, and the backlog arrives in one instant when
  the daemon starts again — every button pressed in between, in order, against a
  desktop that has moved on. That is how the machine once left for Game Mode on
  its own. And no `--kill-whom=main` is named, so the signal reaches every
  process in the daemon's control group: the menu, the panel, and anything
  opened from the menu. `docs/button-contract.md` records what that looks like
  from the front — a panel on screen reading nothing until the keyboard went
  away.

  It is the least-pressed of the three, which is the only reason nobody has met
  it. Settled by the claim above, which is what it wanted in the first place.
