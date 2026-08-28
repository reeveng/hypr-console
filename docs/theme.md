# Blossom

The desktop is pink, dark, and measured.

## One place

Every colour is decided in `theme/palette.toml`. Nothing else on the machine
decides one.

Almost nothing on the machine holds one either. A stylesheet, a Lua table, a
TOML file, an ini file, a shell script and a browser cannot share a variable
with each other, but most of them can import a file written in their own
language. So `tools/legion-theme` writes one small palette file per language,
and everything else imports whichever speaks its own:

| The palette, written | Imported by |
| --- | --- |
| `~/.config/legion/palette.css` | waybar, wofi, the settings panel, GTK 3, GTK 4, libadwaita, Breeze |
| `~/.config/legion/palette.toml` | alacritty |
| `~/.mozilla/firefox/legion/chrome/palette.css` | `userChrome.css`, `userContent.css` |
| `/usr/local/lib/legion/palette.sh` | `osk-start`, which hands the colours to wvkbd as arguments |

The stylesheets say `@pink` and `@text`. The keyboard says `"$pink"`. None of
them holds a hex, so none of them can fall behind.

The names libadwaita asks for and the longer list Breeze asks for are defined
in `palette.css` as references rather than as colours, so a role changing its
shade changes every name that stands for it.

Three cannot import anything and are written into between a pair of markers:

- `kdeglobals`, because KDE's ini format has no include. It is also the odd one
  out in another way: Qt wants three decimal numbers rather than a hex.
- `user.js`, because a Firefox preferences file is a list of literals.
- `hyprland.lua`. Lua could import it, and the compositor is the one place
  where a file failing to load costs the whole session rather than one window,
  so its two border colours are written rather than read.

The wallpaper and the placeholder icon are drawn.

## Where to change it

`theme/palette.toml`, then `make theme`. Nothing else.

Two tests stand behind that. One refuses a checkout where a generated file no
longer matches the palette. The other reads every file under `files/` and
refuses any colour, in any of the five ways a colour is written down here,
that the palette does not declare. A hex typed in by hand is invisible until
somebody looks at the screen in the right light, and by then it has been there
for months.

## How the colours are chosen

They are not chosen. A colour is declared as a hue and how much of it, together
with what it has to be readable against, and the lightness is computed: the
softest shade of that hue that still clears the ratio it was given.

That is why the theme can be pastel and still legible. Pastel and readable pull
the same way on a dark ground, so asking for the palest colour that clears 7:1
gets a colour that is both, and gets it again on its own if the ground behind
it ever moves.

## What is promised

Everything that is read clears **7:1**, which is AAA at any size. Every pairing
is in `theme/report.md` with the ratio it actually reached, rewritten whenever
the palette is.

The exceptions are named:

- A border is looked at rather than read, so `edge` clears the **3:1** a border
  needs.
- `ash` is what a terminal means when it asks for black. It clears **4.5:1**
  and no more, because black at 7:1 would be lighter than half the palette and
  would stop meaning black.

Nothing else is under AAA, including text Qt considers disabled and text a menu
has greyed out. Fading an entry until it cannot be read is a convention, and on
a device somebody is meant to be able to use it is a bad one, so Qt's dimming
is turned off rather than tuned.

The two scripts that print to a terminal ask for colours by number rather than
by shade, so they answer to the same palette. Neither uses the dim attribute,
which halves whatever colour it lands on: half of a colour picked to clear 7:1
is a colour that does not.

## Where the numbers come from

Contrast is WCAG 2.1 relative luminance, measured after the colour has been
quantised to eight bits a channel. That is what a checker reads off a screen,
and it sits about a tenth of a point away from the same arithmetic done on the
unrounded values: the difference between a palette that measures 7.02:1 and one
that measures 6.92:1 to anybody who tests it.

`theme/colour.py` is the arithmetic, in Oklch. It is the same arithmetic as
`Codincod.Design.Oklch` in the Codincod repository, which was written first and
for a different purpose, and the two were checked against each other: colours
and ratios agree to four decimal places. Those cases are vectors in
`tests/test_theme.py`, so this implementation cannot drift away from the other
one without a test saying so.
