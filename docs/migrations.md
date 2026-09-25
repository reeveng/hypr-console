# What the manifest cannot say

`desktop.conf` says what must be on this device. It has never said what must not
be. For a long time the engine had no state for that either: every state
`console check` printed was about a name the manifest *does* carry, and nothing
in the engine unlinked anything. So a name that left the manifest stayed on
every machine that had ever applied it, and stayed there for good.

Two answers now stand where that gap was. The engine takes back, by itself,
what its own recorded applies placed and today's manifest no longer names --
*What the engine takes back* below, and the one a removal gets by default. A
migration is the other, and it is what a removal needs when the work is more
than moving the thing the line named.

## What that has cost so far

Seven units left `[services]` in `55eab99` when everything was renamed from
`legion-*` to `console-*`. Nothing disabled them. Had they been left, the device
would have come up running two generations of every daemon --
`legion-controller` and `console-input-controller` both reading the pad, which
is the fault that reads to a person as "the buttons are flaky" and is very hard
to see from the desktop. What stopped it was `tools/console-migrate`, a script
written by hand for that one rename, run once, whose attic is still at
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

One module per change, under
`crates/console-manifest-migrations/src/history/`, named for the unix time of
the commit that needs it and listed in `history::EVERY`:

    git log -1 --format=%cd --date=unix

The name is omarchy's idea and it is a good one: it sorts into history order
without a counter for anyone to keep, and two people writing a migration on the
same afternoon get different names without having to talk to each other.

The head of the module argues for the sweep, and the rest is a list of steps:

    pub const MIGRATION: Migration = Migration {
        moment: Moment(1790200080),
        says: "sweeping the thumbnail maker under its old name",
        steps: &[Step::Attic("/usr/local/bin/files-thumbs")],
    };

A step is data, and `console-manifest-engine` carries it out: a path to the
attic, a unit stopped or disabled, a process ended, a line in a file rewritten,
a setting read out of one file into another. What a migration answers for is
what its steps move and disable, in the words `holds` puts it in, so the gate
reads the steps and nothing else. They were shell scripts with a `# sweeps:`
header once, kept apart from the body because a gate that grepped a script
would go green on one that mentioned a name in a comment. A step has no comment
to mention it in, and a step the engine does not know is one the compiler
refuses rather than one bash reaches halfway through.

A module written and not listed is a sweep that never runs, so
`every_file_is_a_migration_that_runs` fails when the two disagree.
`console-rename` writes the module for a rename and leaves the listing, and the
argument, to whoever reads it.

**Nothing is deleted.** `Step::Attic` moves. The rename's attic is still on the
device, which is the only reason anyone can now say that sweep did what it
claimed rather than merely that it ran; a deletion is the same operation with
nothing left to check it by.

## Why it cannot be forgotten

This is the part that is not omarchy's. There, someone has to remember to write
a migration. Here the manifest is a file in git, so the tree can be asked what
left it:

    for everything the manifest left on a machine before any machine recorded it
      that it does not leave there now
        a migration must claim it, or someone must have written down why not

`cargo test -p console-manifest-migrations` is that question, and it fails `just
ready`. It asks only about what the manifest carried before
`console_manifest_migrations::RECORDED_SINCE`, the commit time from which every
apply has been written down with the commit it came from. A name carried at or
after that moment is one the engine takes back by itself on any machine that
applied while it was named, so the gate leaves it to the engine. Before it, no
machine kept the record, and nothing but a sweep written by hand can answer for
a name that left then -- every one of which is already answered.

The escape is `migrations/left-on-purpose`: one entry a line with the reason
beside it. It is the right answer when a machine that applied the previous commit
is genuinely holding nothing -- everything the rename swept is in there, with the
attic named as the evidence -- and the wrong answer the rest of the time. A name
in that file is a decision someone made and can be argued with. A name that is
simply never mentioned is nothing at all. The engine honours it as well: a name
in that file is never taken back, which is what keeps the Firefox `profiles.ini`
the desktop once wrote from being taken from a person who has made profiles
since.

## What the rule is actually about

Not a line in `desktop.conf`. What the machine ends up holding.

`console_manifest_migrations::holds` is the whole of it, and it earns its place
twice over. `launcher` was a shell script in `[files]` and is a compiled program
in `[build]`; the line moved between sections and `/usr/local/bin/launcher`
never moved at all. And the manifest used to name the person whose desktop this
is and now writes `@user@`, which is filled in at apply -- so
`/home/ada/.config/console/palette.css` and
`/home/@user@/.config/console/palette.css` are one file. The first version of this gate read the first change as five
programs being abandoned and the second as twenty-eight files being abandoned in
a home, and both times what was wrong was that it was comparing declarations
rather than machines.

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
the worse -- every sweep is written to be safe to run twice, and `Step::Attic`
on a path that is not there does nothing -- so the marker is written *after* the
migration returns. The markers the scripts wrote carry their `.sh`, and are
read as the moment in front of it.

