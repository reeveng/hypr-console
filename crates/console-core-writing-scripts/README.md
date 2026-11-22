# Writing scripts

The Script property of Unicode Standard Annex #24, over Unicode 17.0: the
writing system a character belongs to.

`UnicodeScript::script` answers a character's script as Scripts.txt gives it,
and `Unknown` for a character it gives none. `Script` has a variant for every
script Unicode 17.0 assigns a character to, named the way the Character Database
spells it, and `Script::short_name` answers the four letters ISO 15924 knows it
by. The text layout asks it to choose a font that has a run's script, and the
vector graphics renderer to know whether a script may be letter-spaced.

`tests/the_scripts.rs` holds every code point and every short name to the
Database's own files.

The crate began as unicode-script 0.5.8, by Manish Goregaokar, and keeps only
what this workspace asks of it. Its MIT and Apache-2.0 licenses are retained;
this workspace offers its copy under AGPL-3.0-or-later.
