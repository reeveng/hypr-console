# Blossom

A pink dark theme for a handheld held at arm's length.

Written by `console-theme` from `theme/palette.toml`. Every number
here is measured after the colour has been quantised to eight bits a
channel, which is what a contrast checker reads off the screen and is a
tenth of a point away from the arithmetic on the same two colours.

Every pairing is measured twice. The ratio is WCAG 2, which is what the
law asks for and what a checker will report. `Lc` is APCA, which knows
which of the two colours is the paper: it is negative here because
everything on this desktop is pale ink on a dark ground. A colour is
lifted until it clears both, and on a palette this dark it is almost
always `Lc` that decides where it lands.

## The colours

| | colour | spent on |
| --- | --- | --- |
| `night` | `#110b12` | the deepest ground: the terminal, the picture behind everything, and the ink carried on top of a pastel fill |
| `ground` | `#231b26` | the bar, the strip a set of tabs sits in, and the space behind the keyboard |
| `panel` | `#372c3a` | a window, a menu, a card, a key: whatever is in front of the wallpaper |
| `fill` | `#723b5f` | the length a notification is filled to: the volume, the brightness, the battery |
| `edge` | `#8a7d8e` | a line between two things. It is not read, so it clears the 3:1 an edge needs and no more |
| `ash` | `#ae8baa` | what a terminal means by black. A program that prints in it is asking for something quiet, and this is as quiet as a thing may be and still be read |
| `text` | `#f7e7f3` | everything that is read |
| `soft` | `#e5cde0` | what is beside the thing being read: a shortcut, a unit, a workspace you are not on |
| `pink` | `#ffc2e7` | this one: the workspace you are on, the highlighted row, the key under your thumb, the window you are typing into |
| `rose` | `#ffc5ce` | the cursor in the terminal, and a selection |
| `mauve` | `#dfcbff` | sound, and a key held down |
| `lilac` | `#cad1ff` | bluetooth |
| `sky` | `#a4dbff` | a link, and blue where a program asks for blue |
| `mint` | `#95e2d0` | the network, and the setting that is in effect |
| `leaf` | `#aae1ad` | charging, and anything that went well |
| `butter` | `#e6d48a` | the battery getting low, and a warning |
| `peach` | `#ffc9a0` | a number, and orange where a program asks for orange |
| `coral` | `#ffc6c4` | the battery about to go, something disconnected, and anything that failed |

## What was asked of them

