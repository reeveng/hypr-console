# Explicit-Rust

A dylint suite for the rule this workspace is written to: no behaviour that
matters should be implicit. It is the spirit of Elixir's `{:ok, value}` put to
Rust -- a call either says what it returns or says how it failed, and nothing
important happens because of a `bool`, an `as`, or a panic nobody declared.

The rules after 026 say the same thing about cost. A line that allocates a
list, walks one twice over, runs a program per item or copies something in
order to lend it is a line whose price is nowhere on it, and a price nobody
wrote down is a decision nobody made -- which is the complaint the first
twenty-six make about types, arriving in the profiler instead of in the
compiler.

It is a workspace of its own, on a nightly of its own, excluded from the one
above by `exclude = ["tools/*"]`. Lint crates link against `rustc_private`,
which stable cannot do; `rust-toolchain.toml` pins the nightly and the
`rustc-dev` component it needs, and `cargo dylint` builds them on demand.

    just explicit        the rules, by kind, by count
    just explicit-gate   the rules the workspace already keeps, enforced

## The rules

    EXPLICIT001  a failure met is a failure said
    EXPLICIT002  infallible fns return Result<T, Never>
    EXPLICIT003  Result<T, !> is forbidden; the name is Never
    EXPLICIT004  unwrap / expect / panic / todo / unimplemented / unreachable
    EXPLICIT005  fallible values must be handled or propagated
    EXPLICIT006  Option is for `may not exist`, not for errors
    EXPLICIT007  no bool return values
    EXPLICIT008  no bool parameters
    EXPLICIT009  discarded #[must_use] values must be `let _ = …`
    EXPLICIT010  no implicit numeric coercion
    EXPLICIT011  no `as` casts
    EXPLICIT012  unsafe must carry a // SAFETY: comment
    EXPLICIT013  a block that decides something gets a blank line around it
    EXPLICIT014  no indexing or slicing; ask with `get` and meet the `None`
    EXPLICIT015  no bare integer arithmetic; the policy has a name
    EXPLICIT016  no wildcard arm on a match over an enum
    EXPLICIT017  `?` stands alone: the whole of a statement, never buried
    EXPLICIT018  an `allow` carries its reason, in the attribute
    EXPLICIT019  no `if`; a decision is a `match` that names both outcomes
    EXPLICIT020  no comments; a `//!` head and a `// SAFETY:` are the two that stay
    EXPLICIT021  no waiting on the clock; ask for the thing, and keep asking
    EXPLICIT022  no settling on a number of seconds on the handheld; ask it
    EXPLICIT023  no `let … else`; the `match` goes in the initializer
    EXPLICIT024  no two parameters the compiler would take in either order
    EXPLICIT025  no number standing for a case the type does not declare
    EXPLICIT026  an environment variable is read in the crate that owns it
    EXPLICIT027  no collection made only to be walked again
    EXPLICIT028  no list searched from inside a loop
    EXPLICIT029  no program run and no file read from inside a loop
    EXPLICIT030  no copy allocated only so a borrow of it can be handed over
    EXPLICIT031  no list walked by counting to its length
    EXPLICIT032  no iterator walked to its end to answer a yes or a no
    EXPLICIT033  no `None` answered by a value nobody wrote down
    EXPLICIT034  an answer that is the point of a call says `#[must_use]`
    EXPLICIT035  a thread's lifetime is said where the thread is started
    EXPLICIT036  no program named by a string literal
    EXPLICIT037  no number laid out in the machine's own byte order
    EXPLICIT038  a fault is a type, not a sentence
    EXPLICIT039  the clock is read at the edge and handed inward
    EXPLICIT040  a file is written whole or not at all
    EXPLICIT041  a program written to the contract says what it prints
    EXPLICIT042  a program ends by returning from `main`
    EXPLICIT043  a topic listened to is a topic deafened
    EXPLICIT044  a function decides from what it was handed
    EXPLICIT045  a conversion names the type it becomes
    EXPLICIT046  a jump out of a nested loop says which loop it leaves
    EXPLICIT047  a closure takes what it writes, rather than holding it
    EXPLICIT048  a type spells no state it does not have

All of them are written. Each is one crate with a `ui/` case beside it.

The level in the lint's own source says which tier a rule is in. A `Deny` rule
fails the gate; a `Warn` rule is one written ahead of the code, printing its
remaining distance on every run so it is never out of sight. A rule moves from
`Warn` to `Deny` in its own crate when the last call site that broke it is
fixed, and by the ratchet's one law it never moves back.

Nothing stands warned. 047 and 048 were the last two in that tier, written
ahead of the code the way this README said a rule would be, and both came out
together; what each of them cost is further down. They and 045
are read out of Kast, which is stricter than either. 045 is EXPLICIT010's
argument with the numbers taken out of it: a conversion that names neither end
says nothing at the call site, and Kast has no implicit coercion at all -- a
cast there is a value somebody wrote, so the pair is declared and both halves
are named. It came out in one sweep, because almost every site it found was a
string literal becoming a `String` and a path spelled as one, where the
destination was the only thing the line was not already saying; the two that
wanted more than a spelling were a parameter taking `impl Into<String>`, which
is the permission to convert handed to a caller who then cannot write the
conversion down, and a `PathBuf` turned into an `Option` to be matched against
a `None` that could not happen.

047 was the one with the furthest to go: Kast's closures capture the pointer
and not the access, so `() -> () with mutable_access[x]` says at the call that
calling writes, and what this rule asks for is the argument that says the same
thing in a language with no contexts in it. What it found was one question
repeated rather than a list of independently wrong lines, which is why it stood
warned for as long as it did and why it then came out in one pass. Some of the
sites were an iterator word standing in for a loop -- a `try_for_each` over
something that writes, a `filter` that inserts as it goes, a `map` that asks a
machine -- and those became the `for` they always were, where what is written
is a statement rather than a capture. The rest were the functions that take a
closure and hand it nothing: a wait, a stretch of an apply, the sentence a
failed check prints, the line a run draws quietly. Each of those has a second
spelling now, `_handed`, that takes what the question is asked with and hands
it in at every ask -- EXPLICIT044's own sentence, said about a closure instead
of a function -- which is what `Device::until` had been doing since it was
written,
and the one that could not have a closure at all -- the stretch, because the
thing it hands over is a borrow of the bar itself -- is two halves in one
function body instead. It still catches the
borrow and not the `move` closure that owns what it writes, which is the
larger half and wants the page #108 in the backlog is asking for. 046 arrived
warned beside them and came out in one pass, which is the shortest a rule has
stood there: it is the smallest possible version of Kast's other argument --
Kast has no anonymous non-local exit, a jump names the block it returns to --
and what it found was a jump in a nested loop that names neither loop, in
about a dozen crates. Every one of them meant the loop it was standing in, so
the sweep was a label on that loop and the same word on the jump, and the
reader stops counting braces. Before it, 039, 040, 041 and 044 arrived warned
together and have all come out; 042 and 043 came in denied beside them, 042
with the tree breaking it in a handful of binaries, all of them an error arm
at the end of a `main`, and 043 green, which makes it the third ratchet after
034 and 037. Of what came after 023, 025 and three of the five after 033 also
arrived denied, and the others arrived with the tree breaking them -- which is
the thing this README used to say would be the only way to put anything back
in that tier, and it was right. None of them was one sweep: some of what they
found wanted moving, and some of it wanted a sentence at the site saying why
it is where it is, and the rule was doing its work either way. Neither answer
is available to somebody who cannot see the list, which is what the tier is
for.

