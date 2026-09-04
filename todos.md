# What is still owed

Small work, written down so it is not carried in somebody's head. Anything
that grows past a few lines and a reason belongs in `docs/` or in a check;
anything finished leaves this file rather than gathering a tick.

Say who owns a line if anyone does, and say what would settle it. A line
nobody can act on without the device says so, because the device is one
machine and there is usually somebody holding it.

## Needs the device

- **Six pieces of standing-up-to-a-fault work are in, and none has been near
  the machine.** All of them are laptop-settled: the suite is green and clippy
  is clean. Every one of them is about what happens when something goes wrong
  on a device that is not here, which is exactly the half a laptop cannot
  answer.

  `just deploy`, and then:

  **The restart drop-in.** `systemctl --user show console-bar.service -p
  StartLimitIntervalUSec -p DropInPaths` should say `0` and name
  `console-.service.d/restarting.conf`. That prefix drop-in is one file for
  every console service, and if systemd on the device reads it differently from
  systemd here then eleven units silently keep the old limit. Then break one on
  purpose -- point `console-bar.service` at a program that exits at once -- and
  watch it go on retrying past five falls, at a widening interval, instead of
  going `failed` and staying there.

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

  `just deploy`, then Music in the menu. On **Playing**, take **Play them in
  any order** with something on: the row should turn into *Play them in the
  order they are in* with **any order** beside it, and the song after this one
  should not be the song after it in the folder. Then take **Play this one
  over** and let a song end. Both of them again to put them back, and once more
  from kew left repeating the whole list, which is the state the panel offers
  no way into and has to be able to come out of in one press.

  Y is the other half of it, and it wanted the fork on the device: the kew
  there has to be one that answers `xesam:url`, or the row for the song playing
  now will rightly offer nothing. The manifest carries the built fork at
  `/usr/local/bin/kew` now, so apply puts it there. Press Y on a song in the folder
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

- **Six that are laptop-settled and want a thumb.** The suite is green on all
  of them and not one has been held.

  **The stick scrolls slower.** Slower was a first guess. On the device it is
  either enough, or it wants a curve rather than a speed -- slow near the
  middle and quick at the edge, which is what a stick is for.

  **The paddle on the back scrolls down.** A notch a press, and it keeps
  turning while it is held. Whether the notch is the right size and whether
  holding it runs away are both things a finger says at once and a test cannot.

  **The now playing screen.** Drawn against a nested desktop and against real
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

  **The hourly card is quiet again.** `console well` said this machine had
  drifted every hour because it could not read a file in `/etc/sudoers.d`. It
  should now say nothing at all on a machine with nothing wrong, and `console
  check`, typed, should still name that file and call it *cannot read*.

  **The home screen holds the keyboard, and lets go of it.** The one thing on
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

  **Thai reaches the numbers.** The rule is that the numbers key sits beside
  the language key on every arrangement, and a test holds each of them to it.
  What a test cannot say is whether the thumb finds it there.

## Open

- **The lint suite's last four rules are still allowed, and two of them are
  mine.** `just explicit-gate` denies every rule nothing in the tree breaks and
  names the rest in one ALLOW list. Four are in it. A full `cargo dylint --all`
  on 2 September counted 269 call sites for EXPLICIT001, 146 for EXPLICIT006,
  52 for 007 and 21 for 008, and two stragglers of 013 -- which is *not* in
  ALLOW, so the gate is red on them today: `console-publish/src/tree.rs:56` and
  `console-sky/src/covered.rs:120`.

  001 and 006 are one fault with two spellings, and it is the fault behind the
  entry that used to be below this one: a failure swallowed into a default or
  into an absence. `unwrap_or_default` on a `Result` is the common shape, `.ok()`
  the other. Both say the same untrue thing -- that there was nothing to read --
  about a file that could not be read, and the machine then runs on a value
  nobody chose with nothing anywhere recording the moment it started to.
  `console_writing::Held` is the answer at the read end and is written; what is
  left is the call sites.

  A rule leaves ALLOW when its last call site is answered and never goes back.
  That is the ratchet and it is the whole design, so the work is a grind by
  intent: each denial is a call site that deserves an honest look, and a
  mechanical rewrite that turned every `unwrap_or_default` into an `expect`
  would trade a silent wrong answer for a dead desktop.

  **EXPLICIT002 is a different thing and is not in this entry.** It is
  registered `Allow` inside the suite itself, not merely in the justfile: there
  is no `Never` type in this workspace and turning it on denies every function
  in the tree. Adopting it is a decision about the whole codebase and wants that
  type first.

