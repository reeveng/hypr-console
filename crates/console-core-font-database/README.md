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

The database uses `console-core-font-reading` for font collections, table
directories, `name` records, and `OS/2` style fields. It selects family and
PostScript names and uses the `post` table for pitch and slant. Ordinary
file-backed faces read their bytes into owned memory. The `memmap` feature enables
`make_shared_face_data`, which shares one owned snapshot among all faces of a file.
`std` can be disabled for `no_std` with
`alloc`, which keeps loading from bytes and querying.

The tests in `faces` cover Macintosh Roman names and format-one name tables.
When the database came under the workspace's rules, its faces and queries
were compared with the upstream reader using fonts from `/usr/share/fonts`
and `console-core-font-fixtures`.

From fontdb by Yevhenii Reizner. The upstream MIT notice is retained in
`LICENSE`; this workspace offers its copy under AGPL-3.0-or-later.