The four came out together and each one came out the same shape: where somebody
at the edge could hold the answer, it moved there and became a parameter, and
where the process really is the only thing that can hold it, the site carries
the allow and EXPLICIT018 makes the reason say why. 040 was the largest by a
long way and the least argued-with -- nearly every site was one call and two
words -- and the two that were more than that are worth knowing: the session
that decides what comes back after a reboot was streamed into an open file a
line at a time and is now built whole and written once, and the add-on packer
had written its own beside-and-rename with neither `fsync`, which is the half of
the argument `console-core-atomic-writes` was written for. 044 found a fifth
crate quietly deciding what marks the top of this tree, and it now asks
`console_repository` like the other four. Each rule's own head says what it
found and what carries its allows.

038 was the last one standing there before these, and the largest arrival since
019: not a sweep at all but an enum per crate naming the ways its calls fail,
with every call site moved onto it. 024 went the same way before it, a run of
new types rather than a sweep.

039 to 044 arrived together and are one argument, the way 026 to 032 were. The
rules before them are about what a call says it returns and how it says it
failed. These are about the third thing a call has and never writes down: what
it needs in order to run at all. A function that reads the clock, opens the live
file, prints beside its own contract or reaches into a value the whole process
shares has an input nothing handed it, and the tree's answer at every site that
already got this right is the same one -- hand it in. 041 narrowed twice on its
readings of the tree, the way 029 narrowed to programs on its own: first to
stdout, because what it found on stderr was almost all a fault or a usage line
going to the journal, which is where EXPLICIT038 already sends one; then to the
crates that name the doings rather than the crates that name the contract at
all, because what was left was mostly `Topic` in a bar module whose whole output
is one line down a pipe. `console-cpu-boost`
decides nothing from the clock because `asked(now)` takes the instant and the
binary does the reading; `console-core-atomic-writes` is where a file is written
whole; `Doing::Print` is on the list a program hands back. What the rules do is
stop the other spelling.

042 and 043 are the two ends of that. 042 is a program leaving without
unwinding, which drops nothing and so keeps nothing's promise -- a child started
`Alongside` is killed by a `Drop` and by nothing else. 043 is the one obligation
here a single line cannot answer, because its two halves are in two places on
purpose, so it asks the crate rather than the call and says in its own head why
that is weaker than the type it is standing in for. The backlog carries the rest
of that argument.

028 and 029 grew a half at the same time and for the same reason. Both stopped
climbing at a closure, which meant `wanted.iter().filter(|one| held.contains(one))`
and `named.iter().map(|one| Command::new(one))` were the loop each rule is
against, spelled as a word, getting past. They now ask what the closure was
handed to: the iterator words that run a body once per item carry the climb over
the list they were called on, and the families that share those words and run
once -- `Option::map` and its kin -- are told apart by the receiver rather than
by the word. What that found in a tree where both rules were already green was a
run of real ones and rather more that wanted a sentence saying the list is a
handful, which is the shape every arrival here has had.

Of the rules that came out, 023 was the last, and it went the way 019 did, by
sweep; before it, 022 came out on a change to
what a wait may carry rather than a change to any check: `until` now carries
the fault its question carries, so a question that reads the screen is one a
wait can be given.
018 came out the quiet way: it was written after the
policy it names was already kept everywhere, so its last call site was answered
before the rule existed to count it. 017 came out the long way, which is the way the ratchet
expects -- every buried `?` in the tree lifted into a `let` of its own, one
crate at a time. 020 came out the way that should not be necessary: what it
forbids had been swept out of the tree once already, by hand, and was back in
most of the crates by the time anybody looked. 019 came out the longest
way of all, because it was the rule the whole tree broke: every guard clause, every `if let`, every `else if`
chain rewritten as a `match` that names what the other path was.

**023 is the part of 019 that was let through.** When the guard clauses were
rewritten they were rewritten into `let … else`, and 019's own head said that
was fine: both outcomes written, one of them required to leave. Only one of them
is written. `let Some(held) = asked else` never says `None`, which is the exact
complaint 019 makes about `if let`, and the sweep that was supposed to name
every path left the largest family of them spelled `else`. So it is a rule of
its own, and the tree broke it in the same way and on the same scale. It stood
warned while that was walked back into a `match` on the right of the `let`:

    let said = match std::fs::read_to_string(&at) {
        Ok(said) => said,
        Err(fault) => return Err(fault.to_string()),
    };

That is not the nesting the guard clause was avoiding. A `match` in an
initializer is an expression, so the binding stays where it was and nothing
after it moves right -- which is also why EXPLICIT013 does not ask it for a
blank line. What an irrefutable `let` does is a different thing and is not
touched: `let Ok(at) = at(&home);` has no `else` because `Result<T, Never>` has
no `Err`, and that is 002 being kept rather than a case being hidden.

Three of the first twelve cannot read their own rule off a signature, so they
read it off the code instead, and it is worth knowing which way:

  - **001** cannot see that a function is fallible. It watches for a function
    that swallows somebody else's error -- `unwrap_or`, `unwrap_or_else`,
    `unwrap_or_default`, `ok`, `is_ok`, `is_err` on a `Result` -- while its own
    return type is not a `Result`. That is a function that met a failure and
    decided not to mention it.
  - **006** cannot see intent either. It watches for `Result::ok()` and
    `Result::err()`, which is the exact moment an error becomes an absence.
  - **010** has almost nothing to catch, because Rust has no implicit numeric
    coercion. What it catches is the coercion hidden behind a trait:
    `x.into()` between two numeric types, which reads as the same number and is
    a different range.

**013 is not about types at all.** The twelve before it ask what a signature
promises; this one asks what a screen says before anything is read. An `if`, a `match`, a `while`, a `for`, a `loop`, a `let … else`, an
`unsafe` block and any declaration with a body under it are each set off by a
blank line above and below, so the shape of a function can be seen without
reading it. Elixir gets that spacing for free from `do`/`end`; Rust's braces
are quieter, so it has to be asked for.

It is about statements only. An `if` or a `match` on the right of a `let` is an
expression in the middle of a line and is left alone. `use`, `const`, `static`,
`type` and `mod name;` are left alone too: those are written as packed lists on
purpose, and a gap between every line of one would break apart a block that is
read as a unit.

A comment belongs to the block it explains, so the blank line goes above the
pair rather than between them, and an attribute is read the same way.