- **Every external program should be a name in one enum, not a string in 73
  places.** `Command::new` is called 73 times across 52 files, and what keeps
  the list honest today is `the_programs.rs`: a hand-written table of 53 entries
  and a scan of the source that greps for argv strings under it. The scan is a
  net, and the README for it says so. A net is what you build when the thing
  itself cannot be enumerated.

  It can be. If every call went through one module -- a `Program` enum, one
  variant per program, carrying whether it comes from a package, from base Arch
  or only from a machine that develops this -- then the list of external
  programs would be the enum, exhaustive because the compiler says so. A new
  program would be a variant somebody has to add rather than a string a scan has
  to catch, the table and the scan would both go, and `desktop.conf`'s
  `[packages]` could be checked against the enum directly.

  The same argument was raised for the system libraries this links against, and
  it is the same shape: what a machine has to have on it before this will build
  or run should be one list somewhere, rather than a fact spread over a dozen
  `Cargo.toml` files and a manifest.

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

- **A deploy is locked against another deploy, and not against another editor.**
  `console-deploy` takes `.git/console-deploy.lock` with `mkdir`, holds it for
  the whole run including `--check`, and releases it however the run ends. A
  second deploy is refused and told who is holding it and since when; a lock
  left behind by a killed deploy is taken over, but only where the pid it names
  is one this machine could have been running. The tree is asked again
  immediately before the push, and a file that appeared or a commit that landed
  meanwhile stops the deploy and is printed, so a race that used to send
  something nobody had checked now sends nothing.

  That is the half a lock can do. The other half is what the entry was really
  about: three sessions share this checkout and none of them reads the lock,
  because nothing makes them -- a session that edits a file is not running
  `console-deploy`. What is different is that the deploy notices and says what
  happened rather than quietly deploying it. Settled properly by whatever makes
  a session aware of the others, which is not a thing this repository has.

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

- **The keyboard is ours and the `keyboard` profile is still loaded.** The
  larger half of this settled with the port: `crates/keyboard` is a Rust program
  the device compiles, there is no C on the machine, and the plan this entry
  used to carry -- bring the fork in, take `gamepad.c` out, add a socket to be
  told on -- was overtaken by not needing any of it. `docs/programs.md` has the
  argument and the correction that undid it, which is worth reading once: the
  case against writing our own rested on a keyboard of ours being a uinput
  device, and it never had to be.

  What did not settle is the thing the plan was actually for. The keyboard reads
  the pad itself, so `/etc/inputplumber/profiles/keyboard.yaml` is still loaded
  every time it comes up, and a profile load destroys the pad and builds another
  -- which is the X flake, the `After=` on `console-keyboard.service`, and the
  whole of `Mode::profile`. Nothing about the port moved that, because reading
  the pad raw is exactly what needs the profile.

  It is the same want as the entry below, arrived at from the other side, and it
  is settled by the same thing: a claim taken while a surface is up, with
  `EVIOCGRAB` underneath, and the profile deleted after. Two entries and one
  crate.

  Not settled and worth ten minutes with a thumb before that crate is designed:
  7c found that InputPlumber's composite device carries a `dbus0` target with an
  `InputEvent(s,d)` signal and 41 `Capabilities` strings in the same vocabulary
  the profiles are written in. If that signal fires, a reader gets presses
  without opening the pad at all. Tried twice and got nothing, but neither run
  had a confirmed press behind it, so it is unproven rather than disproven.

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

