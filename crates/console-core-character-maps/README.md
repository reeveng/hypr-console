# Character maps

The maps a PDF reads its text through: a code in a string to the CID it
selects, and a code to the Unicode text it stands for. A cmap a PDF carries
as a PostScript program and one of Adobe's predefined cmaps, which are bundled
in `assets` in a zlib-compressed binary form, are read into the same `CMap`,
and a lookup asks each file of a `usecmap` chain in turn.

Extracted from Hayro's PDF reader, where it was hayro-cmap by Laurenz
Stampfl. The upstream MIT and Apache notices are retained in `LICENSE-MIT`
and `LICENSE-APACHE`; this workspace offers its copy under AGPL-3.0-or-later.