It is also the one rule in the suite a machine can apply. What it asks for is a
newline at a place the rule has already walked to, so it offers exactly that,
machine-applicable:

    cargo dylint --fix --lib explicit013_breathing_room

The other rules do not, and will not. Each of them is asking for a decision --
a name for the case that was left out, a sentence saying why an `unsafe` is
sound -- and a machine that guessed one would be writing the thing the rule
exists to prevent. A blank line is the one thing here that changes what a file
says to a reader and nothing at all to the compiler.

**002 was the last rule to reach `Deny`, and it took the longest.** The other
nineteen described a workspace that already kept them by the time they were
written. This one described one it was walking towards, and it waited longer
than any of them for a reason none of the others had: it had nowhere to point.
`Result<T, Never>` needs a `Never`, and there was no such type here at all.
`console-core-never` is that type -- an enum with no variants, so the `Err` a
caller does not write is a case the compiler agrees cannot arrive.

It was registered `Allow` for as long as that was true, which is not what the
warned tier is for. `Allow` prints nothing, so the rule was real only in the
sense that its source existed; the distance had never once been counted. It
went to `Warn`, which counted it, and stayed there while the tree was carried
across a crate at a time. What the count is read with, for whatever rule stands
in that tier next:

    just explicit

The command this file used to give for counting it did not work. `cargo dylint
--all -- --all-targets -- -W explicit002_infallible_result` passes `-W` past a
second `--` to `cargo check`, which takes no trailing arguments and refuses the
whole run -- so the one number that would have said what adopting the rule
costs was never printed by anybody who tried. A lint level for a whole run goes
through `RUSTFLAGS` or through the tier in the lint's own source, and the tier
is the honest place for it.

What it cost was not the signatures. Every function that gains a `Result` hands
its callers one to meet, and 005 and 017 between them say how: `let answered =
asked()?;`, one call to a statement, everything nested lifted out. So the rule
was adopted a crate at a time, the way 019 was, rather than swept -- and the
last crates to cross were the ones with the most arithmetic in them, because
`console-core-number-conversion` reaches every screen this desktop draws.

**002, 007 and 008 skip a method that implements a trait.** All three are about
a choice: a signature that says `bool`, or says nothing at all, where it could
have said what it meant. In an impl of somebody else's trait there is no choice to skip past --
`PartialEq::eq` answers with a `bool` because the trait says it does, and a
type that wants to be compared has no other way to say so. `Drop::drop` answers with nothing and
`Default::default` answers with `Self` for the same reason. Denying any of it
would leave an allow on every such impl, which is a rule that has stopped
asking anything. Where the choice was made is the trait, and a trait written
here is linted where it is written.

002 skips two more for the same reason, and they are worth naming because they
are the only places in this tree where the shape of a function was never on
offer. An `extern "C"` function belongs to whoever calls it, which here is a
signal and GTK, and `Result` does not cross that boundary. And `fn main` has no
caller at all -- the rule's argument is that a caller should not change shape
when the thing it calls learns how to fail, and the entry point has none to
spare. It is asked by the entry point's own def id rather than by the name, so
a helper somebody called `main` is still asked.

What a function answers is read after the aliases are resolved. `Done` in
`console-test-stages` is `Result<(), Why>`, and every check body in the tree is
written in it; read as it is written, those were the one thing this rule cannot
ask for -- a function that already says how it fails, told to say it again in a
way that cannot.

**021 is the newest, and it is the first rule about time.** Everything before it
is about what a signature says; this one is about what a program does while it
waits. `thread::sleep(SETTLE)` is a claim that the machine will be finished by
then -- checked once, on the machine it was written on, and relied on by every
other machine that ever runs it. What is being waited for almost always has a
name and can be asked: a window that is drawn, a child that has exited, a lock
that is free, a monitor that has taken its mode, a screen that has stopped
changing. So the rule denies `thread::sleep` and its kin, and denies the glib
timers with them, because a callback that runs because time passed is the same
assumption wearing a main loop.

It has somewhere to point, the way 002 needed `console-core-never`.
`console-waiting` is the loop that asks: a patience, a question, and an answer
that says which of the two ways it ended. Every wait in the tree that was a
number became one of those, and the fixing found faults rather than only moving
lines -- `make_the_screen` was three sleeps in a row, and each of them is now a
question the compositor answers, so the nested desktop no longer spends 1.8
seconds being sure and then coming up wrong on a busy machine anyway.

What it does not catch is deliberate. `recv_timeout` is a wait on a real event
with a bound on the patience, which is the shape the rule is pushing towards
rather than away from. And a `sleep` inside a shell script this tree writes into
a string is out of reach on purpose: 020 reads the text of a file because a
comment is not in the syntax tree, but a rule that read string literals looking
for another language's grammar would be a second mechanism with a second set of
edge cases, and the same argument that keeps 020's rule in one place keeps this
one in the other.

The allows it left behind are the point of the rule rather than a hole in it. A
click is a press and a release with a gap between them; a note is shown for a
moment; a backoff is the waiting between two tries; the emulator is playing back
how long somebody held a button. In each of those the elapsing *is* what is
being asked for, and the reason at the site says so. The test the README already
gives applies unchanged: the allow is right where the harm the rule names is
absent, and "poll instead" is not a thing that can be said about a duration
somebody wanted.

One allow is worth naming here because it is the rule's own foundation.
`console_waiting::between` is a `thread::sleep`, and it has to be: the gap
between two questions about a thing nothing will announce is the one wait a poll
cannot poll for. It is one site in the tree rather than fifty, which is what
makes the rest of them answerable.

`console_test_stages::device::settle` is the other one. It is the gap the
device stage's own `until` is built from, so it is the same site as above --
but it is also `pub`, and most checks once reached for it directly with a
number in hand. The rule cannot see the difference between the two uses: from
where 021 stands there is one sleep there and it is already excused. That is
what 022 is for.

**022 is 021 asked at the call instead of at the sleep.** An allow excuses a
site, and a `pub fn` around an excused site turns one of them into as many as
anybody cares to write. `console_waiting::between` is private, so it is still
one site; `console_test_stages::device::Device::settle` is public, and every
check that reached for it is another wait 021 cannot see, because each of them
is a call to a function whose sleep already carries a reason -- a reason that
is true of exactly one caller.

So the rule is asked where the decision is: at the call. It resolves the method
rather than reading its name, because `settle` is a word four other types in
this workspace use for four other things -- laying files down, a turn of the
emulated daemon, a boost that has expired, a screen that has redrawn -- and the
checks call two of them in the same file. `Here::settle` takes turns of a
daemon this process is running and is not a clock at all; the rule is silent on
it, and on the flows, which is most of what a name-matching version would have
shouted about.

It does not exempt tests, which every other rule in the suite does. The checks
that break it are written in test targets, and a rule that skipped those would
be counting a tree nobody runs.

