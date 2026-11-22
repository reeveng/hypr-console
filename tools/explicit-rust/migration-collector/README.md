# Compiler-guided migrations

These Rust tools prepare source changes from compiler types and syntax. They
operate on one immutable source snapshot, reject stale fingerprints and crossing
edits, and save original files under `$TMPDIR/builds` before applying changes.
The default is a dry run. A prepared edit count is not a count of fixed lint
findings: compilation, tests, Clippy and the actual rules still decide that.

Run from the repository root with the pinned compiler:

```sh
cargo +nightly-2026-05-28 build \
  --manifest-path tools/explicit-rust/migration-collector/Cargo.toml \
  --target-dir "$TMPDIR/builds/explicit-migration-collector" --release
```

Run the source migration directly from the workspace root.

```sh
mkdir -p target
just explicit-apis --package console-core-font-reading --facts facts-*.json \
  --syntax syntax.json --passes results,branches,comments,casts,arithmetic \
  --arithmetic wrapping --diff target/explicit.patch
```

The facts and syntax options accept several files. Use `--allow-legacy-facts`
with old captures made from the current sources that lack fingerprints. Any
fingerprints present are still checked.

`apis` uses the compiler lexer. Invoke it through `cargo run` so Cargo supplies
the pinned compiler's shared library search path.

## Capture one dependency family

The `text_adapt` library records function identities, resolved calls, expression
types, source fingerprints and const/foreign/generated ownership. Touch the
selected Rust sources so Cargo actually checks them. Configure all source
packages that can contain a changed function or its callers:

```sh
LEGION_ADAPT_PACKAGES=console-core-example,console-example-consumer \
LEGION_ADAPT_OUTPUT="$TMPDIR/builds/example-migration" \
DYLINT_RUSTFLAGS='--force-warn text_adapt' \
cargo dylint --no-metadata --no-build --no-deps \
  --lib-path /path/to/libtext_adapt@nightly-2026-05-28-x86_64-unknown-linux-gnu.so \
  -- -p console-core-example -p console-example-consumer --all-features --lib
```

The collector library must have Dylint's `@toolchain` filename; copy the built
`libtext_adapt.so` to that filename. Create the output directory first. The
explicit warning flag also enables collection in inherited crates that allow
warnings. Confirm the output names every intended package.

Capture syntax from the same sources with `syntax`, which writes a JSON array
to standard output. It normalizes source offsets exactly as the compiler does.
Pass every family's typed and syntax capture to a single migration plan. Applying
one package and then using an old capture for another package can edit stale
caller spans. Recapture after each applied stage.

## Passes

- `apis`: `--package NAME`, `--facts FILE` and `--syntax FILE` can each repeat.
  `--passes results,branches,comments,casts,arithmetic` composes edits in one plan.
  Arithmetic requires an explicit `--arithmetic wrapping` after reviewing the
  algorithm's overflow policy. Omit passes that need further domain review.
- `numeric`: accepts `--package NAME` repeatedly and `--facts FILE...`.
  The default is casts only; `--arithmetic wrapping` enables reviewed wrapping
  arithmetic. `--diff FILE` writes the prepared patch.
- `array_projections`: uses `--package NAME` and `--facts FILE...` to
  replace literal accesses on compiler-confirmed fixed arrays with patterns.
  Use `--prefix-only` to keep each pattern anchored to the original index
  when the array length may vary across build configurations.
