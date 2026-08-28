# What is still owed

Small work, written down so it is not carried in somebody's head. Anything
that grows past a few lines and a reason belongs in `docs/` or in a check;
anything finished leaves this file rather than gathering a tick.

Say who owns a line if anyone does, and say what would settle it. A line
nobody can act on without the device says so, because the device is one
machine and there is usually somebody holding it.

## Needs the device

- **Install the compiled engine once, by hand.** `/usr/local/bin/legion` is a
  rust program now, and it is the program that installs every other one, so
  nothing on the machine can put the first copy there. `rust` is in
  `[packages]`, so the next apply by the old engine installs the toolchain and
  then ignores `[build]`, which it has never heard of. What is left is one
  command on the device:

      cd /etc/legion && cargo build --release --bin legion \
        && install -m755 target/release/legion /usr/local/bin/legion

  After that `legion apply` is the rust one and keeps itself up to date like
  anything else, because a compiled program is not read as it runs and the
  engine replaces a name rather than a file.

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

      legion-brightness down, by hand      64000 -> 58000, works
      L2 held and d-pad pressed, injected  58000 -> 58000, never arrives

  The trigger was sent forty times around the press, from one ssh connection
  so the two are milliseconds apart, and the chord still did not land. The
  pad reports its own LeftTrigger at rest a few hundred times a second, and
  the injected value does not outlive the next report. `021` passes for the
  same reason it always did: it asserts the carry does **not** happen.

  So the screen, `legion-brightness` and the daemon's `CARRIED_KEYS` are all
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

- **A deploy needs a still tree, and three sessions share one checkout.**
  `legion-deploy` refuses on a dirty tree, and tonight it was beaten twice by
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
  `tests/test_keyboard_colours.py` has `test_no_two_backgrounds_are_the_same_colour`
  standing exactly here and passing, because it asks whether they are the same
  string; widen it to a distance rather than write a new one.
- **Anything opened from the menu is in the controller's control group.**
  Inherited from `launcher`, which `stick-scroll` started, and nothing a
  program can do to itself leaves a control group. So restarting
  `legion-controller.service` takes every application opened from the menu with
  it. `--kill-whom=main` covers the keyboard, which is the harm anybody had
  actually met; this is the other half and nobody has hit it. Settled by
  starting applications in a scope of their own, `systemd-run --user --scope`
  or whatever `uwsm` offers here.