`Device::until` carries the allow, and it is the same sentence as
`console_waiting::between`'s: the gap between two questions is what a poll is
built out of. The checks have all crossed and the rule is denied, so what
`settle` is now is a word said deliberately, wherever it is said, with the
reason beside it. It stays `pub` because all but the gap are written in another
crate, which is a smaller thing than it was: the rule stands between a number
and anybody who would name one without saying why. `docs/checks.md` argues for
what a crossed check looks like.

## The six that came later

014 through 019 arrived together, and each is one of the first thirteen said
again about a place the first pass did not look. 020 came after them on its
own, and is not about the code at all.

**014** is 004 about syntax instead of calls: `xs[i]` and `&s[a..b]` are
panics nobody declared, and `get` turns the absence into a value that 005 then
makes sure is met. Const contexts are left alone, exactly as 015 leaves them:
an index the compiler evaluates and finds out of range fails the build, which
is a failure with a name, at the right time, on the machine that has a screen.

**015** is about the one behaviour in the language that differs by build
profile: bare `+`, `-`, `*` and the shifts panic in debug and wrap in release,
and `/` and `%` panic on zero in both. `checked_*`, `saturating_*` and
`wrapping_*` each name a policy at the site. Const contexts are left alone --
arithmetic the compiler evaluates fails the build, which is a failure with a
name, at the right time -- and a negated literal with them, because `-1` is how
a negative number is written rather than a subtraction anybody performs.
`console-core-number-conversion`'s hand-rolled float decoder is what this rule
looks like adopted early.

Adopting it everywhere else said one thing worth writing down. Nearly every
site wanted `saturating_*`, because nearly every number in this tree is a size,
a coordinate or a count, and the nearest one that can be drawn is a better
answer to a value off the end than a panic in one profile and a wrap in the
other. Where it wanted something else, it wanted it for a reason that had to
be written beside it: `wrapping_*` where the distance is provably under the
width or the value is a seed, `checked_div` and `checked_rem` where the
divisor came from outside and the `unwrap_or` says what a zero comes to. A
policy nobody can say the reason for is the site to look at twice.

**016** is about time: `_ =>` on an enum decides variants that do not exist
yet, silently, at every catch-all in the tree. Named variants make a new
variant a compile error at every site that has an opinion about it. A foreign
`#[non_exhaustive]` enum is exempt, because there the compiler demands the
wildcard and the choice this rule is about does not exist -- the same reasoning
that lets 007 and 008 skip a method implementing somebody else's trait. A
guarded arm is left alone: it covers nothing by omission, and the unguarded
arm it falls through to is the one that answers for the rest.

**017** is 013's kin: `frame(settle(x)?, y)` is an early return with no shape
on the screen. `?` may be the whole of a statement, the right side of a `let`,
a `return`, a block's tail, or the whole body of a `match` arm -- the
positions a scanning eye already reads as an exit. Everything else lifts out
into a `let` of its own.

**018** moves this README's own oldest policy into the attribute: a rule is
allowed only where the harm it names is absent, and the allow says which. What
used to be a comment above the attribute is now
`allow(<name>, reason = "…")`, where the compiler keeps it next to the site it
excuses. `expect` is held to the same sentence; `warn` and `deny` hide
nothing and have nothing to explain.

**019** is the strongest stance in the suite: `if` is forbidden. An `if`
without an `else` decides the false path by omission; an `else if` chain is a
`match` that lost its scrutinee; an `if let` names one case and waves at the
rest. `match cond { true => …, false => … }` puts both outcomes on the screen
with a name on each, and `match value { … }` says what an `if let` was
asking -- where 016 then asks that the variants be named too. `let … else` is
not an `if` and is left alone: both of its outcomes are already written, and
one of them is required to leave. A `while` desugars to an `if` nobody wrote
and is not charged for it.

**020 is not about Rust at all.** It is about the other language a source file
is written in, the one nothing compiles. Every `///` and every `//` in the
workspace was deleted once, on the rule that a sentence beside a line of code is
a second statement of the same thing that nothing keeps true; what was worth
keeping went to `docs/`. Then they came back, a `///` at a time, because the
rule was a paragraph somebody had read and not a thing the build could fail on.
That is the whole argument for this rule existing: it is not a stronger claim
than the paragraph made, it is the same claim with a gate under it.

Two stay. A `//!` module head, because what a file is *for* is the one thing no
line inside it can say, and a `// SAFETY:` reason, because EXPLICIT012 will not
pass an `unsafe` block without one -- a rule that forbade it would leave every
`unsafe` in the tree between two rules that cannot both be kept. A reason often
runs past its first line, so it is the run that is allowed once its first line
says `SAFETY:`, which is how 012 reads it too.

It reads the text of the file rather than the syntax tree. A comment is not in
the tree: `///` survives as an attribute and could be caught there, but `//` is
thrown away before any pass runs, so half the rule would be written against the
text anyway -- and a rule enforced by two mechanisms is a rule with two sets of
edge cases. It uses the compiler's own lexer, which knows that a `//` inside a
string is a string.

The head it points at is not itself judged. Whether a `//!` earns its place is a
reading, and a lint cannot do a reading; the backlog carries that as work for a
person.

## Where a rule does not apply

A rule is allowed at a call site only when the harm it names is absent there,
and the allow says which. That is a narrower test than "the rule is
inconvenient here", and it is the only test: a rule nobody may ever allow is a
rule people work around in silence, and a rule anybody may allow is not a rule.

Every allow in the tree is one of these, and each carries its reason:

  - **`chooser::showing`.** EXPLICIT011. A function turned into the number
    `signal` takes it as. No trait does that, and the way out is a signalfd,
    which changes how a running process is asked to go away and wants deciding
    on its own rather than inside a lint sweep.

  - **`console-core-words`' derive.** EXPLICIT002. A derive answers the
    compiler in token streams, and a `Result` does not cross that boundary any
    more than it crosses an `extern` one -- which is the exemption this rule
    already writes for itself, arrived at by a road none of its four kinds
    names. `#[proc_macro_derive]` decides the signature, so there is nothing to
    choose here, and a derive that cannot do what it was asked says so in a
    `compile_error!` where somebody reading the build will see it rather than in
    an `Err` with no caller to meet it. It is the one function in the tree that
    is reached this way, which is why the exemption is written here rather than
    into the lint: a `proc_macro_derive` cannot be spelled in a `ui/` case,
    because it is only a derive inside a crate that is one, and a branch in a
    lint that no case presses is worse than an allow that says what it is for.

  - **The waits.** EXPLICIT021, and there are more of these than of anything
    else, because the rule is the only one in the suite whose exception is a
    whole category rather than an accident. Each is a site where a duration is
    what was asked for: `console_waiting::between`, which is the gap a poll is
    built out of; `console-core-reconnect` and the retries that share its
    shape; the press, the tap and the hold, where a compositor is being told
    how long a person's finger was down; the coalescing windows, where the
    thing being waited for is a burst of events stopping; the cadences a
    program falls back to when the channel that used to wake it is gone; and
    `console-input-gamepad`'s `Clock`, which is playing a capture back at the
    speed it was made. The rule's own README section says how to read one that
    is not on this list.

  - **`Device::until`, and the calls that keep a number on purpose.**
    EXPLICIT022. The first is the gap between two questions to the handheld,
    which is what a poll is built out of and is the same sentence as
    `console_waiting::between`'s. The rest are the d-pad held down so the
    highlight walks, where the duration is the press, and the checks whose
    whole assertion is that nothing happened, where there is no question to ask
    and the number is how long the desktop is given to do the wrong thing.
    Every other caller of `settle` named a number and asked nothing, and each
    of those has crossed; `settle` stays `pub` because all but the gap are
    written outside the crate that holds it.

