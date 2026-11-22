# PDF functions

Reads and evaluates PDF function objects: sampled (type 0), exponential (type 2),
stitching (type 3), and PostScript calculator (type 4). Extracted from Hayro's PDF
interpreter, with its function tests kept beside the implementations.

`Function::new` takes an object from `console-core-portable-document-syntax`;
`Function::eval` takes and returns `Values`. No rendering device, font loader,
color space, or interpreter context is needed. `logging` enables diagnostics
and is forwarded by the interpreter's feature of the same name. The interpreter
continues to export the same `Function` type at its original public path.

The upstream MIT and Apache notices are retained in `LICENSE-MIT` and
`LICENSE-APACHE`; this workspace offers its copy under AGPL-3.0-or-later.
