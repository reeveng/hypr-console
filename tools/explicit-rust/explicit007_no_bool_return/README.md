# Reviewed enum returns

EXPLICIT007 can apply a reviewed return-type change to a private function and
all its direct callers in the same module. Define a module-local enum with two
unit variants and no destructor. Map the full function path to the enum, the
variant for `true`, and the variant for `false`, in that order.

```toml
[explicit007_enum_outcomes]
"example_crate::decoder::classify_byte" = ["ByteClass", "Control", "Data"]
```

Place the mapping in `dylint.toml` or supply it through `DYLINT_TOML`. Run
`cargo dylint --fix` with the applicable features and targets. The suggestion
maps the original tail expression to the reviewed variants. At each direct
call it matches the enum to give the surrounding predicate the same value.

The fixer checks that every authored reference resolves to a direct call. A
reference in inactive source, a function item passed as a value, or a scope
that can generate unseen callers requires manual review. It also keeps
reporting diagnostics without a fix for methods, exported or foreign
functions, generics, `const` and `async` functions, and explicit returns.

The [Type 1 configuration](font-type1-outcomes.toml) records reviewed
byte-classification migrations from `console-core-font-reading`.
