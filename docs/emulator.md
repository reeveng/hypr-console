# The emulator

Presses the Legion Go's buttons on a machine that is not one, so a change can be
tried in a second instead of over SSH.

    just test      every test that can run here
    just emulate   a Legion Go to press
    just live      the slower tier, needs /dev/uinput

    console-emulate what x                      what X does, in every profile
    console-emulate run scenarios/get-around.txt

It builds the four devices InputPlumber publishes, from a real capture kept in
`crates/console-pad/fixtures/devices.json`. `just capture` asks the device
again, and a diff means something changed under it.

A press goes through the same profile files the device reads, so this tests the
profile as well as the daemon.

`scenarios/` holds what somebody did with their thumbs, in order. The same file
plays against real devices and fake ones.

## Two tiers

**Fast.** The daemon's loop runs in this process against a world that is not
this machine's, with a clock the test controls. No devices, no root, no
compositor. This is most of `just test`.

**Slow.** Real uinput devices, with the daemon started as its own program. It
answers whether the devices this builds are the ones the daemon goes looking
for. Nothing it does reaches the desktop you are running it on.

    sudo tools/allow-uinput
    newgrp input
    just live

Without `/dev/uinput` the slow tier skips and says why.

## What it cannot answer

Whether the compositor did what it was asked. Here `hyprctl` only records that
it was called.

Whether the machine boots, whether a service comes back after a crash, whether a
udev rule fires.

Whether a person can reach a button with their thumb.
