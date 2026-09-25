# Taking GTK out of the panels

This is done. It is kept because what it argues is what every surface here is
still written to, and because the shape of the argument is the one to make
again the next time a library turns out to be holding a second copy of an
answer this tree already has. Read it in the past tense; the last section says
where it ended.

GTK was the last thing on this device that decided something this tree had
decided better. What a toolkit decides is what fits, what order a press moves
in, what a press means and what colour a thing is, and every one of those is
answered here already, in a place a screen cannot reach. The toolkit sits on top
of all of it holding a second copy in widgets, and it is named in one file of
`console-panel` and in a handful of lines in each panel crate; everything else
in them has never heard of it.

The contract already asked for this and was refused by GTK by name.
`console-program-contract`'s head says the runtime redraws from the state, which
is the whole reason a state is asked to be `PartialEq`, and then gives up the
one loop a paragraph later: GTK draws on one thread and will not be driven from
somebody else's, so half the programs here keep their own. That concession is
what this costs today. Taking the toolkit out is what lets a panel be pressed by
transcript, on a laptop, with no screen and no compositor -- which is the one
thing the contract promises and the panels cannot do.

## What is not being built

A toolkit. The tempting shape is elements with an environment -- measure,
layout, draw, an affordance named per interaction -- and it is a second general
layout engine written in this tree's own words. It was looked at and put down:
this device has one arrangement, a page of rows of a settled height, and an
engine for arbitrary arrangement is a larger thing than the one it replaces. An
element holding its own mutable state is also what `docs/programs.md` exists to
forbid, arriving dressed as a widget.

## The surface already exists

`console-notify` draws its own cards on a surface it owns, via
`console-draw-surface`. The keyboard does the same. These are not candidates for
replacement -- they are the proof the surface works. What they use is what the
panels will use:

- `console-draw-surface` hands out a slice of bytes the width and height of the
  frame and has no opinion about what goes in it.
- `console-draw-painting` puts shapes into that buffer via cairo and pango.
- `console-core-shapes` is the closed list of what can go in it.
- `Scale` handles 2.5 via `wp_viewporter`, with hundred-twentieths and buffer
  scale always 1.

The panels were the last surfaces on GTK, and then the home screen was.

## The shape list

The list of shapes this machine can ever draw, closed and matched over. Two
shapes cover every surface on this device today, the same two
`console-draw-painting` already puts into a buffer:

- **`Panel`** -- a filled rectangle, optionally rounded, optionally edged. The
  card behind a row, the strip along the bottom, the background of a progress
  bar, the highlight the d-pad moves, the question standing over a list. A panel
  knows where it is, how big it is, what colour it is, and what it has around
  it.

- **`Words`** -- a run of text in a face, at a weight, in an ink, wrapping into
  a width. A row's label, a tab's name, the clock in the bar, a note in the
  corner. Words know where they start, how wide they may be, what they say, and
  what they look like.

Everything the panels draw today decomposes into these two. A row is a panel
with words beside it. The tab strip is panels with words on them. A progress
bar is two panels. A question is a panel with words over the list. A letter
heading is words without a panel. The close button is a panel with a word on it.

Whether this list actually closes is the first question and is asked of every
`Shape` there is, before anything is drawn. If it closes, drawing is a match,
and EXPLICIT016 makes a shape added later announce itself everywhere it
matters. A third variant is a compile error at every call site, which is the
announcement.

`Picture` arrived with the rows and the squares, and it is what was said it
would be: the decoder hands over decoded pixels and the painter blits them, it
is not a vector primitive and it does not pretend to be one. `Line` is the
fourth. The list is still closed and every match over it is still exhaustive,
which is the only claim this section was making.

## The measuring boundary

Pango is an input to layout and not part of it, and that boundary is the
design. How tall a wrapped line is, is the one thing placement cannot work out
for itself. The rule:

**Measure into the description first. Place purely afterwards.**