- **Touch and buttons, both, everywhere.** The rule this desktop is now held
  to: everything it offers has to be reachable with the screen alone and with
  the pad alone. Neither is a fallback for the other -- it is a machine that is
  held, so a thumb on the glass and a thumb on the d-pad are both the ordinary
  way to use it, and anything only one of them can ask for is a thing half the
  device cannot do.

  The home screen is written to it: A opens, holding A picks up, Y is the card
  that says what is on the screen -- and a tap opens, a held finger picks up, a
  swipe moves a pane, a swipe up is the menu. The rest of the desktop has not
  been read against this rule. The two known gaps are the browser add-on's bar,
  which is pointer-only and has its own line above, and whatever a sweep turns
  up: every surface, asked twice, once with the screen covered and once with
  the pad unplugged.

- **The keyboard will not go away, and it is one press read twice.** Found on
  the device on 2026-09-03, by `console-check --stage device`, on the first run
  where the desktop came up far enough for these three to be asked at all.

      110-the-keyboard          the keyboard would not go away
      240-the-keyboard-comes-back-with-the-desktop
                                round 1 of 20: X raised the keyboard and would
                                not put it away
      250-the-keyboard-types-into-a-page
                                the keyboard came up over the browser and
                                "hello" did not reach the field

  The first two are one fault. X raises the keyboard and X is meant to put it
  away -- that is the one press two programs share, which `todos.md` has been
  saying wants a thumb since the router was reworked. The check's own words for
  it are that this is *one pad read twice rather than none read at all*: the
  press is arriving at both consumers, so the second read undoes the first and
  the keyboard never comes down. That points at the pad rebuild rather than at
  the keyboard, and the ordering claim to check first is
  `console-keyboard.service` being `After=` the controller.

  The third is separate and is about focus, not about the pad. The keys go to
  whoever holds the focus, so either the page never had it or the browser is
  not taking a virtual keyboard, and `MOZ_ENABLE_WAYLAND` is what decides the
  second.

  Everything else on the device passed. `140-the-desktop-is-up` and
  `210-nothing-has-had-to-be-started-again` had been failing on the bar and now
  pass.

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
  `updating`, the module is `bar-updating`, the layer is `updating`, and the
  one writer is the engine. Nothing about a bar that fills from the left is
  about applies, and there are other lengths this machine knows and currently
  says with a card that covers something: a long copy, a download, a scan.

  What would settle it is a name and a protocol that are not an apply's -- one
  place a length and a word are written, one module that draws whatever is
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

- **The build stretch moves by crate, and moves most where there is least to
  wait for.** `building.rs` reads cargo's `Compiling` lines and carries the strip
  a share of what is left on each, plus a tick for silence, so it never stands
  still and never reaches the end before the build does. The curve is
  deliberately front-loaded, on the reasoning that the question early on is
  whether anything is happening at all.

  The ask is the other shape -- a bar that accelerates, by building the longest
  crates first so what is left falls away faster and faster. That is a change to
  the order cargo builds in rather than to the bar, and cargo schedules by the
  dependency graph, not by how long anything took last time. What would settle
  it is whether the crates that dominate a build are leaves that *can* be
  started first, which is a question about this workspace's graph and is
  answerable with `--timings` on a cold build.

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

- **The home screen covers the wallpaper completely.** Since the desktop began
  opening into the applications, `console-home` is a bottom layer spanning
  everything below the bar, so the picture behind it is drawn and never seen.
  Nobody has taken that decision deliberately: the wallpaper still costs what it
  costs to draw, its check still passes, and the machine has a picture it never
  shows. Either the home screen should let it through -- which means a surface
  with a transparent ground rather than a ground of its own -- or the wallpaper
  is a thing this desktop no longer has, and should stop being drawn and checked
  as though it does.

