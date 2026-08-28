# A Legion Go on a machine that is not one

The desktop on that device is four daemons reading a controller. Changing one
of them used to mean editing a file over SSH, restarting a service, picking the
machine up and pressing a button, and reading a journal to find out what
happened. Everything in this directory exists so that loop happens here
instead, in under a second, and so a fix that worked once keeps working.

    make test          every test that can run on this machine
    make emulate       a Legion Go on this machine, to press
    make check         what deploying would change, changing nothing
    make deploy        put this on the device and apply it

## What is being emulated

Not the hardware. The controller is grabbed by InputPlumber and nothing on the
desktop ever sees it. What the desktop sees is the three devices InputPlumber
publishes in its place, plus the controller's touchpad, which InputPlumber
cannot translate and does not touch.

Those four are what this builds. They are not invented: `tools/capture-devices`
was run on the machine itself and wrote down what each one is, down to the
range of every axis, and `emulator/fixtures/devices.json` is that answer.
`make capture` asks again, and a difference in `git diff` is the device telling
you something changed under it.

The one property that could not be captured is the one that matters most. A
device made through uinput has no physical location, and a real one does. That
empty `phys` is the only thing telling the pad InputPlumber published apart
from the pad in a person's hands, it is how the daemons tell them apart, and it
is why the emulated pad is found by the same search that finds the real one.

## What a press goes through

A button is pressed by the name written on it. What that press turns into is
decided by the profile that is loaded, read out of `files/etc/inputplumber/`,
which is the same file the device reads. So this is a test of the profile as
much as of whatever is at the other end: rename a mapping and the guide
changes, change a target and the test that fails is the one about what the
button promises.

    tools/legion-emulate what x        what X does, in every profile

Two things here are a model of InputPlumber rather than a recording of it, and
both are written down where they are assumed, in `emulator/go.py`. A button
with no mapping is passed through untouched, which is what an empty profile
means. An event only reaches a device the profile lists in `target_devices`,
because InputPlumber builds what a profile names and destroys the rest.

The touchpad is not in that loop, here or there.

## The two tiers

**The fast one** runs a daemon in this process against a stand-in for evdev.
There are no devices, no root, no compositor, and no clock but the one the test
holds. That last part is what makes it worth having: a stick held for exactly
one second scrolls exactly as far as the arithmetic says, every run, on any
machine, so the test can be about the number rather than about roughly.

The daemon is not modified and does not know. It is loaded from
`files/usr/local/bin/`, the same file that gets installed.

**The slower one** makes real uinput devices, starts the daemon as its own
program with nothing told to it, and reads what comes out off a device the
kernel published. It answers the one question the fast tier cannot: whether the
devices this builds are the ones the daemon goes looking for.

Nothing it does reaches the desktop it is run on. The daemon's output device is
grabbed the moment it appears, and a grabbed device delivers to whoever grabbed
it and to nobody else.

It needs to be able to make an input device, which means `/dev/uinput`, which
belongs to root:

    sudo tools/allow-uinput

Without that the slower tier skips and says why. Everything it proves about
what a daemon decides is proved in the fast tier too.

## Scenarios

`scenarios/` holds what somebody did with their thumbs, in the order they did
it. The same file plays against real devices and against fake ones, which is
the point of having it: what was tried by hand becomes what is kept as a test.

    tools/legion-emulate run scenarios/get-around.txt

## Pressing buttons at the real desktop

`make emulate` publishes the four devices and takes commands until it stops.
While it runs, the desktop in front of you is reading them, and `press a` will
click on whatever the pointer is over. There is no profile switching daemon
here, so `profile menu` says which profile the emulator translates through; it
does not tell anything else that a menu is open.

## What this cannot answer

Whether the compositor does what it was asked. The daemons run `hyprctl`, and
in a test that is a program that writes down that it was asked. Whether
`hl.dsp.window.close()` closes a window is a question for the device.

Whether the machine boots, whether a service comes back after a crash, whether
a udev rule fires. The manifest is checked for the mistakes that can be read
out of it, and `legion check` on the device answers the rest.

Whether a person can reach a button with their thumb.
