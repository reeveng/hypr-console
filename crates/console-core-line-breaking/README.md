# Line breaking

Where a line of text may end, by Unicode Standard Annex #14 under Unicode
15.0: `linebreaks` names the byte positions in a string at which it may wrap
and whether the break there is allowed or mandatory, and `linebreaks_iter`
answers the same for characters handed in one at a time, so a caller that has
to be able to stop part way can check before each one it supplies. The end of
the text is always a break, including the end of input that stopped early.

The text layout and the diagram renderer both wrap through it. `tables.rs` is
the classes as one sorted list of the code points where the class changes, and
the pair table the annex's rules reduce to.

It began as unicode-linebreak 0.1.5, by Axel Forsman. Its Apache notice is
retained in `LICENSE`; this workspace offers its copy under AGPL-3.0-or-later.
