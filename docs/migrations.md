# What the manifest cannot say

`desktop.conf` says what must be on this device. It has never said what must not
be, and the engine has no state for it: `console_manifest::build::State` is `Ok`,
`Differs`, `Missing` or `Unbuilt`, and every one of those is about a name the
manifest *does* carry. There is no code path anywhere in the engine that unlinks
anything. `console check` ending in *The machine matches the manifest* means
every declared thing is there. It has never meant that nothing else is.

So a name that leaves the manifest stays on every machine that ever applied it,
and stays there for good.

## What that has cost so far

Seven units left `[services]` in `55eab99` when everything was renamed from
`legion-*` to `console-*`. Nothing disabled them. Had they been left, the device
would have come up running two generations of every daemon --
`legion-controller` and `console-controller` both reading the pad, which is the
fault that reads to a person as "the buttons are flaky" and is very hard to see
from the desktop. What stopped it was `tools/console-migrate`, a script written
by hand for that one rename, run once, whose attic is still at
`/var/tmp/console-migration-20260829-234115` on the device. Its section 3.6 is
where the rule was first written down: *the manifest installs a name and never
sweeps one*.

Everything removed since had no such script. On the device, when this was
written, that came to ten programs in `/usr/local/bin` -- `console-pictures`,
`console-poke`, `home-place`, `keyboard-start`, and the six left from the era
when the on-screen keyboard was wvkbd. None of them was running and nothing in
the tree reached for any of them, so this time it cost about forty megabytes and
nothing else. That is luck about which things happened to be removed, and not a
property of anything.

## A migration

One file per change, under `migrations/`, named for the unix time of the commit
that needs it:

    migrations/$(git log -1 --format=%cd --date=unix).sh

The name is omarchy's idea and it is a good one: it sorts into history order
without a counter for anybody to keep, and two people writing a migration on the
same afternoon get different names without having to talk to each other.

The top of the file declares what it answers for, and the rest is a shell script
that does the work:

    # sweeps: /usr/local/bin/console-poke
    # sweeps: enabled legion-bar.service

    console-attic /usr/local/bin/console-poke

The `sweeps:` lines are read by the gate and the body never is. That separation
is on purpose: a script that moves `/usr/local/bin/osk` has not thereby said it
answers for `osk` leaving `[files]`, and a gate that decided by grepping the body
would go green on a migration that mentions a name in a comment.

Each is handed `migrations/attic.sh`, which is where `console-attic` and
`console-unenable` live, along with `CONSOLE_ATTIC` for the directory this run is
filling and `CONSOLE_HOME` for the home the desktop belongs to.

**Nothing is deleted.** `console-attic` moves. The rename's attic is still on the
device, which is the only reason anybody can now say that sweep did what it
claimed rather than merely that it ran; a deletion is the same operation with
nothing left to check it by.

## Why it cannot be forgotten

This is the part that is not omarchy's. There, somebody has to remember to write
a migration. Here the manifest is a file in git, so the tree can be asked what
left it:

    for everything the manifest has ever left on a machine
      that it does not leave there now
        a migration must claim it, or somebody must have written down why not

`cargo test -p console-migrations` is that question, and it fails `just ready`.
Delete a line from `desktop.conf` and the gate says
*`/usr/local/bin/music-panel` left `[build]` and nothing sweeps it*. The way to
green is to write the migration in the same commit as the removal, which is the
only moment anybody knows why the line went.

The escape is `migrations/left-on-purpose`: one entry a line with the reason
beside it. It is the right answer when a machine that applied the previous commit
is genuinely holding nothing -- everything the rename swept is in there, with the
attic named as the evidence -- and the wrong answer the rest of the time. A name
in that file is a decision somebody made and can be argued with. A name that is
simply never mentioned is nothing at all.

## What the rule is actually about

Not a line in `desktop.conf`. What the machine ends up holding.

`console_migrations::holds` is the whole of it, and it earns its place twice
over. `launcher` was a shell script in `[files]` and is a compiled program in
`[build]`; the line moved between sections and `/usr/local/bin/launcher` never
moved at all. And the manifest used to name the person whose desktop this is and
now writes `@user@`, which is filled in at apply -- so
`/home/ada/.config/waybar/style.css` and
`/home/@user@/.config/waybar/style.css` are one file. The first version of this
gate read the first change as five programs being abandoned and the second as
twenty-eight files being abandoned in a home, and both times what was wrong was
that it was comparing declarations rather than machines.

### Why git's rename detection is not the answer

It looks like it should be. `git log -M` already knows these are renames, and
says so with a similarity score: `R100` for the home path, `R077` for
`legion-bar.service` becoming `console-bar.service`. Both are renames of a file
under `files/`, and git cannot tell them apart.

They are opposite. Renaming `files/home/ada/…` to `files/home/@user@/…`
changed nothing about where the file is installed. Renaming
`files/etc/systemd/user/legion-bar.service` to `console-bar.service` changed
exactly that, and left the old unit on the machine -- which is the stranding this
whole document is about. A gate that trusted git's rename detection would have
concluded that the seven units needed nothing.

The identity that matters is not the source file's. It is the installed path's,
after the template is filled in, and that is what `holds` computes.

## Running them

    console migrate --pending    what this machine has not run, and nothing else
    console migrate              run them
    console apply                runs them first, before anything is installed

The record of what has run lives on the machine, at
`/var/lib/console/migrations/`, one empty file per migration. On the machine and
not in the tree because the question is about the machine: two devices a release
apart share a history and share every migration in it, and what separates them is
only how far each has got.

One empty file rather than one list, because a list is a thing to rewrite, and a
rewrite that is interrupted leaves a machine that has run a migration and
forgotten or has not run one and thinks it has. Of those two, forgetting is much
the worse -- every sweep is written to be safe to run twice, and `console-attic`
on a path that is not there does nothing -- so the marker is written *after* the
migration returns.

An apply runs them before it installs anything, because a migration exists
precisely because the manifest stopped naming something, and a sweep left until
afterwards would be deciding about paths a fresh install had just written over. A
migration that fails stops the apply, which is not the cautious choice but the
only honest one: what comes next is installing over a machine whose state nobody
now knows, and `console apply` is what people reach for when something is
already wrong.

## What is not swept

`[packages]`. pacman already keeps the better answer: a package the manifest
stops asking for falls back to being held as a dependency or by nothing, and
`pacman -Qdtq | pacman -Rns -` is the line that collects it. Sweeping a package
here would be this tree deciding something pacman decides better, and
`console_manifest::packages` already explains why the *reason* a package is held
matters more than its presence.