The form is `#[cfg_attr(dylint_lib = "<name>", allow(<name>))]`, which is inert
under an ordinary build. `dylint_lib` is an unexpected cfg there, so the crate
declares it in its own `[lints.rust]`; that keeps the root manifest out of it.

One allow that used to be on this list came off it, and how is worth keeping.
`checking::ought` took an assertion's condition as a `bool`, and the argument
for it was that the condition is spelled out at the call site -- `ought(came ==
0, ...)` -- so nothing is left to look up. That held for the comparisons and
not for the rest: `ought(stage.drawn(PATIENCE), ...)` is a name whose truth
condition a reader has to go and find, which is the harm the rule names,
arriving by the road the argument said was closed. It was replaced by a family
of named checks -- `same`, `not_same`, `more_than`, `less_than`, `empty`,
`not_empty`, `every`, `seen`, `happened` -- that put the question in the call.
The lesson is that an allow's reason has to hold at every call site, not at the
ones that come to mind.

`console-core-number-conversion` came off the same way, and it is the better
story because the reason was true. Float to whole number and count to float
genuinely have no conversion in the standard library -- no `From` and no
`TryFrom` in either direction, and the compiler says so if you ask it. What the
allow's reason did not survive was the difference between "the standard library
has no conversion" and "there is no way": `f64::to_bits` is safe and total, and
once a float is in pieces the rest is integer arithmetic. So the crate now
decodes the exponent and shifts the significand, and saturating is three cases
in a type rather than something the arithmetic is trusted to arrive at.

What made that safe to write by hand is the exemption below. A hand-rolled
float decoder is exactly the kind of code whose bug is a slightly wrong number
rather than a crash, so the tests hold every family against `as` itself, over
the named edges -- both NaNs, both infinities, the subnormals, the ends of
every width and a half either side of them -- and a long sweep besides. Tests
may write `as`, so the claim the module comment used to make is now a claim the
suite checks. An allow that is replaced by a test is the ratchet working the
way it is meant to.

Three things were considered and rejected as ways around a rule, and they are
written down because each looks reasonable until it is said out loud. Wrapping
a `bool` parameter in a closure to satisfy EXPLICIT008 -- the rule is then one
that anybody can pass by wrapping, which is no rule. Taking the `bool` as
`self` in a trait impl so it resolves as `Self` -- that passes on an accident of
how the lint reads types, silently, and turns a later improvement to the lint
into a mystery failure. And a newtype whose only purpose is to carry a `bool`
past the check, which buys notation and still needs the allow.

## What an error says

EXPLICIT001 and EXPLICIT006 both end the same way: a call that used to swallow
a failure has to say what happened instead. The error is a type of the crate's own now, so what it says is a sentence
somebody reads in a journal
rather than a type a compiler checks -- which is what EXPLICIT038 walked the
tree out of, and the last sub-section here is what that does to this one. Three things about that sentence are not the call site's own choice,
and each is written here because it was got wrong first.

### A fallback is not a failure, and the rule cannot tell them apart

001 watches for `unwrap_or`, `unwrap_or_else`, `unwrap_or_default`, `ok`,
`is_ok` and `is_err`. That is a list of calls, not a list of mistakes. Some of
them are a failure somebody dropped. Others are a decision somebody made, and
the decision is right:

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());

is not a program ignoring an error. It is a program saying that a session with
no `HOME` still has somewhere to keep its files. Converting it to

    let home = asked("HOME")?;

answers the rule, and turns a daemon that used to start into one that exits --
on the machine where it matters, which is a unit whose environment is whatever
the unit file says and nothing else. Both spellings lint clean. One of them is
a desktop that does not come up, and it says so at boot rather than in review.

So the rule does not ask for a `Result`. It asks that the failure be met, and a
`match` with a named `Err` arm meets it while keeping the default:

    let home = match std::env::var("HOME") {
        Ok(home) => Some(home),
        Err(fault) => {
            eprintln!("controller-desktop: HOME: {fault}; no button anybody moved will be read");
            None
        }
    };

That is the form to reach for where the default is deliberate, and it is how
`console-input-controller` came to zero without making its callers fallible.
Which of the two a site wants is a question about the site -- what should this
program do on a machine where this is missing? -- and the shape of the call that
is there now does not answer it. Every site gets read for that before it is
converted, because the conversion that is wrong is the one that lints.

### Returning `Result` used to be a pass, and is not one now

001 began by asking only whether the *enclosing* function returned `Result`: if
it did, it had already told its caller it could fail, so swallowing inside it
was a choice it was entitled to make. That reasoning holds for the caller and
not for the failure. Saying somewhere in a signature that you can fail is not
saying that you did, and what the exemption sheltered was every shape that
reads as a decision and is not one -- a `strip_prefix` handing back the whole
path, a clock reading becoming zero, a file that could not be read becoming an
empty one -- sitting inside functions whose `Result` was about something else
entirely.

Closing it cost five sites, which is what the exemption had been worth in the
whole workspace by the time anybody counted. Each of them was already doing the
right thing; none of them said so. The rule now asks the same question
everywhere, and the answer is the `match` above.

### The sentence names the purpose, not the mechanism

An error here is read by somebody holding a journal and not the source.
`hyprctl failed` names the program that broke and not what the desktop was
trying to do, which is the half that says whether it matters.

    .map_err(|fault| format!("asking hyprctl what is on the screen: {fault}"))

The mechanism is already in `{fault}`, which carries the operating system's own
words about it; a sentence that names it again has spent itself saying what the
next clause repeats. The purpose is also the part that differs between two call
sites running the same command, which is why it cannot be written anywhere but
at the site.

### A helper for a repeated fault belongs to one crate

Where a crate meets one kind of fault in several places and answers it the same
way each time, a private helper is right. `console-onscreen` has one:

    fn asked(name: &str) -> Result<String, String> {
        std::env::var(name).map_err(|fault| format!("{name}: {fault}"))
    }

It folds an unset variable and one that is not text into a single answer, and
its comment says why: to that crate's callers neither is a thing they can do
anything about beyond saying so.

