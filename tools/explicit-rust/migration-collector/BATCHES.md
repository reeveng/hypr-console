# Bulk fixes through Dylint

`batch` captures the lints' `MachineApplicable` suggestions and applies
them directly. Run it in each checkout to plan fixes against that branch's
current sources and features.

Run these commands from the repository root.

```sh
just explicit-batch audit
just explicit-rust fix-all
just explicit-batch fix --package PACKAGE --rules explicit033_no_unnamed_absence
just explicit-batch fix --package PACKAGE --rules explicit033_no_unnamed_absence --apply --max-batches 1
just explicit-batch fix --min-fixes 100 --batch-size 500 --max-batches 1 --apply
```

The Option fallback and conversion fixers can run together in one batch.

```sh
just explicit-batch fix --rules explicit033_no_unnamed_absence,explicit045_no_unnamed_conversion \
  --min-fixes 100 --apply
```

`fix` previews which packages and targets have Dylint suggestions.
`fix --apply` applies them. The default budget is 5,000 suggestion groups
per batch. The packages with the most suggestions are tried first. The first
attempt keeps each crate whole, even if it exceeds the budget. Set `--min-fixes` to skip
a batch with too few suggestions, and use `--batch-size` and
`--max-batches` to control how much work a run attempts.
`--package NAME` can repeat. `--exclude NAME` removes an audited package from
the lint and compiler-check scopes. `--rules LIB,...` restricts the fixing stage,
and every rule still contributes to the before and after counts by default.
Add `--selected-rules-only` to use that rule list for the inventory too. This
speeds up development of a fixer before measuring its effect across the full
suite.

```sh
just explicit-batch fix --package console-core-font-reading \
  --rules explicit045_no_unnamed_conversion --selected-rules-only
```

Each batch saves the current source files, including existing uncommitted edits,
under `$TMPDIR/builds/explicit-batch`. Identical suggestions from different
rules and targets share one edit. Conflicting edits can be picked up from the
next capture. The runner writes the batch before it recompiles, so a sweep can
fix a crate and its dependents together. A workspace compiler check and another
lint capture follow each batch. The runner restores edits if compilation or
capture fails or the total fails to decrease. It retries with fewer crates,
then with smaller sets of suggestions within a crate. Multi-part suggestions
stay together. The report lists rejected packages in `automatic.json`.
The runner resolves the workspace's Dylint configuration once per capture.
An explicit `DYLINT_TOML` value takes precedence over `dylint.toml`.
Tests and strict Clippy follow the final batch. `--verify none` omits those
final checks. `just explicit-rust fix-all` uses that setting and still checks
every applied batch with the compiler.

Each run writes its files under a unique directory in `target/explicit-batch`.
`coverage.json` lists the total, automatic and review counts for each audited
rule. `remaining.json` locates each finding, `automatic.json` records the
package and target plan, and `progress.json` records before and after counts
for applied batches. The preview counts candidate groups before conflicts are
resolved. A single loop rewrite can clear several findings. Inventories count
each distinct diagnostic at each primary span
once across build targets. A discriminant rule can report multiple pairs on
one enum, and each pair counts separately.

The terminal shows a preview with source locations, lint help, and whether a
compiler suggestion is available. Open `findings.txt` in the run directory for
the complete listing. With `--baseline`, both the preview and `findings.txt`
show new findings, and `remaining.json` contains the full inventory.

The `review_patterns` array in `remaining.json` groups sites that share a
rule and its diagnostic guidance, with occurrence counts by source directory
and an example location. Start with recurring patterns across directories
when adding the next compiler-backed suggestion.
## Merge audit

Use `just explicit-batch audit` or `just explicit-gate` for the merge check.
The batch recipe uses nightly `2026-05-28` and Cargo runs offline with the
root and tool lockfiles. Use `cargo-dylint` and `dylint-link` 6.0.4 in the
merge environment, matching the lint workspace lockfile. Run with the
repository's Dylint settings and build prerequisites.

- Workspace members are audited with all host targets in both their default
  and all-features configurations.
- The UEFI kernel libraries and binaries are audited in both feature
  configurations when the kernel is selected.
- Owned path packages under `forks/` that resolve in the root lockfile are
  audited as libraries with their resolved dependency features. They can also
  be selected by name for a focused audit or fix.
- Standalone workspaces such as the Servo and WebKit renderers have their
  own manifests, lockfiles and system build requirements. Audit shipped
  configurations outside the root matrix on a runner provisioned for them.

During migration, keep a successful inventory from a reviewed reference
revision and run `just explicit-batch audit --baseline PATH/TO/remaining.json`.
The gate matches findings by rule, file, diagnostic and source text. Line
movement and indentation changes preserve the match. Extra occurrences and
changed source sites appear in `regressions.json`. An audit with the baseline
option exits 2 when that file contains findings. It requires a report
generated by this runner with the same rules and lint settings.

Require a zero audit without a baseline for each cleaned family and each
internalization. Pin the upstream revision and retain its license notices.
Keep the upstream import and local adaptation in separate commits. Run the
same upstream tests or parity fixtures on both sides, then run the scoped
audit and the merge check. Document required upstream or FFI exceptions at
the affected site.

Use the same inventory to find effect migrations in a crate.

```sh
just explicit-batch audit --package console-input-controller \
  --rules explicit105_no_blocking_in_handler --selected-rules-only
```

EXPLICIT105 follows local calls from `Machine::initialize` and
`Machine::handle`. A finding points to the call in the machine and the first
blocking operation it reaches. The inventory groups these as review findings
because the migration must decide which state owns an outstanding effect and
how to handle its reply. The [state machines and effects guide](../README.md#state-machines-and-effects)
shows how to make that decision and test it.

`just explicit-av1-transform-bounds` applies EXPLICIT014 to the AV1 transform
and reconstruction functions that propagate `Rav1dError`. The config lists
their fully qualified function names in `explicit014_fix_functions` and maps
an absent element to `Rav1dError::EINVAL`. Diagnostics outside those functions
remain visible during the run.

Use `--capture PATH/TO/capture.json` to reuse a successful capture. The root,
package scope, sources, lint implementations, and lint settings must still
match. `--no-build` is only
for runs whose lint libraries are already built and current; normally leave it
out so Dylint rebuilds changed lints. With `DYLINT_LIBRARY_PATH` set, a
`--no-build` run loads the supplied libraries directly.

For `audit` without a baseline and for `fix`, exit status 0 means the inventory is empty and 2 means
findings remain. `fix-all` exits 0 once its automatic queue is empty. It still
reports findings that need review. Exit 1 means a build, verification, tool or
automatic fix failed. The applied changes and backups remain
available for inspection. Use `just explicit-batch --restore BACKUP_DIRECTORY`
to undo a batch. Restoration checks for source edits made after that batch.

Regression tests, including planning 5,000 fixes and preserving existing edits:

```sh
cargo +nightly-2026-05-28 test --offline --locked \
  --manifest-path tools/explicit-rust/migration-collector/Cargo.toml \
  --target-dir "${TMPDIR:-/var/tmp}/builds/explicit-migration-collector" \
  --bin batch
```