The notification daemon already does this: `measured()` asks Pango how tall each
line wraps to, with no surface in the room, and hands back a `Size`. The shape
builder uses those sizes to position everything with arithmetic. The placing
stays something `Body::Here` can check against stated sizes while the measuring
is checked where there is a screen.

A measurement kept in the state is the first thing that would undo it.

This is why `console-draw-painting::measured()` uses hint metrics off: cairo
hints glyph advances to whole device pixels, so the same string in the same
face is a different width at 1x and at 2x. Hint metrics off is the only setting
under which a measurement taken with no screen is true on every screen, which is
the whole claim the split is making.

## What gets drawn and how

The panels are a page of rows of a settled height. The arrangement is not
arbitrary: one tab strip at the top, rows under it, a card of fixed width, the
whole thing centred. An engine for arbitrary arrangement is a larger thing than
the one it replaces.

What goes into a frame, in order:

1. **Measure** every text run in the page. A `Run` is the text, the weight, and
   the width it may wrap into. Pango returns a `Size`. This happens once per
   redraw, off the drawing thread, with no surface in the room.

2. **Place** every shape using the measurements and the constants in
   `console_panel::shape`, `fitting`, and `strip`. The card width is 93% of the
   room. The ceiling is 80% of the screen. A row is 45 points tall. The strip
   is 82 points. These are the only numbers and they are all shares or
   constants.

3. **Match** the shapes against the frame. A `Panel` is a filled rectangle.
   `Words` are rendered by Pango on cairo. The match is exhaustive and the
   compiler asks for it.

4. **Commit** the buffer to the compositor via `wl_shm`, with the viewporter
   saying how big it is in logical pixels.

## What is rented stays rented

**Pango**, because shaping is not a decision and because Thai is a layer this
desktop wears unless it is told otherwise: a script with no spaces between its
words wraps by dictionary, and a text engine of our own would break the one
feature the on-screen keyboard exists for.

**Wayland**, because it is not a dependency but the way to every program nobody
here is going to write, which is the whole of the Dressed tier below.

**The decoders**, by the rule further down this section.

**Hyprland** is the one that is genuinely takeable -- it holds the input and the
window list, which is where the entry about crossing ambiently has to be
enforced if it is ever enforced at all -- and it comes after the generations,
the recovery surface and the first run, and it is a Smithay compositor rather
than one written from nothing.

## The order of work

The order inverts the obvious one. The panels work today and nobody is held up
by them being somebody else's. What is missing is the surface for the morning
the machine does not come up, and that surface is these same shapes drawn on a
buffer the kernel gave us instead of one a compositor did.

The notification daemon and the keyboard already draw on surfaces of their own.
They are not the work. They are the proof the surface works. The work is the
panels.

1. **The screencopy client.** Written before the surface changes rather than
   after, so the old card and the new one are compared in a picture instead of
   by eye on a laptop that is not the device.

2. **The recovery surface.** The morning the machine does not come up. A
   programme that draws itself from the state, with no compositor answers
   behind it. This is what the contract promised and the panels could not do.

3. **The bar.** `console-bar` moves onto the same surface model.

4. **The panels last.** The tab strip, the rows, the highlight, the scroll.
   Each panel crate hands over a `Card` closure that builds `Vec<Page>`, and
   the host draws them as shapes rather than as widgets.

Taken that way the promise this whole section is about closes early, and the
most enjoyable part of the work is the part that waits.

## Where the panels have got to

`console-panels` draws whichever panel it is asked for on a surface of its own:
`console_panel::surface` measures the rows, places the shapes and commits the
buffer, and nothing in that path is a widget. Two things were missing, and a
panel nobody could see is the first of them -- a layer surface has no size until
the compositor configures one and is configured only once it exists, so a loop
that waited for a size before asking for a surface waited for ever. Every panel
the host drew was drawn into nothing. The surface is stood up before the loop
now, and what it draws is a card, the tab on it, the rows, and the highlight
behind the row the pad is standing on.

The second was the half that makes a drawing a panel: a surface nobody can drive
is a picture. So:

- **A key is a keysym**, read through the keymap the compositor is driving
  rather than through a toolkit's names for keys. `console-draw-surface` binds
  the seat's keyboard and hands back what was pressed; `console_panel::keys` is
  the same arithmetic it always was, now spelled in the words xkb uses. The
  panel still on GTK converts at its own edge, which is one line.
- **What a press means is decided once and away from the screen.**
  `surface::told` takes the state, the meaning and the rows and says what
  happened: the highlight moved, a tab turned, a row was chosen, the panel is
  going away. It never touches a surface, so the d-pad is asserted against with
  no compositor in the room, which is the thing the panels could not do at all.
- **A row that was chosen runs what it says it does**, `Handler::Call` with the
  panel handed in and `Handler::Run` through `console_panel::running`, and what
  the row asks for while it runs -- a note, another tab, the row to stand on --
  is collected and applied after it returns rather than reached for mid-call.

- **A panel that asks something stands over its own list.** A question, a
  yes-or-no and a search line are three states of the same card rather than
  three windows: while one is up the rows are not drawn, the letters that
  arrive go into it, and return answers it. A password draws its letters as
  marks, because a surface of our own draws what it was handed and nothing
  else decides that for it.
- **What else a row offers stands beside the row**, outside the card where a
  thumb is not already resting, and a row that holds a level spends the same
  press on the level instead. Which of the two a row does is `page::wears` and
  `page::holds`, so the answer is the same one the panel on GTK gives.
- **What is measured is what is drawn.** Pango is asked for the size of a title
  and of an aside before anything is placed, and the text is drawn in the font
  and weight it was measured in: a tab measured plain and drawn bold wraps
  inside its own pill, and an aside measured small and drawn large falls off
  the row. Both were on the screen before the measuring was made to match.

- **A row's picture is bytes somebody else decoded.** `Shape::Picture` is the
  third shape and it carries pixels rather than a path, so nothing on the
  drawing loop opens a file or works out a format: the panel's own picture
  store already holds every icon and thumbnail a list wants, at the size a row
  draws it, and the surface reads it and places it. A picture the store has not
  got is asked for from the maker and drawn at the next opening, which is what
  it always did. The store is read again when it has been written again, which
  a process that drew one panel and exited never had to care about.

What is not there yet, and is what a panel opened on this surface cannot do:
draw a photograph or a film at the size of the card, draw an icon asked for by
theme name rather than by file, or draw the rows that are a grid of cells rather
than a list. Each is a shape or a state this page already names; none of them is a
toolkit. A key held down is one press, for the reason `console-draw-surface`'s
own head gives.

## What stays in `console-panel`

The parts that have never heard of GTK:

- **`shape`** -- share-of-screen arithmetic. The card is 93% of the room, the
  ceiling is 80% of the screen. These are the only numbers.

- **`fitting`** -- how much room the panel has, how many rows fit, how tall a
  picture can be. Arithmetic over the room the compositor granted and the
  constants in `strip`.

- **`strip`** -- which tabs fit in the strip. The card is one width whatever is
  written on its tabs, and the arrows at either end are the shoulders.

- **`page`** -- what a page is: the tab, the rows under it, and what each row
  says, does and has beside it. This is the description a renderer would
  otherwise have to invent.

- **`marks`** -- Unicode marks and the names things are known by. Not widgets.

- **`icons`** -- the exhaustive list of icons this desktop needs. Not an icon
  theme lookup.

- **`keys`** -- key meaning and swipe meaning. Input, not drawing.

- **`telling`** -- what a panel drew, for automated checks. The JSON format
  stays; what produces it changes.

- **`notes`** -- persistent per-panel notes. Nothing to do with a display.

- **`tab`** -- remembering which tab was last open. State, not drawing.

- **`pictures`** -- the thumbnail cache. Decoded once, drawn many times.

- **`before`** -- what the machine said last time it was asked. The cache a tab
  draws itself from, filed under `~/.cache/console/asked/`.

- **`card`** -- what a panel crate hands over: the `Card { build, column, start,
  done }` struct. Not a widget.