It does not belong in a shared crate, and `console-input-controller` is the
reason. That crate reads the same `VarError` and pulls the two cases apart,
because there they are not the same thing at all -- a device nobody pointed at
is ordinary and is most of the time, while a name set to something that is not
text is somebody trying to point at a device and missing, and it used to arrive
as the same silence. Both crates are right about their own machine. A shared
helper would have to pick one of them, and picking either makes the other one
wrong.

So a helper of this kind stays in the crate whose policy it carries. What is
shared is this page: the grammar of the sentence, and the question in the first
section that every site has to answer for itself.

### And then 038 reopened the first sentence of this section

"The error is a `String`" was the standing answer when 001 and 006 were
written, and EXPLICIT038 is the argument that it should not be. Nothing above
is withdrawn: every word of it is about the *wording* of a fault, and the
wording is exactly what survives the change. An enum naming the ways one call
can fail, with `Display` on it, says the same sentence into the same journal --
what it adds is that the caller reading it is no longer the only thing that can
act on it.

The three sub-sections keep their jobs and gain a place to live. The purpose
rather than the mechanism is what the `Display` arm spells. The helper that
folds two faults into one answer becomes the `From` that turns somebody else's
fault into one of this crate's own cases, in the crate whose policy it is, for
the same reason it could not be shared before. And the question in the first
section -- what should this program do on a machine where this is missing --
is the one a variant is a name for.

## Where clippy disagrees

Two of these rules contradict a clippy lint, and `just ready` runs clippy with
`-D warnings`, so the disagreement is a broken build rather than an argument.

`clippy::manual_ok_err` wants `Result::err()` wherever a `match` turns a
`Result` into an `Option`. That call is exactly what EXPLICIT006 forbids: it is
the moment an error becomes an absence. The house rule outranks the inherited
one, so the `match` stays and clippy is allowed at the site, with the reason
written above it:

    // clippy wants `Result::err()` here, which is the exact call EXPLICIT006
    // forbids: it turns a failure into an absence. The rule the workspace
    // wrote for itself outranks the one it inherited.
    #[allow(clippy::manual_ok_err)]

On the smallest item that silences it. If a rule ever collects more of these
than can be read at a glance, the entry belongs in `[lints.clippy]` in the
workspace manifest instead, once -- a scatter of attributes saying the same
sentence is worse than one place saying it.

`clippy::single_match` is the other one, and it crossed that line the day
EXPLICIT019 conversions began: every guard clause rewritten as a `match` has
one arm that names an outcome and does nothing about it, which is the shape
clippy wants folded back into the `if` the house rule forbids. It is allowed
in `[workspace.lints.clippy]` in the root manifest, with the reason beside it,
rather than on every site -- and beside it the rest of the same argument,
`single_match_else` and `match_bool` and `equatable_if_let` and
`option_if_let_else`, each of which asks for an `if` back somewhere 019 has
put a `match`. Every crate inherits that table with `[lints] workspace = true`
and says nothing else about lints, so the next crate is covered before it is
written.

## Tests are exempt, except from 020

Every lint but one returns early when `cx.sess().opts.test` is set. A test that panics
is a test that fails, which is what a test is for, and an `as` in a fixture is
arithmetic nobody ships.

Nothing real is lost by it. `opts.test` is true only for the harness build of a
target, and the ordinary build of the same library is linted as production, so
a `#[cfg(test)] mod tests` inside a crate is skipped while the crate around it
is not.

020 is the exception, and it is the exception because the reason above does not
reach it. Every other rule is exempt where the harm it names is absent, and a
comment in a test is prose beside code read by the same person and going stale
at the same rate -- a test is where somebody goes to find out what a thing is
supposed to do, so it is the last place that should be explaining itself twice.
Exempting it would also cut the rule along a line nobody can see from the file:
`tests/the_tree.rs` is a test build and a `#[cfg(test)] mod` inside a library is
not, so the same comment would be legal in one and not in the other.

## The nine that came after, and the half of them that are about cost

024, 025 and 026 are the suite finishing an argument the backlog had been
keeping for it. Each is a thing a signature cannot say: 024 that two quantities
are not the same quantity, 025 that a number is standing for a case the type
does not have, 026 that a name read out of the air belongs to somebody. All
three are the same complaint the first twenty-three make -- something that
matters is not written down -- reaching one step further out each time, from
what a function returns, to what it takes, to where it got what it takes from.

**024** is the largest of the three and the largest arrival since 019. What it
asks is narrow enough to decide off a signature: no two parameters the compiler
would accept in either order, over bare representations only. What it cost was
a type for every quantity this tree actually has, and nobody had counted that
before the rule existed to count it. Geometry was the worst of it, and
`oklch_to_rgb(lightness, chroma, hue)` was the shape of the whole problem in one
line -- three `f64` in an order that is right because somebody remembered it.
`console-core-geometry` is what came out of the largest half: a place and a size
had been written out in about fifteen crates, and one `Point { across, down }`
and one `Size { wide, tall }` answer for all of them. The rest went a family at
a time rather than a function at a time, because the same pair kept arriving --
a summary and a body, an old name and a new one, a heading and a key -- and a
type per function would have been a type nobody could name twice.

**025** arrived denied, which almost nothing does. The tree was already keeping
it, and 015 is why: a rule that makes arithmetic name its policy sends every
clamp to `saturating_*` and leaves nowhere for a hand-written `MAX` to be.

**026** is the format rule said about ambient state. A `.desktop` file is read
in one crate and `desktop.conf` in another because a second reading of one
thing drifts quietly; a variable read from the environment is the same shape
with less ceremony, and `console-core-places` exists because two crates once
answered `/root` for `HOME` and neither of them said so. The rule denies the
call and lets the owning crate carry the allow, which makes the set of owners
greppable -- and makes a second crate wanting the same name come and take the
allow off the first one.

Then 027 through 032, which are about what a line costs rather than what it
says. The suite's sentence still holds and only the noun changes: an allocation
nobody wrote down, a walk nobody wrote down, a process nobody wrote down. The
harm has the shape 016's has -- it is invisible while the list is short, and
the day the list is not short there is nothing to notice, because the code
still reads exactly as it did.

**027** is the pair of words that cancel. `.collect()` followed by `.iter()`
allocates a list to do what the iterator it was made from was already doing.
Two shapes: chained, where the collection is a temporary and nothing else can
possibly be looking at it, and bound, where a `let` is used once and used to
walk. The bound half is asked narrowly on purpose -- a list walked twice, or
walked inside a loop, is saving the work rather than wasting it, and both of
those are common and right.

**028** is the accidental square: a list searched from inside a loop over
another list. The first sweep of it wrote the sentence rather than the fix at
most of its sites -- a handful of open menus, a handful of networks in range --
and the sentence was wrong at every one of them, because a handful is what a
list is on the day somebody writes the allow and not on the day it matters.
There are no allows on it now. What the sites turned into is the argument for
the rule: a `BTreeSet` beside the list where the order of the list is the
answer, an `entry` on a `BTreeMap` where the walk was really grouping and the
search falls away with it, `Path::ancestors` where the walk was really a prefix
test. Two things it does not ask about, and both are why the rest is
believable: a table whose length was typed out, and a list made inside the loop
that walks it -- built and walked once per turn, which is one pass rather than
a square.

