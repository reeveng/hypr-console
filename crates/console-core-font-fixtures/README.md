# console-core-font-fixtures

The fonts and tables the font reader, metrics and hinting crates are tested
against, kept in one place because all three read the same ones. It began as
fontations' `font-test-data` 0.7.0.

Most of it is bytes: whole fonts under `test_data/`, compiled from the `ttx`
and `fea` sources beside them, and tables transcribed from the specification's
own examples as static arrays, one module to a table. What is not bytes is
`BeBuffer` and the `be_buffer!` macro, which write a table out of scalars in
the order the format lays them down, with a name on any field a test needs to
find again and patch; patching a field by a name that was never given, or past
the end of what was written, is a `BufferError` and leaves the bytes as they
were. `colrv1_json::expected` is the drawing commands each `COLR` test font is
expected to produce, by name.

`test_data/README.md` says how the fonts were made and how to make them again.
The upstream code is MIT or Apache-2.0 at your choice; both licences are in this
directory.