- `just explicit-repair`: repeated `--package NAME` and one `--pass`. Use
  `--diagnostics target/explicit-batch/run-…/rules.jsonl` after a batch audit.
  The passes that read the
  rules' findings take `--diagnostics FILE`, the JSON messages of a run of the
  rules over the same sources, and refuse a finding whose quoted line no longer
  matches the file, so recapture between passes:
  - `spacing` inserts the blank lines EXPLICIT013 suggests, at the positions it
    suggests and never inside a literal. Before writing, it checks that the
    complete Rust token stream is unchanged. Local packages under both
    `crates/` and `forks/` are supported.
  - `questions` lifts `?` expressions reported by EXPLICIT017 into their own
    bindings. Add `--questions-only` to leave earlier `Result<T, Never>`
    migration blocks outside this pass and measure the lint reduction separately.
  - `index_rows` replaces repeated EXPLICIT014 accesses in a literal
    `for row in 0..HEIGHT` loop with iteration over fixed arrays. It handles
    `matrix[row][column]` and flat `array[row * WIDTH + column]` written with
    `usize::wrapping_mul` and `wrapping_add`. The array declaration and every
    column must prove that the loop fits. Run with `--diagnostics` from the
    same sources and add `--apply` to write the edits.
  - `must_use` puts `#[must_use]` directly above the signature of each function
    EXPLICIT034 reports, under its other attributes and its documentation.
  - `division` names the policy of a division or remainder EXPLICIT015 reports
    when the divisor is a positive integer literal: `x.wrapping_div(2)`, and
    `x = x.wrapping_rem(3)` for a compound assignment to a name or a field.
    Such a divisor can neither be zero nor minus one, so the wrapping method
    answers exactly what the operator did, signed or unsigned, in either
    profile. Any other divisor, and addition, subtraction and multiplication,
    need the operand's type and an overflow policy, and are left.
  - `compiler` applies the two repairs a lift can leave the compiler asking
    for: the `mut` E0596 suggests for a value that is now a named local, and
    `let Ok(_)` where a discarded value turned out not to be unit. It reads
    `cargo check --message-format=json` rather than a run of the rules.

  Two passes read only the sources:
  - `tidy` takes the one-line blocks the first `Result<T, Never>` rewrite left
    inside expressions, `({ let Ok(explicit_value) = f(); explicit_value })`,
    and lifts each into a statement before the expression that used it, or
    makes it the pattern when it was a whole initializer (`let Ok(x) = f();`).
    A block written over several lines is a body somebody spelled out and is
    left alone, so the pass reaches a fixed point.
  - `bodies` writes a function body of the form `Ok({ … })` or
    `Ok(match { … } { true => …, false => … })` as the statements it holds at
    the function's own depth and an answer at the end: `Ok(tail)`, `Ok(())`, or
    `Ok(match tail { … })`. A questioned tail becomes a `let` first, so the `?`
    stays alone.

  `questions` and `tidy` share `lift`, which follows the language's evaluation
  order through each statement. A value moves only when everything evaluated
  before it in the statement is a path, a literal, a field, a reference or a
  structure of those, when it names nothing read before it, and when it is
  evaluated on every path: never out of the right of `&&` or `||`, a guard, an
  `else if` or loop condition, a macro other than the formatting and assertion
  macros whose arguments always run, or an expression carrying attributes. A
  closure or arm body written without braces is given them. A `cfg` on the
  statement goes onto each `let`, a statement that decides keeps its blank line
  below the new `let`, and a block's tail gets one.

  A lifted value is named by `names`: the method or function without `get_`,
  `to_`, `as_`, `read_` and the like, the field, the type a constructor builds,
  or what a conversion is handed, then the same prefixed with what owns it
  (`other_width`, `own_root`). A name already used anywhere in the function, a
  primitive's name, an arithmetic method and a single letter are refused. A
  value with no such name is left where it is and reported, never numbered.
  `REPAIR_EXPLAIN=1` prints each site left and why.
- `comments`: repeated `--package NAME`. Reads the sources with the compiler's
  lexer and deletes the comments EXPLICIT020 denies, grouped as the rule groups
  them, and keeps the `//!` head, every `// SAFETY:`
  reason and the documentation of an `unsafe fn`, trait or impl. A licence or
  copyright notice is moved into the `//!` head, or becomes it. A blank line
  left by a deletion is kept once where it separated two statements and dropped
  after an opening brace or before a closing one. Where `src/lib.rs` asked for
  `missing_docs`, the lint is withdrawn with the documentation it asked for.
  Doctests go with the documentation that held them, so a crate that had them
  loses them.
- `whitespace`: repeated `--package NAME` trims comment-removal residue outside
  literal spans in `src`, `generated`, `tests` and `benches`.
- `compiler_repair` and `test_assertions`: source and diagnostic repair passes
  for typed Never extractions, macro templates and test failures.

Every pass edits only the span it names and never reformats a file; each
output is parsed again before anything is written.

All editing binaries require `--apply` to write source. Use `--help` for each
binary's exact arguments. `--include-generated` explicitly extends API/numeric
ownership to checked-in `generated/` sources.

Boolean functions require a reviewed `--roles FILE` mapping:

```json
{
  "answers": {
    "is_empty": {"name": "Emptiness", "yes": "Empty", "no": "NonEmpty"},
    "is_non_empty": {"name": "Emptiness", "yes": "NonEmpty", "no": "Empty"}
  },
  "aliases": {
    "console-example-consumer": {"console-core-example": "example_alias"}
  }
}
```

Use full compiler function identities for ambiguous method names. Opposite
predicates can share an enum with reversed outcomes. Unmapped booleans, foreign
signatures, const conversions, unknown patterns and conversions needing a domain
policy are reported for review. The converters retain module heads and complete
safety comment blocks, expression attributes, mutable bindings, macro-valued
returns and primitive compound-assignment evaluation order.

## Verification

```sh
cargo +nightly-2026-05-28 test \
  --manifest-path tools/explicit-rust/migration-collector/Cargo.toml \
  --target-dir "$TMPDIR/builds/explicit-migration-collector" --bins
```

Regression tests include a compiled program proving macro return side effects
occur once and inactive conditional branches remain inactive. Other tests cover
nested edits, Unicode offsets, stale snapshots, const/foreign exclusions, mutable
bindings, operand order and literal-preserving whitespace cleanup.

An order that works for one crate: `tidy` and `compiler` until `tidy` lifts
nothing, then `bodies`, then a run of the rules for `questions`, another for
`spacing`, and a last one to count what is left.

After application, compile the changed crates and their consumers, run their
functional tests and strict Clippy, then touch their sources and run `cargo x
rules`. Check that the output names the changed crates and loads the required
libraries. An empty Dylint discovery does not verify any migration.