**029** is the same multiplication where this device actually pays for it: a
started program, which is a fork and an exec, and is one short word inside a
loop. `console-compositor` will say what every window is in a single reply, and
the loop belongs on the far side of that reply. It was written asking after
files too and that half was taken back out, because a read is microseconds and
almost every file this tree opens in a loop is genuinely one file per item --
sysfs keeps one file per core, and a rule that is four-fifths allow is
paperwork rather than a rule.

**030** is what a borrow-checker complaint turns into when the fix was a guess.
`f(&held.clone())` allocates a copy so it can lend a borrow of it, and `&held`
was already that borrow. It is asked only where the copy and the thing are the
same type once references are peeled, which is where `&held` is exactly what
was meant. What it found is worth reading rather than deleting: a copy lent to
one argument of a call whose next argument moves the original is a copy the
compiler asked for, and that is the site's allow.

**031** is the shape 014 left behind. Stock clippy's `needless_range_loop`
fires where a counter is used to index, and indexing is denied here -- so what
`for at in 0..held.len()` turns into in this tree is `held.get(at)` and a
`None` arm that cannot happen, which clippy reads as a counter used for
something and leaves alone. `iter()` says the walk with the item as its
subject, and takes `enumerate` where the position is really wanted.

**032** is a whole walk spent on one bit. `Iterator::count` consumes to the end
by definition, and the definition is the part nobody reads: compared against
nothing or one it is a question about emptiness wearing a number, and `.any(…)`
and `.next().is_some()` stop at the element that answers it. A `len` is a field
rather than a walk and is not asked about here -- stock clippy's `len_zero` is
on in this workspace and has the rest of what there is to say.

None of the six is a rule stock clippy already keeps. `needless_collect` and
`redundant_clone` are nursery and off; `format_push_string` is restriction;
`needless_range_loop` is on and does not reach the shape 014 produces. What is
on is on, and this suite does not repeat it.

**033** is 001 said about an `Option`, and it is the last of the family 004
started. 004 took `unwrap` and `expect`, the two that announce themselves by
crashing. 001 took `unwrap_or`, `unwrap_or_else` and `unwrap_or_default` over a
`Result`, where what is swallowed is somebody else's error. What was left was
the same three methods over an `Option`, where there is no error to swallow and
the fault is the quietest of the four: a `None` meant something, and the line
answers it with a value that is either a sentinel -- 025's fault, reached by a
different road -- or, with `unwrap_or_default()`, not written down at all. The
reader has to know the type to know whether the machine just ran on an empty
string, a zero, a false or an empty list, and a type that changes under the line
changes the answer without touching it.

It arrived denied, and the tree broke it in three hundred and eight places, in
thirty-eight crates. What those sites turned into is the argument for the rule,
because they were not all one thing:

  - most were a `match` with both arms and the chosen value written at the call
    site, often beside a named constant -- `NEVER_OPENED`, `NOTHING_SAID`,
    `THE_FIRST_PAGE` -- which is the whole of what the rule asks for;
  - a few were a real fault wearing a default. `console-browser-extension` wrote
    a file called `file` beside a path that had no file name, and
    `console-manifest-engine` staged and kept one the same way; both now refuse,
    and `laying::staged` hands back an `Option` so the caller has to;
  - three shapes were the same answer written out in several crates, and the
    rule is what made that visible. `Option::unwrap_or` cannot be deleted from
    seven crates stepping round a list without somebody deciding what an empty
    list means, so `console-core-walking` decides it once;
    `without_a_comment` joined `console-core-ini-files`, whose business a
    comment already was; and the evdev device description that
    `console-input-gamepad::finding` owns had been copied into two other
    binaries, which now call it.

`ok`, `is_ok` and `is_err` are 001's and are not repeated here. `ok_or` and
`ok_or_else` are not asked about at all: turning an absence into a fault is
naming it, which is one of the two things this rule wants.

015 changed to let the largest of those families out honestly. `/` and `%` by a
`NonZero` are no longer bare arithmetic, because division has exactly one
failure and that type is the proof it cannot happen -- so
`checked_rem(many).unwrap_or(0)`, which used to be the only spelling 015
allowed, is now `at % many` over a `NonZeroUsize` built once, where the decision
about an empty list is made out loud instead of at every division downstream of
it. It is the one place in the suite where a policy is a type rather than a
method, and it is there because a type can carry a proof a method call cannot.

## The five that came out of somebody else's list

034 through 038 were read off a taxonomy written for C++ -- clang-tidy's
`performance-*`, `bugprone-*`, `concurrency-*`, `portability-*` families and a
list of architectural rules under them. Most of it was already answered here or
is a fault the borrow checker will not let anybody have, and what was left was
five questions this tree could be asked and had not been. Two of the five came
back with call sites, one came back with a number, and two came back green,
which is the ratchet working rather than five rules landing at once.

**035 is `concurrency011`, and it is `console-program-lifetime`'s own argument
finished.** That crate exists because a panel leaked sixty-five `pactl
subscribe` processes, and its answer is that a caller has to say whether what it
started dies with it or outlives it. Threads were never asked the same question,
so every listener in the tree was started with `let _ = std::thread::spawn(...)`
-- the handle dropped on the line that made it, nothing able to ask whether the
thread was still running, and nothing at the site saying whether outliving its
starter was the point or an oversight.

Only half the answer carries over, and the half that does not is why the word
matters more here. A child can be ended from outside, so `Alongside` does
something as well as saying something; a thread cannot be, so a caller that is
waiting on one holds the handle and joins it, and a caller that is not has
nothing to hold. `console_program_lifetime::threads::let_go` is that second case
said out loud: it takes the handle, takes only a thread that answers nothing,
and drops it. The same drop, with a word on it.

One site keeps the rule allowed and is the reason the allow test reads the way
it does. `console_core_reconnect::keep` *is* the thread -- the crate is a
subscription made again for as long as somebody wants one -- so the harm the
rule names, that nothing says how long this runs, is absent at the one site
where the function's own name says it.

**036 is `architecture008`, and it turns a scan into a question.** A program
this desktop runs and did not write has been a variant of
`console_core_external_programs::Program` for a while, crossed against
`[packages]` by a test; what held the rule was a scan of the source for a
string literal at the front of an argv, which is a net, and the README of that
crate already says what is wrong with nets. The lint asks the compiler instead:
`Command::new` with a literal, wherever it is spelled and whatever the type is
imported as.

What the sweep found was the half nobody had looked at. Every literal left in
the tree was one of *our own* programs -- `console-say`, `panel-pictures`,
`console-dictate` -- which are on the device only because `desktop.conf`'s
`[build]` names them, and were therefore exactly the same unchecked claim the
foreign ones had stopped being. `console-core-our-programs` is the other list,
crossed against `[build]` by the test beside the one that crosses `[packages]`.
It also resolves: a program of ours is looked for beside the binary that is
running before it is looked for on `PATH`, because a nested desktop runs what
was staged for it and three crates had written that walk out separately.