- **Next goes nowhere with the songs in the order they are in, and that half
  is kew's.** The other half is settled: a press of A gave the player one song
  to hold, so next and previous had nowhere to go from it. Both roads ended
  there -- `kew --noui <a word>` looks the word up and plays what answers to
  it, which for one song is a playlist of one, and the fork's `OpenUri` cleared
  the playlist and built a new one out of the file it was handed. Watched on
  this machine, against a library of six: next, pressed five times, played the
  same song five times.

  The fork answers `OpenUri` on a song by building the playlist out of the
  whole library around it now, and `music-onward` carries the song so that the
  press which *starts* the player tells it what to play once it is there to be
  told. Watched again the same way: the song asked for plays, and the five
  others each play once before it comes round. Songs are a library apart, which
  is what was asked for.

  The paragraph that used to be here said that with shuffling off next moved
  one song and then stood still. That was wrong, and it was wrong because the
  harness that found it never asked for repeat-round: the list was walking
  correctly to its end and stopping there, which is what a list with no repeat
  is supposed to do. Recorded because a reproduction that leaves out a step the
  panel always takes will keep finding faults that are not there.

  What was really left was two, both found by pressing the shuffle button,
  which nothing had done. Filling the library into the playing list left the
  player's other list -- the unshuffled one it restores from -- empty, so
  pressing shuffle off copied an empty list over the library. And the restore
  itself freed every node in the playing list, the song playing among them,
  under the thread that answers the bus: a use after free that showed as a
  crash or as a dead queue depending on when the other thread looked. Both are
  fixed in the fork, the second by reordering the list in place the way
  shuffling on has always done.

  **Needs the device only for the last mile:** the fork is built, installed
  there, and carried in `files/usr/local/bin/kew` so that apply keeps putting
  it there. `280-a-song-pressed-plays-the-library` asks the whole of it on the
  device now. What is left is a hand on the machine: Music, a song, and press
  next five times -- five different songs, none of them twice, and the sixth
  press comes round to the one you started on.

- **The first press of next after the shuffle button does nothing.** Watched on
  a library of six: press shuffle, press next, and the song playing is the song
  that was playing. The press after it moves. Nothing is lost and nothing plays
  twice -- the list is right, the button is simply swallowed once -- but a
  button that does nothing on a handheld reads as a button that is broken, and
  the hand presses it again. It is the same corner as the two faults above: the
  toggle marks the next song as needing working out, and the press that arrives
  before it has been worked out is dropped rather than held.

- **Nothing says which commit a carried fork was built from.** Two built
  programs are in the tree now, `/usr/local/bin/hyprsession` and
  `/usr/local/bin/kew`, and each is a binary with no mark on it saying what
  source made it. `docs/forks.md` says where upstream is and how to build, which
  is enough to make a new one and not enough to answer the question that
  matters: whether the one carried here is still the one the fork's source
  makes. The fork moves, the binary does not, and nothing goes red. What would
  answer it is small -- the commit written down beside the path, and a check
  that the binary is not older than what the paper claims -- and it wants
  deciding where that line lives, since the fork's source is not in this tree
  and its address is not this tree's to write down.

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

- **Nothing asks whether an icon a surface names is an icon the theme has.**
  The colours have this: `every_name_the_desktop_asks_for_is_defined` crosses
  every name a stylesheet asks for against the palette, and it exists because a
  GTK stylesheet that names a colour nobody defined drops the declaration and
  carries on. An icon name nobody has is the same fault with a louder ending --
  GTK draws the broken square -- and nothing in this tree crosses the names
  against a theme.

  It turned up under the music player: the transport asked for
  `media-playlist-no-repeat-symbolic`, which is in neither Adwaita nor breeze on
  the machine this is written on, for the state the strip is in nearly all the
  time. The names are `&'static str` in a dozen crates, so this is the same
  shape as the entry above about `Command::new`: a net that greps for them, or
  one place they come from. The theme is `Papirus-Dark` and it is a package
  this desktop installs, which is what makes crossing them possible at all --
  though not on a laptop that has not got it, so this is a check on the device
  tier rather than a test in the suite.

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