| front | on | asked | got | | asked | got | |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `text` | `night` | 7:1 | **16.36:1** | AAA | Lc 90 | **-94.8** | Lc 90, preferred for body text |
| `text` | `ground` | 7:1 | **14.07:1** | AAA | Lc 90 | **-93.3** | Lc 90, preferred for body text |
| `text` | `panel` | 7:1 | **11.16:1** | AAA | Lc 90 | **-90.0** | Lc 90, preferred for body text |
| `soft` | `night` | 7:1 | **13.09:1** | AAA | Lc 75 | **-80.1** | Lc 75, body text |
| `soft` | `ground` | 7:1 | **11.26:1** | AAA | Lc 75 | **-78.6** | Lc 75, body text |
| `soft` | `panel` | 7:1 | **8.93:1** | AAA | Lc 75 | **-75.3** | Lc 75, body text |
| `text` | `fill` | 7:1 | **7.05:1** | AAA | Lc 75 | **-80.4** | Lc 75, body text |
| `fill` | `panel` | 1.05:1 | **1.58:1** | a visible step | -- | **-8.5** | not a contrast claim |
| `night` | `pink` | 7:1 | **13.07:1** | AAA | Lc 75 | **80.5** | Lc 75, body text |
| `night` | `rose` | 7:1 | **13.08:1** | AAA | Lc 75 | **80.5** | Lc 75, body text |
| `night` | `mauve` | 7:1 | **13.07:1** | AAA | Lc 75 | **80.5** | Lc 75, body text |
| `night` | `lilac` | 7:1 | **13.03:1** | AAA | Lc 75 | **80.3** | Lc 75, body text |
| `night` | `sky` | 7:1 | **13.10:1** | AAA | Lc 75 | **80.6** | Lc 75, body text |
| `night` | `mint` | 7:1 | **13.02:1** | AAA | Lc 75 | **80.3** | Lc 75, body text |
| `night` | `leaf` | 7:1 | **13.03:1** | AAA | Lc 75 | **80.3** | Lc 75, body text |
| `night` | `butter` | 7:1 | **13.10:1** | AAA | Lc 75 | **80.6** | Lc 75, body text |
| `night` | `peach` | 7:1 | **13.06:1** | AAA | Lc 75 | **80.5** | Lc 75, body text |
| `night` | `coral` | 7:1 | **13.08:1** | AAA | Lc 75 | **80.5** | Lc 75, body text |
| `pink` | `night` | 7:1 | **13.07:1** | AAA | Lc 75 | **-80.1** | Lc 75, body text |
| `pink` | `ground` | 7:1 | **11.24:1** | AAA | Lc 75 | **-78.6** | Lc 75, body text |
| `pink` | `panel` | 7:1 | **8.92:1** | AAA | Lc 75 | **-75.3** | Lc 75, body text |
| `rose` | `night` | 7:1 | **13.08:1** | AAA | Lc 75 | **-80.1** | Lc 75, body text |
| `rose` | `ground` | 7:1 | **11.25:1** | AAA | Lc 75 | **-78.6** | Lc 75, body text |
| `rose` | `panel` | 7:1 | **8.92:1** | AAA | Lc 75 | **-75.3** | Lc 75, body text |
| `mauve` | `night` | 7:1 | **13.07:1** | AAA | Lc 75 | **-80.1** | Lc 75, body text |
| `mauve` | `ground` | 7:1 | **11.24:1** | AAA | Lc 75 | **-78.5** | Lc 75, body text |
| `mauve` | `panel` | 7:1 | **8.92:1** | AAA | Lc 75 | **-75.3** | Lc 75, body text |
| `lilac` | `night` | 7:1 | **13.03:1** | AAA | Lc 75 | **-79.8** | Lc 75, body text |
| `lilac` | `ground` | 7:1 | **11.20:1** | AAA | Lc 75 | **-78.3** | Lc 75, body text |
| `lilac` | `panel` | 7:1 | **8.89:1** | AAA | Lc 75 | **-75.0** | Lc 75, body text |
| `sky` | `night` | 7:1 | **13.10:1** | AAA | Lc 75 | **-80.2** | Lc 75, body text |
| `sky` | `ground` | 7:1 | **11.27:1** | AAA | Lc 75 | **-78.7** | Lc 75, body text |
| `sky` | `panel` | 7:1 | **8.94:1** | AAA | Lc 75 | **-75.4** | Lc 75, body text |
| `mint` | `night` | 7:1 | **13.02:1** | AAA | Lc 75 | **-79.9** | Lc 75, body text |
| `mint` | `ground` | 7:1 | **11.20:1** | AAA | Lc 75 | **-78.3** | Lc 75, body text |
| `mint` | `panel` | 7:1 | **8.88:1** | AAA | Lc 75 | **-75.1** | Lc 75, body text |
| `leaf` | `night` | 7:1 | **13.03:1** | AAA | Lc 75 | **-79.9** | Lc 75, body text |
| `leaf` | `ground` | 7:1 | **11.21:1** | AAA | Lc 75 | **-78.4** | Lc 75, body text |
| `leaf` | `panel` | 7:1 | **8.89:1** | AAA | Lc 75 | **-75.1** | Lc 75, body text |
| `butter` | `night` | 7:1 | **13.10:1** | AAA | Lc 75 | **-80.2** | Lc 75, body text |
| `butter` | `ground` | 7:1 | **11.26:1** | AAA | Lc 75 | **-78.6** | Lc 75, body text |
| `butter` | `panel` | 7:1 | **8.93:1** | AAA | Lc 75 | **-75.4** | Lc 75, body text |
| `peach` | `night` | 7:1 | **13.06:1** | AAA | Lc 75 | **-80.1** | Lc 75, body text |
| `peach` | `ground` | 7:1 | **11.24:1** | AAA | Lc 75 | **-78.5** | Lc 75, body text |
| `peach` | `panel` | 7:1 | **8.91:1** | AAA | Lc 75 | **-75.3** | Lc 75, body text |
| `coral` | `night` | 7:1 | **13.08:1** | AAA | Lc 75 | **-80.1** | Lc 75, body text |
| `coral` | `ground` | 7:1 | **11.25:1** | AAA | Lc 75 | **-78.6** | Lc 75, body text |
| `coral` | `panel` | 7:1 | **8.92:1** | AAA | Lc 75 | **-75.3** | Lc 75, body text |
| `soft` | `ground` | 7:1 | **11.26:1** | AAA | Lc 75 | **-78.6** | Lc 75, body text |
| `edge` | `ground` | 3:1 | **4.30:1** | clears the 3:1 a border needs | Lc 30 | **-33.5** | clears the Lc 30 a border needs |
| `edge` | `panel` | 3:1 | **3.41:1** | clears the 3:1 a border needs | Lc 30 | **-30.3** | clears the Lc 30 a border needs |
| `ash` | `night` | 4.5:1 | **6.55:1** | AA, on purpose | Lc 45 | **-45.3** | Lc 45, on purpose |
| `ground` | `night` | 1.05:1 | **1.16:1** | a visible step | -- | **0.0** | not a contrast claim |
| `panel` | `ground` | 1.1:1 | **1.26:1** | a visible step | -- | **0.0** | not a contrast claim |

## The terminal

| slot | normal | | | bright | | |
| --- | --- | --- | --- | --- | --- | --- |
| black | `#ae8baa` | 6.55:1 | -45.3 | `#c4a0c0` | 8.46:1 | -56.5 |
| red | `#ffc6c4` | 13.08:1 | -80.1 | `#ffe7e6` | 16.50:1 | -95.4 |
| green | `#aae1ad` | 13.03:1 | -79.9 | `#c0f8c3` | 16.16:1 | -94.0 |
| yellow | `#e6d48a` | 13.10:1 | -80.2 | `#fdeba0` | 16.26:1 | -94.4 |
| blue | `#a4dbff` | 13.10:1 | -80.2 | `#d5eeff` | 16.20:1 | -94.1 |
| magenta | `#ffc2e7` | 13.07:1 | -80.1 | `#ffe6f5` | 16.56:1 | -95.7 |
| cyan | `#95e2d0` | 13.02:1 | -79.9 | `#acf9e7` | 16.16:1 | -94.0 |
| white | `#e5cde0` | 13.09:1 | -80.1 | `#f7e7f3` | 16.36:1 | -94.8 |