**034 is `api012` and 037 is `portability002`, and both arrived green.** 034
asks that a function which hands back a value and changes nothing say
`#[must_use]`, so that EXPLICIT009 can see a caller throwing the answer away;
it found one site in the tree, a proc-macro entry point whose shape belongs to
the compiler, and that is now one of the four kinds of signature it does not
ask about. The reason there was nothing else is 002: every function here goes
through `Result`, and `Result` carries the attribute itself. 037 denies
`to_ne_bytes` and `from_ne_bytes`, which are the one property of a layout that
is nowhere in the line that writes it; `console-bus` reads a byte order out of
the first byte of a D-Bus header and has never reached for the native one.

Neither of those two is a rule the tree earned by breaking it, and both are
worth the file anyway: what they cost is a paragraph, and what they buy is that
the day somebody writes the spelling that looks like it means "no conversion",
the gate is already there. That is a different thing from the warned tier,
which is for a rule the tree is walking towards.

**038 is the suite arguing with itself, and it was the last rule with a
distance.**
001 makes sure a failure met is a failure said. 002 puts every function through
a `Result` so a caller's shape does not change when the thing it calls learns
how to fail. 005 makes sure the answer is met. Then what arrives is a `String`,
and a caller that met it can do exactly one thing with it, which is show it to
somebody: "the socket is not there" and "this desktop may not read it" reach
the same arm, nothing can retry one and give up on the other, and a fault
carried up through two crates is a sentence the second one is guessing the
wording of.

It stood warned while that was walked back, which is what that tier is for, and
the distance was most of the crates in the tree. `Wire`, `Torn` and `Why` were
what the answer looked like where it was already written -- an enum naming the
ways one call fails, `Display` on the enum so the words are written once, `From`
where a fault crosses a crate boundary and becomes one of the receiving crate's
own cases. Every other crate has one now: `Unwritten` for the four steps of a
write, `Unpressed` for everything a press walks through, `Awry` for what a stage
can see, `Unapplied` for the ways an apply does not happen. It went a crate at a
time, bottom-up, the way 002 went, and the sentence each fault used to print is
the `Display` arm that replaced it -- so the journal reads as it did, and the
caller can now ask which fault it is looking at.

## 048, which asks about the type rather than the code in it

Every rule before it reads something somebody wrote in a function. 048 reads
the shape of a type and asks how many of the states it can be written into are
states it really has. The struct that prompted it is the one everybody has
written: a `bool` saying whether the thing worked, an `Option` holding what it
produced and an `Option` holding what went wrong, which is three fields, eight
spellings and four meanings -- and the other four are reachable by a caller who
sets one field and forgets the next. That struct is an enum written out flat,
and the states are real; they were simply never named, so nothing counts them
and nothing fails when a fifth arrives. `Option<Option<T>>` and
`Option<Result<T, E>>` are the same complaint in a smaller shape, three states
spelled as two questions, with the reader left working out which absence meant
what.

It is EXPLICIT016's argument moved from the match to the type. 016 denies the
wildcard arm so a decision names every case; the case the wildcard was hiding is
usually a combination the type should never have let exist. Both halves are read
out of a Haskell habit that has a sentence for itself: make illegal states
unrepresentable.

The false positive decides how narrow it is. What is asked about is a `bool`
standing beside an absence in the same struct -- a flag and the thing the flag
is about -- or beside another flag, which is the same shape with the payload
left out. Two `Option` fields are frequently two optional things and are left
alone. The message says how many fields it read that way and how many spellings
they make, so what `just explicit` prints is a distance to argue with rather
than a verdict -- and where the fields really are independent, the site carries
the allow and EXPLICIT018 asks the reason to say so.

The fix at a site is usually more than a rename, and the help says so. Where the
flag was set by a check, the variant holds what the check found -- `Yes(Socket)`
rather than a yes -- so whatever does the work takes the thing and cannot be
called without it, and the gap between the check and the doing closes with it.
That is what Elixir gets from a second function head. Rust has one definition
per name and no dispatch on a return type, so the same guarantee has to live in
the type, which is where this rule keeps pointing.

`Result<Option<T>, E>` looks like the third nesting and was in the rule until
the count came back. It is in hundreds of signatures here, and every one of
them is right: EXPLICIT002 puts every function through a `Result`, so that shape
is the ordinary way to say *the call happened and the thing is not there* --
the outer answer is about the call, the inner one about the value, and they are
two questions rather than one asked twice. A rule that condemns the house style
is a rule nobody keeps, and finding that out cost one run of `just explicit`,
which is what the warned tier is for.

What the run found when the distance was walked was a split, and the split is
the rule. Some of the structs were the flat enum it is named for and are enums
now: a player that is playing, paused or stopped rather than two flags that can
both be true; a button that is loose, held, shared or already gone rather than
an instant and two more flags; a key drawn pressed, under or plain rather than
a pair whose fourth spelling the drawing quietly ignored. Every one of those
lost a combination nobody meant, and two of them were answered by an enum the
same file already had. The rest were fields that really are independent -- a
wayland surface's separate promises, the flags somebody typed on a command
line, what hyprctl says about one window -- and each carries the allow with a
sentence saying which different question each field answers. Neither half was
available to anybody who could not see the list, which is what the tier was
for.

## What it is, and what it is not yet

`just explicit-gate` is a gate. It denies the rules that nothing in the tree
breaks and warns the rest; which tier a rule is in is the level in its own
crate and nowhere else. The warned tier is empty today, which is a thing that
is true between sweeps rather than a thing that is finished: the next rule
somebody writes ahead of the code will stand there until its last call site is
answered. A rule moves up when the last call site that broke it
is fixed, and it never moves back. That is the whole ratchet.

`just explicit` is the other half: every rule over every crate, counted rather
than enforced, so the distance is visible whether or not it is being enforced
yet. There is no number written down here on purpose -- a count in a README is
a count that is wrong by the next commit. Run it.

Turning every rule on at once would mean a deploy nobody can make until every
call site is answered, and a gate somebody starts going around is worse than no
gate. The grind is the point when it happens: each denial is a call site that
deserves an honest look.

## It needs rustup on PATH

`cargo dylint` asks the toolchain what it is before it builds anything, so
`rustup` has to be findable. Cargo will find the `cargo-dylint` subcommand in
`CARGO_HOME/bin` whether or not that directory is on PATH, which is the trap:
without `rustup` the run gets far enough to look like it started and then dies,
and a summary that greps for warnings then reports a clean workspace when
nothing was ever linted. Both recipes put `CARGO_HOME/bin` on PATH and check
the run before they count it.

Nothing here is on the device's path. The device compiles what
`desktop.conf`'s `[build]` names, and this is not in it.
