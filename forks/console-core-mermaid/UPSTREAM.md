Forked from [Merman main at af773bacc9e397f5b77a029f71d32c1b822aab59](https://github.com/Latias94/merman/tree/af773bacc9e397f5b77a029f71d32c1b822aab59),
version 0.10.0-alpha.2. `UPSTREAM.toml` records the baseline for future update comparisons.
The upstream licenses and notices are retained.

This copy uses Legion's maintained graphics libraries so their rendering
behavior can be changed together.

Each package lives directly under `forks/` in a directory matching its local
package name. `forks/Cargo.toml` owns their workspace; the Node candidate keeps
its separate workspace. Shared distribution files, contracts, licenses and
notices live beside the packages.

`UPSTREAM.toml` maps each local directory to its original upstream source path
and package name for update comparisons. Rust library names and import aliases
retain their upstream spelling.
The renderer's two ELK-only helpers are gated on `layout-elk`, so builds that
select another layout engine compile without unused-code warnings.
