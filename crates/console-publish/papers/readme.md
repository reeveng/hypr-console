# hypr-console

A handheld running Hyprland as a desktop, driven entirely by its controller.

This is a Lenovo Legion Go with CachyOS on it, set up for somebody who is not
going to plug in a keyboard. There is no Game Mode and no Plasma. There is a
compositor, a bar, a menu, an on-screen keyboard and a settings panel, and
every one of them is reachable with two thumbs.

What is worth taking from here is probably not the desktop. It is the two
things underneath it.

## Everything is declared, nothing is done by hand

`desktop.conf` is the whole inventory: packages, files, services, masked units,
and the palette. The content of every file it names is under `files/` at the
same path. `console` is the only thing that installs any of it.

    console check     where the machine has drifted from the manifest
    console apply     bring it back
    console save      take a file edited in place back into the source

This exists because of one complaint: "every restart breaks previously fixed
things, it's like changes aren't persisted between fixing them". Every helper
was running because somebody had started it by hand over ssh, and the
compositor config launched things it then forgot about. State lives in a file
now rather than in whatever happens to be running.

## The controller is emulated, so the desktop can be tested without touching it

The device publishes three virtual input devices through InputPlumber, plus a
touchpad it cannot translate. `console-pad` builds all four somewhere else, from
a capture of the real ones, and presses their buttons by name through the real
profile. So what a button does can be tested in under a second on a laptop.

    just test          every test that needs no machine at all
    just desktop       the device's own desktop, nested, at its own screen size
    just checks        every feature it has grown, tried again
    just deploy        push it to the device and apply it

There are three places a change can be tried, and the same files describe all
three. In this process, against a stand-in for evdev with a clock the test
holds, so a stick held for a second scrolls exactly as far as the arithmetic
says. In a nested Hyprland reading the device's own compositor config at the
device's own 1024x640, so a bar that does not fit can be seen not to fit. And
finally on the device itself, where a button is pressed through InputPlumber's
own SendEvent, exactly as the hardware's would arrive.

## Where to start reading

| link | description |
| --- | --- |
| [`docs/button-contract.md`](docs/button-contract.md) | What the buttons promise, and why it is checked rather than remembered |
| [`docs/emulator.md`](docs/emulator.md) | The controller, in software |
| [`docs/desktop.md`](docs/desktop.md) | The desktop, running on a machine that is not the device |
| [`docs/checks.md`](docs/checks.md) | One check to a feature, replayed oldest first |
| [`docs/forks.md`](docs/forks.md) | The two programs that are not in this repository |

## Licence

MIT, for everything in this repository. The two forks named in
`docs/forks.md` are not here and keep their own.