- **`opening`** -- how long this panel took to appear, stamped as it appears.
  `console_response_times` is where the numbers go.

- **`whose`** -- which panel this is, said rather than worked out. A thread-local
  for the reason `opening` is one.

## What leaves `console-panel`

- **`panel.rs`** -- the GTK widget tree, event handling, the draw pipeline,
  image decoding via glib, the `Panel` struct. This is the toolkit. It is
  replaced by a match over shapes and a `wl_shm` surface.

- **`style.rs` and `style.css`** -- the GTK stylesheet. Colours are in the
  palette and shapes carry their own. There is no CSS selector to match over.

- **`actor.rs`** -- the `Machine` trait. This was the glib main loop adapter.
  The program contract's own loop replaces it.

- **`asked.rs`** -- signal handling via `g_unix_signal_add_full`. Replaced by
  the standard signal handling the runtime already has.

- **`held.rs`** -- asking `console-panels` to draw via Unix socket. The host
  draws directly from the state.

- **`room.rs`** -- remembering last window size. The compositor grants the room
  and the panel takes it.

- **`running.rs`** -- starting processes via glib. The program contract's
  `Effect::Run` already does this.

## What stays in `console-panels`

The host process, but drawing shapes instead of widgets:

- Opens the Wayland connection once.
- Loads the palette once.
- Reads the icon theme once.
- Draws whichever panel it is asked for, using `console-draw-painting::onto()`.
- The one-picker lock via `flock` stays: the kernel drops it however the process
  ends, which is what lets a picker that was killed outright leave nothing
  behind.

The `Card` closure stays: it builds `Vec<Page>`, which is the description. What
changes is what the host does with it: a match over shapes rather than a widget
tree.

## What the panels become

Each panel crate stays where it is and keeps its `card()` function. What it
returns is still `Card { build, column, start, done }`. The `build` closure
still returns `Vec<Page>`. The pages still contain `Row`s with `Does`, `Picture`,
`Level`, and all the rest.

What changes is the rendering side. The host reads the pages, measures the text,
places the shapes, and draws them. The panel crate never sees a surface, a
compositor, or a widget. It describes, and the host draws.

This is the split the contract already argues for: a program says what it means,
and something else decides how big things are and where they go. The difference
is that the "something else" is now a match over two shapes and a buffer from
the kernel, rather than a toolkit holding a second copy in widgets.

## Where it ended

No crate in this tree names gtk4, gtk4-layer-shell, glib or gio. The panels
went first, then the bar, then the files panel and the viewer, then the home
screen -- which was the last window, a grid of boxes over a stylesheet, and is
now `shape::laid`, a `Vec<Shape>` and a layer surface of its own. The music
player was the last caller of gio that drew nothing at all: it kept a
`glib::MainLoop` running because the bus was gio's, and it answers on
`console-bus` now, through the same `heard(held, &message) -> Turn` the
notification daemon is written to.

What replaced the toolkit in `desktop.conf` is what the toolkit was actually
carrying: cairo fills the rectangles, pango shapes and wraps the words, and
libxkbcommon says which letter a keycode is under the compositor's own keymap.
All three were already on every machine as gtk4's dependencies, which is why
nothing changed on the screen the day it left.

Three things named GTK stay, and none of them is this desktop drawing:

- **glib2**, because pango is glib2's and because `gio trash` is still how the
  Files panel deletes -- a delete somebody can take back is worth a package.
- **The GTK stylesheet the palette writes.** waybar, wofi, libadwaita and
  Breeze read GTK CSS, and the colours those programs wear are still decided in
  `theme/palette.toml` like everything else.
- **`.config/gtk-3.0` and `.config/gtk-4.0`.** Somebody else's program arriving
  as a flatpak finds this desktop's shades already written down.

What the contract gave up for GTK is what taking it out bought back. A panel is
pressed by transcript now, on a laptop, with no screen and no compositor, which
is the sentence `console-program-contract` opens with and could not keep while a
toolkit owned the loop.
