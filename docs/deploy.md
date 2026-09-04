# Deploying

    just check     what a deploy would change, and change nothing
    just deploy    that, then ask, then apply
    just pull      take back what was saved on the device

A deploy is two things and nothing else: a `git push` into the repository the
device keeps at `/etc/console`, and then the machine's own `console apply`.
Everything clever happens on the handheld, out of the manifest it was just
handed. Nothing compiled travels, and no file is copied into place from here.

That is why the tree has to be committed before anything is sent. What reaches
the device is the history, so what is deployed is what somebody can look at
afterwards and read. `tools/console-deploy` refuses a dirty tree for that
reason, and its escape hatch is a clone: when other work is uncommitted in this
checkout, send the history alone out of a copy nobody is working in.

    clone=$(mktemp -d)/console && git clone . "$clone" && cd "$clone"
    tools/console-deploy

## The address

Every tool here reads the device from `CONSOLE_HOST` and none of them has a
default, because an address is somebody's machine and this tree does not carry
one. Set it in the environment; a `.envrc` at the root of the checkout is the
quiet way, and `.gitignore` holds that file and `.direnv/` out of the history so
the address cannot reach a copy of this repository by being forgotten about.

A tool that cannot see it says so and stops. The one place that matters most is
`console-publish`, which checks less when the host is unset rather than failing:
see the last section.

## What must hold before anything is sent

`just ready` is the whole list and the deploy runs it itself, so what reaches
the device has passed it whether or not anybody thought to. The tests, clippy
denied rather than printed, `--locked` because the device builds with the same
lockfile and a lockfile that is behind would fail there instead -- halfway
through an apply, on a handheld -- the emulated checks, and the EXPLICIT rules
the workspace already keeps.

Then the device is asked what it has that this does not. Anything committed on
the machine by `console save` shows up here as itself rather than arriving days
later as a push refused for not fast-forwarding, and `just pull` is the answer
to it. The tree is asked once more at the last moment too: `just ready` and the
fetch are minutes, and a file written in this checkout while they ran is exactly
the fault that check exists for.

## What `console apply` does with each kind of thing

`desktop.conf` is the whole inventory and its sections are applied in the order
they are written, because each needs the one before it.

**`[packages]`** first, because compiling needs the toolchain the packages
bring. The manifest names what this desktop asks for; anything the machine has
on somebody else's account is reported as borrowed rather than owned, which is
how a dependency that arrived with the base install and was never declared gets
found before a rebuilt device goes without it.

**`[build]`** next: every crate under `crates/` that the manifest lists, compiled
on the device and installed into `/usr/local/bin` under its own name. The engine
is the first of them and is the one thing this repository puts in place from the
laptop, before `apply` is asked to do anything -- it builds every other program
including a newer copy of itself, but it cannot be what replaces the engine that
is running.

**`[files]`** then, laid down from `files/` at the same path. Ownership and mode
are worked out from the path and the content, so nothing is kept in step by
hand. `@user@` is not a name: it is the mark that stands for whoever the desktop
belongs to, filled in at the moment a file is written, which is why this source
names nobody and still installs itself correctly.

**`[services]`** and **`[masked]`** last, enabled for the desktop user and
pulled in together by `console.target`.

One writer at a time. `apply` and `save` hold a lock for as long as they run, so
an apply started on the device while a deploy is applying over ssh is refused
rather than interleaved with it. `console check` and `console list` are never
blocked.

## The programs that are carried rather than built

The rule is that what we write is built on the machine that runs it, and what
somebody else wrote and we only forked is carried here as a built binary. Both
of the carried ones are GPL programs kept under their own names, and both are
ordinary `[files]` entries: `apply` lays them down like any other file, and the
bin directory in the path is what makes them executable.

`/usr/local/bin/hyprsession` is what `console-session` starts. A machine put
back together from this manifest alone has to end up with this fork and not
whatever is published under the name, which is the whole reason it travels
built. What is ours is the unit around it.

`/usr/local/bin/kew` is the music player under the Music panel, and it is the
more interesting of the two because the package is listed as well. The `kew`
package brings the libraries this links and puts its own program at
`/usr/bin/kew`; the fork sits in front of it on the path, and the panel starts
it by name, so what answers is the fork. What the fork adds is what the panel
needs and the package does not offer: `OpenUri`, so a song chosen is the library
it came from rather than a playlist of one, and `xesam:url`, so the song playing
now can be opened where it lives. Without it a chosen song restarts the player
and that row has nothing to offer.

So a package and a file can name the same program on purpose. The package is
there for what it brings; the file is there for what it answers.

`crates/console-publish` keeps the list of which paths are forks, and it is the
only place that list lives -- a program that stops being a fork leaves it in the
same commit that makes it a crate, the way the on-screen keyboard did.

## After

The deploy ends on the hardware. Everything that could be asked without the
device was asked before anything was sent, so the device tier runs only what
nothing else can answer and says of the rest where it was answered:
[docs/checks.md](checks.md) is the rest of it. `--all` is the whole tier, for a
run that is about the machine rather than about the desktop.

## Publishing

A release to the public copy is a separate errand, and it comes after a deploy
and after looking at the device, in that order.

    cargo run --bin console-publish -- <path to the public checkout>

It builds a scrubbed copy out of `git ls-files`, runs the whole suite inside the
copy, and checks that nothing in it says a name it must not. It does not push.

The forks are left out of it, both the built binaries and any source kept here
for one. The binary because a binary published without its source is a licence
somebody else wrote being broken on their behalf; the source, when there is one,
because an adaptation made for one device would carry an obligation to keep it
level with upstream and answer for it. In their place the copy carries a page
saying what the missing programs are and how to build them.

What it publishes is a snapshot with a plain descriptive subject rather
than a mirror of this history, and other machines push to the same place, so
fetch and rebuild on the public branch before committing.

The names it watches for are asked rather than remembered, which is the point:
whoever is building the copy, what this machine calls itself, what the device
calls itself, and whoever the device belongs to. None of them is written down in
this repository, and the file that does the checking is carried into the copy
and checked along with everything else -- a test written against a real name
would be the one file that fails its own check.

A name has to stand on its own to count as said, so a machine named after the
distribution it runs does not make every copy unpublishable while a hyphenated
form of the same name is still that machine being named.

Two of those four names come from the device over ssh. **With `CONSOLE_HOST`
unset, or the device off, the copy is checked for less** -- it says so out loud
rather than passing quietly, and that sentence is the one to read before
pushing. Export the host and have the device reachable when publishing, or take
the warning seriously.
