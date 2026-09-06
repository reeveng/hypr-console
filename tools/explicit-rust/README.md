# Explicit-Rust

A dylint suite for the rule this workspace is written to: no behaviour that
matters should be implicit. It is the spirit of Elixir's `{:ok, value}` put to
Rust -- a call either says what it returns or says how it failed, and nothing
important happens because of a `bool`, an `as`, or a panic nobody declared.

It is a workspace of its own, on a nightly of its own, excluded from the one
above by `exclude = ["tools/*"]`. Lint crates link against `rustc_private`,
which stable cannot do; `rust-toolchain.toml` pins the nightly and the
`rustc-dev` component it needs, and `cargo dylint` builds them on demand.

    just explicit        the rules, by kind, by count
    just explicit-gate   the rules the workspace already keeps, enforced

## The rules

    EXPLICIT001  fallible fns return Result<T, E>
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

All of them are written. Each is one crate with a `ui/` case beside it.

The level in the lint's own source says which tier a rule is in, and every
rule the workspace keeps is `Deny`: the gate fails on all twenty. Nothing
stands in the `Warn` tier, which is where a rule written ahead of the code
waits, printing its remaining distance on every run so it is never out of
sight. A rule moves from `Warn` to `Deny` in its own crate when the last call
site that broke it is fixed, and by the ratchet's one law it never moves back.
018 came out the quiet way: it was written after the
policy it names was already kept everywhere, so its last call site was answered
before the rule existed to count it. 017 came out the long way, which is the way the ratchet
expects -- every buried `?` in the tree lifted into a `let` of its own, one
crate at a time. 020 came out the way that should not be necessary: what it
forbids had been swept out of the tree once already, by hand, and was back in
most of the crates by the time anybody looked. 019 came out the longest
way of all, because it was the rule the whole tree broke: every guard clause, every `if let`, every `else if`
chain rewritten as a `match` that names what the other path was.

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
`console-never` is that type -- an enum with no variants, so the `Err` a caller
does not write is a case the compiler agrees cannot arrive.

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
`console-number-conversion` reaches every screen this desktop draws.

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
name, at the right time -- and a negated literal with them, because `-1` is
how a negative number is written rather than a subtraction anybody performs.
`console-number-conversion`'s hand-rolled float decoder is what this rule looks like
adopted early.

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
reading, and a lint cannot do a reading; `todos.md` carries that as work for a
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

`console-number-conversion` came off the same way, and it is the better story because the
reason was true. Float to whole number and count to float genuinely have no
conversion in the standard library -- no `From` and no `TryFrom` in either
direction, and the compiler says so if you ask it. What the allow's reason did
not survive was the difference between "the standard library has no conversion"
and "there is no way": `f64::to_bits` is safe and total, and once a float is in
pieces the rest is integer arithmetic. So the crate now decodes the exponent
and shifts the significand, and saturating is three cases in a type rather than
something the arithmetic is trusted to arrive at.

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
a failure has to say what happened instead. The error is a `String`, so what it
says is a sentence somebody reads in a journal rather than a type a compiler
checks. Three things about that sentence are not the call site's own choice,
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
            eprintln!("stick-scroll: HOME: {fault}; no button anybody moved will be read");
            None
        }
    };

That is the form to reach for where the default is deliberate, and it is how
`console-controller` came to zero without making its callers fallible. Which of
the two a site wants is a question about the site -- what should this program
do on a machine where this is missing? -- and the shape of the call that is
there now does not answer it. Every site gets read for that before it is
converted, because the conversion that is wrong is the one that lints.

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

It does not belong in a shared crate, and `console-controller` is the reason.
That crate reads the same `VarError` and pulls the two cases apart, because
there they are not the same thing at all -- a device nobody pointed at is
ordinary and is most of the time, while a name set to something that is not
text is somebody trying to point at a device and missing, and it used to arrive
as the same silence. Both crates are right about their own machine. A shared
helper would have to pick one of them, and picking either makes the other one
wrong.

So a helper of this kind stays in the crate whose policy it carries. What is
shared is this page: the grammar of the sentence, and the question in the first
section that every site has to answer for itself.

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

## What it is, and what it is not yet

`just explicit-gate` is a gate. It denies the rules that nothing in the tree
breaks and warns the rest; which tier a rule is in is the level in its own
crate and nowhere else. A rule moves up when the last call site that broke it
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
