# hypr-console

A Lenovo Legion Go running Hyprland as a desktop, driven entirely by its
controller. No Game Mode and no Plasma: a compositor, a bar, a menu, an
on-screen keyboard, panels and a guide, every one of them reachable with two
thumbs.

Two things hold it up. `desktop.conf` is the whole inventory -- packages,
files, services, masked units -- and `console apply` is the only thing that
installs any of it, so a restart cannot lose a fix no one wrote down. And the
controller is emulated from a capture of the real one, so what a button does
can be tried on a laptop in a second rather than over ssh.

    console check / apply / save     where the machine has drifted, put it back,
                                     take a file edited in place back into source
    just test / desktop / checks     here, nested at the device's own size, and
                                     every feature it has grown
    just ready                       everything that must hold before a deploy
    just deploy                      push it to the device and apply it

The recipes that decide something are Rust rather than shell: `cargo x` lists
them, and `just` is a line apiece in front of them.

## The tree

    desktop.conf    the whole inventory
    machines.conf   what is true on one machine and nothing else
    files/          the content of every file it names, at the same path
    crates/         every program on the device, in one workspace
    docs/           why each thing is the way it is
    migrations/     what a machine is told when the manifest stops naming something
    scenarios/      presses a person would make, replayed
    theme/          palette.toml, the one place a color is chosen
    tools/          the lint suite, and the renamer it keeps company with

`ls crates/` is the tour. Every crate is named for what it does, the ones that
share a subject share the word after `console-`, and each one's head says the
rest. [`docs/`](docs) is the argument behind them, one file to a subject, and
[`docs/forks.md`](docs/forks.md) names the programs that are not here.
[`tools/explicit-rust`](tools/explicit-rust) is the dylint suite the workspace
is written to, and its README says what each rule is for.

This README used to list all of those by hand, and it went on describing the
tree it was written beside long after that tree had doubled: crates it did not
name, tools that had become crates, rules it stopped counting at twenty. The
list worth reading is the one the tree keeps.

## License

AGPL-3.0-or-later, for everything in this repository but one crate. Take it,
change it, run it, sell it -- and whoever you hand it to, over a wire as much
as on a disk, gets the source of what you handed them under the same terms.
That is the whole reason for the choice: what is open here stays open
downstream, and a desktop someone serves rather than ships is not a hole in
that.

The exception is
[`crates/console-input-keyboard`](crates/console-input-keyboard), which is
GPL-3.0-or-later. It is a port of [wvkbd](https://git.sr.ht/~proycon/wvkbd) and
carries the license it was given; the AGPL is not a later version of the GPL, so
it was never ours to move. Its own module comment says so.

The two forks named in [`docs/forks.md`](docs/forks.md) are not here and keep
their own.
