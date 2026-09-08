# Which crate owns a thing

The names are settled elsewhere: `console-*`, a family sharing the word after
the prefix, a crate named for what it does. This is the question that comes
after the name, and it is the one that goes wrong quietly — when two crates
want the same knowledge, which of them keeps it, and what does the other one
ask for.

## A format is read in one place

`.desktop` files, systemd units and `desktop.conf` are the same shape: a
heading in square brackets, the lines under it, the next heading ending it.
What they have in common is that walk and nothing else. `[services]` means
something to the manifest and nothing to a desktop file; `Exec` means something
to both a unit and a desktop file and not the same thing.

So the walk is `console-core-ini-files`, which knows what a heading is and what
an `=` is and no more than that, and every format's vocabulary belongs to the
crate that understands the format: what a desktop file means is
`console-applications`, what `desktop.conf` means is `console-manifest-engine`.

The tempting mistake is to move the vocabulary down into the shared reader — a
`Section` enum with `DesktopEntry` and `Services` in it, a list of the keys
anybody has ever asked for. It looks like centralising and it is the opposite:
the reader grows a variant every time something new is read, every caller
depends on the names of formats it never opens, and the crate that was general
is now the place where three unrelated file formats meet. That is a shelf with
a better name.

The rule that stays out of trouble: **the general crate reads the shape, the
owning crate spells the words, and a call site names no strings at all.**
`DesktopEntry::read` writes `Type`, `Name`, `Exec` and the rest once, in the
crate that knows why they are spelled that way, and hands back a value. The
settings tab asks it what the file says instead of reading the file.

## A literal spelled twice means one of them is guessing

Before this was true, three crates walked bracketed sections themselves, each
carrying a flag for whether the line in hand was inside the part it wanted.
They had drifted, in the way that only shows up when somebody reads all three
at once:

- one required its heading; one took whatever group came first and could not
  say that it had;
- one honoured `NoDisplay=TRUE` and one only `NoDisplay=true`;
- one dropped the space either side of the `=` and one kept it in the key;
- a test answered a question about `[services]` by walking `desktop.conf` by
  hand rather than by asking the crate that reads it.

None of that was a bug anybody had filed. Two readings of one file disagree
silently and surface later as a feature behaving differently in two places,
which is a much longer walk back to the cause than an argument in the code
would have been.

So the second caller for a literal is when it moves, not when it is copied.
Copying is cheaper in the minute and it is the minute that does the damage: at
the second copy the two are already free to drift, and nothing in the tree
says which one is right.

## A path is a literal, and a default can be worse than a failure

Where a thing is, is a fact like any other, and it went the same way. Three
crates each wrote out the directories a `.desktop` file can be in, and by the
time anybody read all three at once they no longer agreed: two of them read
`XDG_DATA_DIRS` and the third had three directories spelled into an array, so a
machine that set the variable offered a different set of applications on the
settings tab than in the menu. Four crates each worked out where the person's
home is, and three of them answered differently when `HOME` was unset.

The homes are the part worth arguing about, because two of them answered
`/root`. A default that stands in for a missing answer is only harmless when
being wrong looks like being wrong, and this one does not: it does not fail, it
succeeds against the wrong person's dotfiles, reading settings nobody wrote and
writing settings nobody will look for. It is `unwrap_or_default()` on a
`Result` wearing a path, and it is answered the same way — the absence is
carried, so `console_core_places::home` is an `Option` and every caller says
what it does without a home. Usually that is nothing, and nothing is the right
amount: the menu still lists what `/usr/share` holds, it just keeps no counts.

So the places are `console-core-places`, and what is in it is the standard's
own vocabulary and no more than that: the base directories, and `applications`,
because the desktop entry specification is what names that directory rather
than this desktop. What a `.desktop` file *means* is still
`console-applications` — the general crate says where, the owning crate spells
the words, which is the same division as the one above.

One home is deliberately not in it. `console-manifest-engine` asks the machine
for `/home/{whoever}`, and that is a different question with a different right
answer: the engine runs as root and is asking whose desktop it is applying,
not where its own settings are. Two questions that would give the same answer
on most machines are still two questions, and folding them together would be
the shelf built out of a coincidence.

Every other crate asks here now, and the ones that had invented an answer stop
inventing it: a screenshot with nowhere to put a picture fails instead of
writing under `/root`, the panel's picture store is absent instead of root's
cache, the add-on is not packed for somebody who is not there, and a wait that
has nowhere to be written is not written. One fallback is still not the absence
-- the music folder looked for from the working directory -- because it is
reached through a function whose callers cannot yet say *there is no folder*.
The two that were beside it, the voice model kept under `/tmp` and the
wallpaper cache likewise, went when the bases below did: giving those callers
the words for it turned out to be four lines each, and `/tmp` is a worse
address than `/root` rather than a better one. It does not write into somebody
else's home, which is what made it look survivable; it writes where anybody can
stand in front of it, and a model downloaded there is a gigabyte nobody finds
twice.

