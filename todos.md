# What is still owed

Small work, written down so it is not carried in somebody's head. Anything
that grows past a few lines and a reason belongs in `docs/` or in a check;
anything finished leaves this file rather than gathering a tick.

Say who owns a line if anyone does, and say what would settle it. A line
nobody can act on without the device says so, because the device is one
machine and there is usually somebody holding it.

## Needs the device

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

- **Tap the × on a panel with the keyboard up.** It did nothing, and now we
  know why: the panel was a stopped process for as long as the keyboard was
  up, so it could not answer a finger any more than a button. `osk-hook` now
  stops the daemon alone. What is left is one tap to confirm it, and that
  still needs a finger, because touch is not InputPlumber's to send. Open a
  panel, press X, tap the ×.

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
- **Anything opened from the menu is in the controller's control group.**
  Inherited from `launcher`, which `stick-scroll` started, and nothing a
  program can do to itself leaves a control group. So restarting
  `console-controller.service` takes every application opened from the menu with
  it. `--kill-whom=main` covers the keyboard, which is the harm anybody had
  actually met; this is the other half and nobody has hit it. Settled by
  starting applications in a scope of their own, `systemd-run --user --scope`
  or whatever `uwsm` offers here.

- **A file dropped from the manifest stays on the device.** `console apply`
  writes what the manifest names and never asks what it wrote last time, so a
  path that leaves the manifest goes on living where it was installed. Tonight
  that was the file pinning which session logs in: it had stopped being ours,
  it still sorted after the one the switcher writes, and it quietly overruled
  every attempt to leave for Game Mode. Removed by hand. Settled by the apply
  keeping a record of what it laid down and taking away what the manifest no
  longer claims.

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
