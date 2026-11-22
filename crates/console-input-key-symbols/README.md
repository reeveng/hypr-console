# Key symbols

Defines XKB key-symbol values independently of keymap compilation. `Keysym`
owns symbol names and aliases, Unicode and legacy character mappings, casing,
keypad classification, formatting, and the X11 constants. The `syms` module
exposes the constants as module items as well as associated `Keysym` constants.

This crate has no layout parser, include-path discovery, compositor, or device.
It uses `console-core-perfect-hash` for immutable table lookup and
`console-core-number-conversion` for checked numeric spellings. Membership is
validated after a candidate lookup, as in the original reader.

`console-input-keymaps::Keysym` remains a re-export of this same type. Modifier
and group lookup still belong to the keymap compiler. Drawing surfaces obtain
the symbol type directly from this crate.

Extracted from the kbvm code carried in this workspace, originally by mahkoh
under MIT or Apache-2.0. This workspace offers its copy under AGPL-3.0-or-later.
