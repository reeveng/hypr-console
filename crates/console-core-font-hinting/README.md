# Font hinting

Executes embedded TrueType instructions on scaled glyph points supplied by a
caller. This crate owns the bytecode interpreter, graphics state, control values,
function definitions, execution limits and target settings.

`FontProgram` borrows a font's programs and resource limits. Reuse it across sizes
and variation locations. `HintInstance::reconfigure` runs the font and control
value programs for a scale, pixel size, target and normalized coordinates.
`HintInstance::hint` applies the retained state to a `HintOutline`, borrowing
caller-owned geometry and scratch buffers. Scratch capacities come from
`FontProgram`; callers can reuse their buffers for each glyph.
[tests/the_programs.rs](tests/the_programs.rs) runs a program directly against
caller-owned points without constructing an outline scaler.

`console-core-font-metrics` composes this interpreter with glyph loading,
variation deltas, scaling and drawing. Its existing `outline::Target`,
`outline::SmoothMode` and hinting error paths remain re-exports of these types.
Automatic hinting and CFF stem hinting remain with their respective outline
pipelines in the metrics crate.

The interpreter supports `no_std` with allocation. Disable default features and
select `libm` when using it with the font reader without the standard library.
`scaler_test` exposes a control-value-program simulation hook for scaler tests.

Extracted from the Fontations code carried in this workspace. The original
MIT and Apache-2.0 licenses are retained; this workspace offers its copy under
AGPL-3.0-or-later.