An apply runs them before it installs anything, because a migration exists
precisely because the manifest stopped naming something, and a sweep left until
afterwards would be deciding about paths a fresh install had just written over. A
migration that fails stops the apply, which is not the cautious choice but the
only honest one: what comes next is installing over a machine whose state no one
now knows, and `console apply` is what people reach for when something is
already wrong.

## What the engine takes back

Nix and Kubernetes' prune compare against what was last applied and remove the
difference. That needs two things this machine has: a record of which commit
each apply came from, which is `/var/lib/console/generations`, and the tree that
commit is in, which is `/etc/console`, a clone holding the whole history because
a deploy is a push into it. So what the engine placed here is `[build]`,
`[files]`, `[services]` and `[masked]` at every recorded commit, read with this
machine's own `machines.conf` block, and what it placed that the manifest has
stopped naming is that less today's. The engine's `pruning` module is the whole
of it.

`console check` prints each one under `left` before anything happens, and
`console apply` takes them straight after the migrations and before anything is
installed -- after, because a migration that claims a name is what answers for
it, and before, for the reason migrations run before an install. Taking means
what the sweeps written by hand have always done:

- a unit that left `[services]` is disabled and stopped, as `disable --now`
- a unit that left `[masked]` is unmasked
- a program that left `[build]` and a file that left `[files]` are moved into
  the same `/var/tmp/console-migration-<when>` attic a sweep uses, and systemd
  is reloaded when one of them was a unit

Only what the records say this engine put here, and only as it put it there:

- **A file is taken when it still holds what a recorded commit shipped**, with
  `@user@` filled in. One that holds anything else was edited on the machine;
  it is printed as *edited, kept* and left for a person, since whether an edit
  is worth keeping is the question `console save` exists for. A file no
  recorded commit can show is treated the same way, since nothing can say it is
  ours.
- **A `once` file is kept**: something on the machine is meant to rewrite it,
  so what it holds cannot say whose it is.
- **Anything a package now owns is kept**, which `pacman -Qo` answers. A
  package laying its own copy over our path is what took the touchpad once, and
  the engine taking the package's file would be that fault the other way round.
  Only pacman naming a package, or saying *No package owns*, is an answer; a
  pacman that could not run or said anything else leaves the path printed as
  *owner unknown, kept*, because a question nobody answered is not a no.
- **A program is taken whatever it holds.** It was compiled here and no commit
  holds what it was, so whether a package owns it is the only question it can
  be asked, and the attic is what keeps that from being a guess nobody can undo.
- **A name a migration claims, or that `left-on-purpose` names, is not
  touched**, because somebody has said what to do with it.

What this cannot reach is a machine with no record. One that last applied
before the generations began, and whose next apply comes after a name left,
never recorded that name and is not asked about it; one whose
`/var/lib/console/generations` went missing is the same. A recorded commit its
tree can no longer show -- a history rewritten under it -- is printed as unread
rather than guessed at.

### What is still written by hand

A migration is what a removal needs when moving the named thing is not the
whole of it, and most of the ones in the history are that. The player's sweep
carried the music folder kew had been told into this desktop's own setting
before it moved kew's configuration. The session keeper's took state
directories the manifest never named. The move out of `~/.config/hypr`
rewrote `zz-steamos-autologin.conf` for the session it now logs in through. The
profile the pad wore while a card was asking was written by the engine and never
had a line in `[files]` at all. None of that can be derived from a manifest, and
none of it was caught by the gate either -- it was caught by whoever was writing
the sweep, because the gate had sent them there. So the question at a removal
has not changed, only who is made to ask it: the engine moves what the line
named, and whether anything else is lost is the removal commit's to answer,
with a migration when the answer is yes.

That was the argument for leaving it as it was, and it is a real cost: a
removal is green now without anyone having written a word about the machine.
What it bought was the rest -- `console check` saying what an older manifest
left rather than only what this one wants, the unit that is still enabled
being stopped whether or not anyone remembered it was a daemon, and the file
somebody edited being left where it is instead of moved by a script that could
not see the edit.

A migration that claims a name keeps it from the engine, which is what to
write when the named thing must *not* simply go to the attic: a setting read
out of it first, a process stopped by name, a file edited rather than moved.

## What is not swept

`[packages]`, by a migration or by the engine. pacman already keeps the better answer: a package the manifest
stops asking for falls back to being held as a dependency or by nothing, and
`pacman -Qdtq | pacman -Rns -` is the line that collects it. Sweeping a package
here would be this tree deciding something pacman decides better, and
`console_manifest_engine::packages` already explains why the *reason* a package
is held matters more than its presence.
