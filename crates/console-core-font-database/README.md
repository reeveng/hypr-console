# Font database

The font faces a machine has, held in memory, and which of them serves a CSS
family, style, weight and stretch. A face is loaded from bytes handed in, from a
file, or from every font file under a directory, and each face it finds is given
an `ID` the rest of the desktop asks about it by. `query` narrows the faces the
way CSS Fonts Level 3 does: the first family in the list that any face has, then
the nearest stretch, then the style, then the nearest weight.

`load_system_fonts` reads the machine's fontconfig files through
`console-core-font-configuration` for the directories and the generic families,
and falls back to the usual font directories under `/usr` and the home when
there is no configuration to read. `FONTCONFIG_FILE` is the one variable read,
because the file it names decides where the fonts are; the home and the base
directories come from `console-core-places`.

What a face says about itself comes from its own small TrueType reader in
`ttf_parser`: where the tables are, the `name` table for families and the
PostScript name, the `OS/2` table for the weight, width and slant, and the
`post` table for a fixed pitch and an italic angle. Ordinary file-backed faces
read their bytes into owned memory. The `memmap` feature enables
`make_shared_face_data`, which keeps one mapping for every face of a file.
`std` can be disabled for `no_std` with
`alloc`, which keeps loading from bytes and querying.

There are no tests beyond the Macintosh Roman name in `faces`: the crate was
compared against the upstream reader over every font under `/usr/share/fonts`
and in `console-core-font-fixtures`, face by face and query by query, when it
came under the workspace's rules.

From fontdb by Yevhenii Reizner. The upstream MIT notice is retained in
`LICENSE`; this workspace offers its copy under AGPL-3.0-or-later.
