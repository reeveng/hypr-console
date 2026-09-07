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
afterwards and read. `console-deploy` refuses a dirty tree for that
reason, and its escape hatch is a clone: when other work is uncommitted in this
checkout, send the history alone out of a copy nobody is working in.

    clone=$(mktemp -d)/console && git clone . "$clone" && cd "$clone"
    just deploy

## Who is asked

A deploy stops twice to ask, and neither question is this laptop's to answer.
The machine that is about to change is in somebody's hands, and the second
question -- whether to press every feature now -- takes their screen for
several minutes. So the question goes to the device: `console-confirm` raises a
card in their session, they answer it with the pad, and its status comes back
as the answer.

`--yes` skips both, and means a person has already said so. A device with no
card to raise -- the first deploy that carries one, or a machine whose desktop
is not up -- says so with a status of its own, and the question falls back to
this terminal rather than being decided for anybody.

## The address

Every tool here reads the device from `CONSOLE_HOST` and none of them has a
default, because an address is somebody's machine and this tree does not carry
one. Set it in the environment; a `.envrc` at the root of the checkout is the
quiet way, and `.gitignore` holds that file and `.direnv/` out of the history so
the address cannot reach a copy of this repository by being forgotten about.

A tool that cannot see it says so and stops. The one place that matters most is
`console-manifest-publish`, which checks less when the host is unset rather than
failing: see the last section.

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

## What it looks like while it is running

An apply is minutes, and most of them used to be silent. Pacman said nothing
this end could read, `cargo build --release` says nothing at all until it is
finished, and what somebody watching over ssh got was a cursor. The two
questions they actually have are whether it is still going and whether it is
about to break something, and neither had an answer.

It is drawn the way pacman draws now, and the reason is not the hashes. Pacman
is the long thing everybody on an Arch machine has already watched a hundred
times without once wondering whether it had hung, and what earns that is not how
it looks: it is that pacman never says anything it does not know. Its counters
come out of a transaction that was settled before the first byte moved, so
`(2/14)` is a fact rather than an estimate. It names the thing it is on, by
name, so a slow one can be told from a stuck one. It ends every line it
finishes, so what has already happened stays on the screen to be read back. And
it never fills the bar and then sits there, because the bar reaching the end is
the only thing it uses to say the end has come.

Those four are the rules, `console-how-far` is where they are kept, and the
shape follows from them -- pacman's own, off its own format strings: the
counters padded to the width of their total so the line does not jitter as they
climb, the name on the left, the bar on the right, redrawn over itself while it
fills and ended with a newline when it is full. An apply is then a list of
stretches that got done, with one line at the bottom still moving, and
everything the apply has to say -- the crate being compiled, the file being
staged, the service being restarted -- scrolls past above it.

The line moves inside a stretch, not only between them. Cargo names each crate
as it starts one, pacman names each package as it fetches and writes it, and the
files and the services are lists whose length is known before the loop begins.
The build is the one with no honest total to count towards -- how many crates a
build compiles depends on what changed, and asking cargo in advance means
running the resolver twice -- so that one moves a share of what is left per
crate, which always moves forward and never arrives on its own. The stretch
ending is what fills it.

The same line is what a device run of the checks draws, out of the same crate,
because they are the same question asked about two long things. `docs/checks.md`
is that end of it.

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

`crates/console-manifest-publish` keeps the list of which paths are forks, and
it is the only place that list lives -- a program that stops being a fork leaves
it in the same commit that makes it a crate, the way the on-screen keyboard did.

## After

The deploy ends on the hardware. Everything that could be asked without the
device was asked before anything was sent, so the device tier runs only what
nothing else can answer and says of the rest where it was answered:
[docs/checks.md](checks.md) is the rest of it. `--all` is the whole tier, for a
run that is about the machine rather than about the desktop.

## Publishing

A release to the public copy is a separate errand, and it comes after a deploy
and after looking at the device, in that order.

    cargo run --bin console-manifest-publish -- <path to the public checkout>

It builds a scrubbed copy out of `git ls-files`, runs the whole suite inside the
copy, and checks that nothing in it says a name it must not. It does not push.

The path is written over rather than made from nothing. Everything already there
goes, except `.git` -- the history the copy is committed into -- and `target`,
which is cargo's and is ignored by both trees. That is what lets the suite run
where the copy lands, and it has to: two of the tests read the history of
`desktop.conf` to work out what has left the manifest, and in a directory that
is not a checkout they can only say that they could not look.

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