- **The flows past the second are still prose.** `docs/flows.md` names the
  long walks across crates, and `crates/console-flows` runs two of them at the
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

- **The guide has no idea where it was raised from.** It is read out of the
  one table the daemon obeys, which is what keeps its words true, and the
  sweep in `getting_around.rs` presses every bare button in a place and asks
  the guide about the same button in the same breath. What it cannot ask is
  the first thing a person sees: `console-buttons --menu` calls
  `panel::show(.., None)`, so it always opens on Anywhere, where A is a click
  and R1 is a workspace. Raised over a chooser, both of those are false of the
  screen it was raised over, and the true answers are one tab along under
  Menus.

  It has a mode to open on already -- the daemon reads one off the compositor
  every press, and `panel::show` takes the tab to start on -- so what is
  missing is the guide asking. Settled when the guide raised with a chooser up
  opens on Menus, and the sweep in that flow asks the guide the way a person
  reads it rather than ordering the sections itself.

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

- **The music flow has no player to talk to off the device.** The panel
  drives kew over MPRIS, and the fork is the device's. Headless, the playing
  tab's promises -- the song survives the panel closing, the reopened panel
  agrees with the ears -- need a player that answers `OpenUri` and position
  without kew installed. Settled when those two promises are asserted in
  `just test`.

- **The viewer flow has no folder to walk.** Stepping from a photograph
  onto a film wants a fixture folder of things that weigh nothing and decode
  everywhere, kept in the tree. Settled when the reel steps through it in a
  test and the step onto the film is the same test's next line.

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

- **How many squares there are is compiled in.** `COLUMNS`, `ROWS` and `PANES`
  are constants, and the ask is that they are a setting. The layout is ready for
  it: the grid fills the screen and the squares fill their cells, so more
  columns is narrower plates and fewer is wider, with no number anywhere that
  has to be changed to match.

  What is not ready is the height. A square's minimum is its picture plus its
  name, and stacking those minima is already most of the room under the bar --
  the margin above and below had to be trimmed to make three rows fit, and a
  fourth would not. A layer surface cannot be smaller than what it holds, so it
  would hang off the bottom of the screen rather than crowd, which is what it
  was doing before the trim: the pane dots were drawn below the edge of the
  panel and nobody could see them. So the settling is that the picture's size is
  worked out from the room and the number of rows rather than being a constant
  of its own, and then the number of rows is a thing somebody can choose.

- **Nothing else has been asked whether it answers a finger.** There is a way to
  press a place on the screen now -- `console-poke`, and `Device::touch` on top
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

- **A machine can be ahead of the manifest and nothing on it says so.** Twice
  now a program has been installed on the device by hand -- a fork built here
  and copied over, a crate compiled and pushed ahead of a deploy -- because
  something unrelated was holding the deploy up. It works, and it is sometimes
  the right thing to do while a shared tree is being untangled, and afterwards
  the machine is in a state nothing records: `/etc/console` names one commit,
  and one or two of the programs beside it came from another. An apply
  reconciles it, so the drift is temporary; what is not temporary is that
  nobody can tell it is there. The next person to read `git -C /etc/console
  log` gets a true answer to the question they asked and a false impression of
  the machine.

  What would say so is small and wants deciding rather than designing: a line
  written where a hand-install happens, naming the program and what it was
  built from, and `console check`-or-apply reading it and saying "this machine
  has three programs ahead of its manifest" until an apply clears it. The
  awkward half is that a hand-install is by definition somebody working around
  the tooling, so the note has to be something the tooling can find and not
  something the person has to remember to write.
