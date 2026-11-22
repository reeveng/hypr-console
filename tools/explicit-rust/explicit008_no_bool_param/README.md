# Reviewed enum migrations

EXPLICIT008 can fix a reviewed family of private Rust functions in one pass. Add
a `Copy` enum with two unit variants in the family’s module, then provide the
variant mapping and every signature and source file in the family.

```toml
[[explicit008_enum_families]]
module = "example_crate::decoder"
enum = "crate::decoder::Scaling"
when_true = "Apply"
when_false = "Skip"
sources = ["example_crate/src/decoder.rs"]
members = { scale = "apply_scale", forward = "apply_scale" }
```

The fixer updates declarations, direct calls with literals, local variables
or const parameters, and exhaustive matches on the parameter or its fully qualified
`bool::then_some(parameter, ())` conversion. It passes the enum through between
members. Other parameter uses, external ABIs, function-pointer references,
conditional code in a member body and unresolved written references leave the
family for manual review.

When a parameter feeds Rust's `From<bool>` conversion to a primitive integer,
the fixer matches the named variants to the same typed zero and one.

Set `inherent_type` to the crate-qualified receiver type to review an
inherent method family. The fixer follows method syntax and type-qualified
calls, identifies unrelated methods by their resolved declarations, and
checks that each rewritten caller can use the enum. The methods must be
internal to the crate.

Set `forwarded_macro_arguments` to map a reviewed `macro_rules!` definition
to its expression metavariable. This allows the fixer to update a literal in
the invocation when the macro has one rule and forwards that metavariable
once, directly in the selected argument position.

Supply the reviewed configuration through `DYLINT_TOML`. List source files
containing every declaration and direct caller, including files reached by
`include!`. Review configurations that the compiler cannot see together, and
run the fix with the applicable features and targets. The checker requires the
listed files to be loaded and reconciles written function names with the calls
and declarations rustc resolved. A written call in an inactive branch prevents
a suggestion. Cargo applies suggestions one file at a time, so the lint emits
one coordinated suggestion per file after it verifies the complete family.

The [AV1 configuration](reviewed/av1-rectangular-scale.toml) opts into authored
spans preserved by Archmage's `rite` and `arcane` attributes on its named
target. It also lists two declaration macros that contain direct callers.
Set a target whenever you opt into reviewed macro spans, then verify the native
dispatch boundaries with the full target and test configuration.
