## Dependencies

Get explicit approval before introducing a new third-party dependency, directly or transitively.

Send library diagnostics through `console-core-logging`.

## Rust changes

Read the [Explicit Rust workflow](tools/explicit-rust/README.md#writing-explicit-rust)
before editing Rust. Check the owning type and affected callers before changing
a signature. For lifecycle changes or work that finishes later, use the
[state machine and effects guidance](tools/explicit-rust/README.md#state-machines-and-effects)
when choosing the implementation.