## The four bases, and the desktop's own directory under them

The same drift a third time, one level down. Where this desktop keeps a
person's settings, what it remembers, what it holds for her and what it can
make again -- `~/.config/console` and the three beside it -- was spelled out in
about a dozen crates, each joining its own filename onto its own guess. The
guesses disagreed twice over. An unset `HOME` sent two of them to `/tmp`. And
an `XDG_*_HOME` set to nothing was a directory to two of them and no directory
to three, so the same empty variable put a panel's notes under the person's
home in one program and in whatever directory it happened to be started from in
another.

So `Base` is the four of them, each knowing what the environment calls it and
what it is when nothing is said, and the standard's own rule settles the empty
variable without a case for it: a value that is not an absolute path is not a
directory, so it is ignored. Empty is not absolute, and neither is the relative
path somebody meant to make absolute. `ours` is the desktop's own directory
under a base and it is one word rather than four spellings of `.config/console`;
what goes in it stays the owning crate's to name, so this crate has never heard
of `scale`, `buttons.toml` or `waited.jsonl`.

`ours_under` is the same answer for a home that is not this process's, and it
reads no variable. The device over ssh and a stage tree standing here are both
that: the environment on this side of the wire is not the one that will open
the file, and every caller of it names a machine rather than a process.

The migration that moves a machine off the old name is the piece that shows
this was worth doing. It held a list of the four directories the old name left
behind, and that list had to be kept in step by hand with every crate that
spelled one. It now works the list out from `Base::EVERY`, and the test beside
it pins the five moves, because a base that arrives after the rename must not
turn up in a migration for a name that was never under it.

What is not done is the toolkit. `glib::user_cache_dir` and
`glib::user_config_dir` are the same question with the same two answers in
them, and the files that ask one of them are every one a program drawn with
GTK, where the toolkit is already in the room. Each hands the directory to a
function that joins the rest of the path onto it, and those functions cannot
yet say *there is no directory*: the same sentence the fallbacks needed, which
is what makes it a sweep of its own.

`console-manifest-engine/tests/the_home.rs` and `the_places.rs` are what keep
both. Neither rule is a type error anywhere, so each is a test that reads the
tree for the literal and names whoever spells it. They look for the literal
rather than for `env::var` because the site that hid longest from a grep was
one that had wrapped the read in a helper of its own and asked the helper for
`"HOME"` -- and where a site is left standing on purpose, the test names it in
a list rather than saying nothing, because a guard that is silent about a
standing site cannot be told apart from one that never looked.

## An answer is a format, and the compositor gives four of them

The same drift again, and this time not in a file. The program is a variant --
`Program::Hyprctl`, with its origin crossed against `[packages]` by a test --
and that was the whole of what anybody owned. What came back from it was read
wherever it landed: the home screen and the wallpaper each counted the windows
on the workspace in front, the nested desktop read the monitors wherever it
needed them, and the settings panel looked for `"scale"` in the text with
`find` and took the digits after the colon, which is the first scale the answer
happens to spell whatever it belongs to.

They disagreed about failure, which is the part that had already gone wrong. A
compositor that would not answer was an empty string in one place -- and an
empty string reads on as a desktop with no screens and nothing on it, which is
a lie a caller cannot tell from an empty desktop. It was the covered-up answer
in another, which is exactly right where the wallpaper is asking and is not an
answer anybody else should be handed. So `console-compositor` asks once and
says how it failed, and what to do about a compositor that has gone quiet
stays with the crate that knows what it was going to do with the answer.

Asking and reading are separate there for a reason that is not tidiness. The
nested desktop asks a *different* compositor: same program, an environment
handed over, another machine's worth of answers. So the walk is over an answer
somebody already has, and who asked for it is the caller's business --
`console-test-desktop` hands in a command carrying the session it means.

The division above holds here as it does for a file. hyprctl's own words are
the questions, because they are hyprctl's and a second spelling of `layers`
would be a second name for a thing that has one; the answers come back in this
desktop's words. What is *not* in it is which namespaces are furniture and
which one the home screen draws under -- that is `console-onscreen`, spelling
this desktop's words over the walk, and it is the crate that would have to
learn a new surface next year.

## The half that is not extraction

Pulling the shared thing out is the easy half. The half that decides whether it
was worth doing is naming the owner, and the answer is never "a crate for
shared things". It is the crate that would have to change if the fact changed —
if `.desktop` files grew a key tomorrow, `console-applications` is what would
learn it, so `console-applications` is where the keys live.

When no such crate exists, that is what the new one is named after: the job,
never the category. `console-core-ini-files` reads ini files. A
`console-core-parsers` holding it and whatever came next would be a bag, and
everything wanting one of them would compile all of them.

None of this licenses extracting for its own sake. One caller is not a pattern,
and a crate created before its second caller exists is a guess about a future
that usually arrives in a different shape.
