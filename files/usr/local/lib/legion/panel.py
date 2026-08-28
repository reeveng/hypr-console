"""A panel: tabs across the top, and under them only what that tab is about.

Everything on this device that has more than one section is drawn this way, so
that a section is a place you are rather than one more row to pick. wofi draws
a list and nothing else, which is right for a list of applications and wrong
for anything with sections in it.

A page is (what the tab says, its rows). A row is (what it says, what is
written beside it, what it does, and, where there is one, what left and right
do to it). A row that does nothing is read rather than chosen, which is what a
guide is made of. What it does is a command to run and leave on, or a function
to call and stay: that one is for a panel whose rows change what they say,
like a list of networks.

Pass a function instead of a list of pages and it is called again whenever
something changes, so the panel redraws itself rather than going stale.

The shoulders move between tabs, matching the shoulders that move between
workspaces on the desktop, and they stop at the ends: a button that wraps
sometimes goes the other way, and then it is a button you have to think about.

Up and down move between rows and left and right move within one, which is
what a row carrying a level is for. A row with nothing to move ignores them.

Everything a button does here a finger does too, because the screen is a
touchscreen and the controller is not always in somebody's hands. A tab is
tapped, a row acts when it is tapped, a row carrying a level draws the two
ends of it either side of the reading, and the strip finishes with the mark
that closes the panel.

One panel is on the screen at a time. Which one, and what happens when a
second is asked for, is `chooser`; this draws whatever it is told to draw.
"""
import gi

gi.require_version("Gdk", "3.0")
gi.require_version("Gtk", "3.0")
gi.require_version("GtkLayerShell", "0.1")

import signal
import subprocess
import threading

from gi.repository import Gdk, GLib, Gtk, GtkLayerShell

# There is no colour here. The palette is written from theme/palette.toml into
# a stylesheet every GTK surface on this machine imports, and this is the one
# place that has to name it absolutely: a stylesheet loaded from a string has
# no directory for a relative import to be relative to.
PALETTE = f"file://{GLib.get_user_config_dir()}/legion/palette.css"

# What the panel keeps between itself and whatever else is on the screen. A
# card that reaches the bar at the top and the keyboard at the bottom reads as
# a thing wedged into a gap rather than a thing lying on the desktop.
BREATH = 16

# The line drawn round the card. Named because the panel has to subtract it
# when it works out how many rows will fit, and a number the drawing knows and
# the measuring does not is how a list ends up cut through its last row.
EDGE = 3

STYLE = f"""
@import url("{PALETTE}");

/* The surface is the whole of the room the compositor left and almost none of
   it is painted. The panel is the card in the middle of it. */
window {{ background-color: transparent; }}
#card {{ background-color: @panel; border: {EDGE}px solid @pink; border-radius: 16px; }}
* {{ font-family: "Noto Sans"; font-size: 18px; transition: none; }}

#top {{ margin: 14px 14px 0 14px; }}
#strip {{ background-color: @ground; border-radius: 12px; padding: 6px; }}
#tab {{ background-image: none; background-color: transparent; border: none; box-shadow: none;
       color: @soft; padding: 10px 22px; border-radius: 9px; font-weight: bold; }}
#tab:hover {{ color: @text; }}
#tab.here {{ background-color: @pink; color: @night; }}

/* The way out, for a hand with no B under its thumb. It is a tab's height and
   sits at the end of the strip, which is the corner a thumb reaches for. */
#shut {{ background-image: none; background-color: @ground; border: none;
        box-shadow: none; color: @soft; font-weight: bold;
        padding: 10px 20px; margin-left: 8px; border-radius: 12px; }}
#shut:hover {{ background-color: @pink; color: @night; }}

/* The two ends of a level. They do to the row they sit on what left and right
   do to the row you are standing on, so a volume can be turned up by somebody
   holding nothing. Dark on the highlighted row rather than pink, because two
   pinks touching is the panel asking a question it does not mean. */
#step {{ background-image: none; background-color: @ground; border: none;
        box-shadow: none; font-weight: bold;
        padding: 2px 20px; margin: 0 8px; border-radius: 8px; }}
row:selected #step {{ background-color: @night; }}
/* The mark, rather than the button it is on. A button carries a label and the
   rule above it dresses every label in a highlighted row in the row's own dark
   ink, which on a dark step is a mark that is there and cannot be seen. */
#step label {{ color: @text; }}
#step:hover {{ background-color: @pink; }}
#step:hover label {{ color: @night; }}

scrolledwindow, list {{ background-color: transparent; }}
/* A row is a card on the panel rather than a line in a list, because a thumb
   aims at a shape. Left unsaid it would take the widget theme's base colour,
   which is a colour this palette never chose. */
row {{ padding: 13px 18px; border-radius: 10px; background-color: @night; }}
row label {{ color: @text; }}

/* The setting that is in effect, which is not the same thing as the row your
   thumb is on. Two pinks side by side made the panel ask a question it did not
   mean to ask, so the highlight keeps the pink and what is already true is
   said in mint. */
row.now label {{ color: @mint; }}

row:selected {{ background-color: @pink; }}
row:selected label {{ color: @night; font-weight: bold; }}

/* Held off the sides, and nothing above or below: space there is the
   scroller's to give. Given here it belongs to the list, which starts the
   rows that far down a viewport counted in whole rows and pushes the last
   one the same distance past the bottom of it. */
#panel {{ padding: 0 14px; }}
#aside {{ color: @soft; font-size: 15px; }}
row:selected #aside {{ color: @night; }}
#said {{ color: @soft; }}
row:selected #said {{ color: @night; }}

#asked {{ color: @text; padding: 4px 4px 10px 4px; }}
entry {{ background-color: @ground; color: @text; border: 2px solid @pink;
        border-radius: 9px; padding: 10px 12px; caret-color: @pink; }}
""".encode()


def later(argv, then):
    """Run something slow without the panel going deaf while it happens.

    Connecting to a network takes seconds. Waiting for it on the drawing
    thread stops the panel answering the buttons, which reads as a machine
    that has crashed rather than one that is working.
    """
    def work():
        try:
            subprocess.run(argv, capture_output=True, text=True, timeout=45)
        except (OSError, subprocess.SubprocessError):
            pass
        GLib.idle_add(then)

    threading.Thread(target=work, daemon=True).start()


def controller(profile):
    """Tell the controller which buttons this is, and open regardless.

    The panel says this before it draws, so anything slow or missing here is a
    menu that never appears. Told nothing, the buttons keep the meaning they
    had, which is a menu you drive with the pointer rather than no menu.
    """
    try:
        subprocess.run(["controller-profile", profile], timeout=5,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except (OSError, subprocess.SubprocessError):
        pass


class Panel:
    def __init__(self, pages, width=620, column=0, height=470, start=None):
        """`column` is how wide the first words are held, for a page that reads
        as two columns rather than as a list of things to pick. `height` is as
        tall as the panel may get before it scrolls instead."""
        self.build = pages if callable(pages) else (lambda: pages)
        self.pages = self.build()
        self.column = column
        self.here = self.find(start)
        self.at = 0
        self.asking = None
        self.reading = 0
        self.asked = None
        # What each tab said last time, so coming back to one shows it at
        # once and corrects itself a moment later rather than blinking
        # empty while the machine is asked all over again.
        self.remembered = {}

        self.window = Gtk.Window()
        GtkLayerShell.init_for_window(self.window)
        GtkLayerShell.set_layer(self.window, GtkLayerShell.Layer.OVERLAY)
        GtkLayerShell.set_keyboard_mode(self.window, GtkLayerShell.KeyboardMode.EXCLUSIVE)

        # Anchored to all four edges, and claiming no exclusive zone of its
        # own, so the compositor hands over exactly the room nothing else has
        # taken: the screen less the bar, and less the on-screen keyboard
        # while that is up. The panel is measured against that room rather
        # than against the screen, so it never has to know that a bar or a
        # keyboard exist, and it is told again the moment either changes.
        #
        # Before this the surface was unanchored, which centres it and leaves
        # it whatever height it asked for. With the keyboard up that was 34
        # too tall for the gap between the two: it hung over the bar and its
        # last rows were behind the keys.
        for edge in (GtkLayerShell.Edge.TOP, GtkLayerShell.Edge.BOTTOM,
                     GtkLayerShell.Edge.LEFT, GtkLayerShell.Edge.RIGHT):
            GtkLayerShell.set_anchor(self.window, edge, True)

        # Most of that surface is not drawn on, which needs a visual that can
        # hold nothing. Without it the untouched part is opaque black and the
        # panel is a card on a sheet rather than a card on the desktop.
        clear = Gdk.Screen.get_default().get_rgba_visual()
        if clear is not None:
            self.window.set_visual(clear)
        # As tall as its longest tab, and no taller than it is allowed to be.
        # Sized to each tab in turn it would jump about as you moved along the
        # strip; sized to one fixed number it leaves a floor of empty panel
        # under a short tab, which reads as a page still loading.
        self.width = width
        self.tallest = height
        self.window.connect("destroy", Gtk.main_quit)
        self.window.connect("key-press-event", self.pressed)
        self.window.connect("size-allocate", self.reshaped)
        self.reshaping = False

        # The panel, as opposed to the surface it is drawn on: the size it
        # asks for is asked for here, and the compositor's answer about the
        # room is the window's.
        self.card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.card.set_name("card")
        self.card.set_halign(Gtk.Align.CENTER)
        self.card.set_valign(Gtk.Align.CENTER)
        self.window.add(self.card)

        # The strip, and beside it the way out. They are held in one row so
        # that the panel is as tall as the taller of the two, whichever that
        # turns out to be once the font is known.
        self.top = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        self.top.set_name("top")
        self.card.pack_start(self.top, False, False, 0)

        # The tabs share the width between them. There is room for it, and a
        # tab the width of its word is a small thing to hit with a thumb.
        self.strip = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        self.strip.set_name("strip")
        self.strip.set_homogeneous(True)
        self.top.pack_start(self.strip, True, True, 0)

        # B closes a panel and a finger has no B. This is the same door said
        # in the other language, and it is the only one the bar's icons can
        # reach: they open a panel over a screen where nothing else answers.
        shut = Gtk.Button(label="\u00d7")
        shut.set_name("shut")
        shut.connect("clicked", lambda _button: self.window.destroy())
        self.top.pack_end(shut, False, False, 0)

        self.rows = Gtk.ListBox()
        self.rows.set_name("panel")
        self.rows.set_activate_on_single_click(True)
        self.rows.connect("row-activated", self.chose)
        self.rows.connect("row-selected", self.moved)

        # The pointer moves the highlight, so there is one answer to where you
        # are rather than two. A is a keypress and a keypress acts on what is
        # highlighted; without this, moving the pointer over a row and pressing
        # A chose whatever the highlight had been left on somewhere else.
        self.rows.add_events(Gdk.EventMask.POINTER_MOTION_MASK)
        self.rows.connect("motion-notify-event", self.hovered)

        # The rows are held off the border by the scroller's own margins
        # rather than by padding inside it. Padding scrolls away with the
        # content, so a long list ran its last row over the panel's edge and
        # rubbed out the line it was drawn in.
        self.scroller = Gtk.ScrolledWindow()
        self.scroller.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.scroller.set_propagate_natural_height(False)
        self.scroller.set_margin_start(14)
        self.scroller.set_margin_end(14)
        self.scroller.set_margin_bottom(14)
        self.scroller.set_margin_top(10)
        self.scroller.add(self.rows)
        self.card.pack_start(self.scroller, True, True, 0)

        for index, page in enumerate(self.pages):
            button = Gtk.Button(label=page[0])
            button.set_name("tab")
            button.connect("clicked", self.tab_clicked, index)
            self.strip.pack_start(button, True, True, 0)

        style = Gtk.CssProvider()
        style.load_from_data(STYLE)
        Gtk.StyleContext.add_provider_for_screen(
            Gdk.Screen.get_default(), style, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

        self.due = False
        # Every watcher started, so every one of them can be stopped. A panel
        # is opened and closed dozens of times a day and each one used to
        # leave its `pactl subscribe` behind, reparented to init and reading a
        # pipe nobody holds. Twenty-five of them were found alive on the
        # device, the oldest four hours old, which on a handheld is battery.
        self.watchers = []
        self.window.connect("destroy", self.stop_watching)
        for index, page in enumerate(self.pages):
            if len(page) > 3 and page[3]:
                self.watch(index, page[3][0], page[3][1])

        self.draw()
        self.entered()

    def rows_of(self, page):
        """The rows of one tab, asked for at the moment it is drawn.

        A tab that names a function is not computed until you are looking at
        it. Everything here is read off the machine, and reading all of it to
        show one tab meant scanning for networks to open the sound.
        """
        rows = page[1]
        return rows() if callable(rows) else rows

    def watch(self, index, argv, about):
        """Redraw a tab when something outside the panel changes it.

        The volume rocker on the top edge moves the same number the Sound tab
        shows, and a panel that goes on showing the old one is worse than one
        showing nothing: it is a reading, and it is wrong.

        Only lines about `about` count. What these commands report is
        everything the machine is doing, most of it caused by this panel
        reading the machine, and answering all of it means every redraw asks
        for another one. Even the lines that count are answered on a delay, so
        a rocker held down redraws a few times rather than a hundred.

        Both ends of the pipe have to hand a line over as it is written: a
        program whose output is not a terminal holds it back by default, and
        the news arrives in a batch long after it was news.
        """
        def redraw():
            self.due = False
            if self.here == index and self.asking is None:
                self.refresh()
            return False

        def heard():
            if not self.due:
                self.due = True
                GLib.timeout_add(250, redraw)
            return False

        # Started here rather than inside the thread, so the handle is kept
        # before anything can ask for it back.
        try:
            running = subprocess.Popen(argv, stdout=subprocess.PIPE,
                                       text=True, bufsize=1)
        except OSError:
            return
        self.watchers.append(running)

        def work():
            for line in running.stdout:
                if about in line:
                    GLib.idle_add(heard)

        threading.Thread(target=work, daemon=True).start()

    def stop_watching(self, _window):
        """The panel is going, and what it started goes with it.

        The threads are daemons and end with the process. The programs they
        read do not: nothing owns them once this exits, and they sit on init
        holding a pipe with no reader for as long as the machine is up.
        """
        for running in self.watchers:
            running.terminate()

    def entered(self):
        """What a tab wants done when you arrive on it, if anything.

        Drawing a tab shows what is already known; this is for going and
        finding out. The two are separate on purpose, so the panel appears at
        once and fills in, rather than waiting on a radio before it draws.
        """
        page = self.pages[self.here]
        if len(page) > 2 and page[2]:
            page[2](self)

    def find(self, name):
        """Which tab to open on, by the word on it.

        Something on the bar that stands for one of these opens the panel at
        that one, so tapping the battery and pressing Legion right arrive in
        the same place by different roads. A name nothing answers to opens the
        first tab rather than nothing at all.
        """
        if not name:
            return 0
        wanted = name.strip().lower()
        for index, page in enumerate(self.pages):
            if page[0].lower() == wanted:
                return index
        return 0

    def line(self, says, aside, does, level=None):
        """One row, laid out for reading or for picking."""
        line = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        label = Gtk.Label(label=says, xalign=0)
        # Two columns for a row that only tells you something. A row you can
        # act on is drawn like every other row you can act on, wherever it
        # is, so that what is clickable looks clickable.
        if self.column and does is None:
            label.set_size_request(self.column, -1)
            line.pack_start(label, False, False, 0)
            said = Gtk.Label(label=aside, xalign=0)
            said.set_name("said")
            said.set_line_wrap(True)
            line.pack_start(said, True, True, 0)
            return line

        line.pack_start(label, True, True, 0)
        # A level is its two ends with the reading held between them, so the
        # mark that makes it smaller is on the side it shrinks towards and the
        # one that makes it bigger is on the side it grows into. Packed from
        # the right inward: the plus, the reading, the minus.
        if level is not None:
            line.pack_end(self.step(level, "+", 1), False, False, 0)
        if aside:
            note = Gtk.Label(label=aside)
            note.set_name("aside")
            if level is not None:
                # Room kept for the longest the reading gets, counted in
                # characters rather than in pixels so that it holds whatever
                # the font turns out to be. Left to size itself to what it
                # says, the two marks either side would move under the thumb
                # every time the number changed width.
                note.set_width_chars(16)
            line.pack_end(note, False, False, 0)
        if level is not None:
            line.pack_end(self.step(level, "\u2212", -1), False, False, 0)
        return line

    def step(self, level, mark, amount):
        """One end of a level, as something to press."""
        end = Gtk.Button(label=mark)
        end.set_name("step")
        end.connect("clicked", self.stepped, level, amount)
        return end

    def refresh(self):
        """Ask for the rows again, in case they say something else now."""
        self.pages = self.build()
        self.here = min(self.here, len(self.pages) - 1)
        self.draw()
        return False

    def ask(self, question, then, secret=True):
        """Take a line of text, and hand it on.

        The on-screen keyboard is how it gets typed, so X still brings the
        keyboard up over this: the panel keeps the focus, and wvkbd types into
        whatever holds it.
        """
        self.asking = then
        for old in self.rows.get_children():
            self.rows.remove(old)

        row = Gtk.ListBoxRow()
        row.does = None
        row.set_activatable(False)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        label = Gtk.Label(label=question, xalign=0)
        label.set_name("asked")
        box.pack_start(label, False, False, 0)
        entry = Gtk.Entry()
        entry.set_visibility(not secret)
        entry.connect("activate", self.answered)
        box.pack_start(entry, False, False, 0)
        row.add(box)
        self.rows.add(row)
        self.rows.show_all()
        entry.grab_focus()

    def answered(self, entry):
        then, self.asking = self.asking, None
        then(entry.get_text())
        self.refresh()

    def draw(self):
        """Move to the tab, then go and find out what is on it.

        The tab moves on the press and nothing waits for anything. Reading a
        tab means asking the machine, and asking the machine is quick rather
        than instant: held behind it, the strip answers late enough that a
        second press feels like it was not noticed, and the way to press a
        button that ignores you is to press it again.
        """
        self.mark()
        self.place(self.remembered.get(self.here, []))
        self.fill()

    def mark(self):
        """Which tab is in front. Nothing here asks the machine anything."""
        for index, button in enumerate(self.strip.get_children()):
            marks = button.get_style_context()
            marks.add_class("here") if index == self.here else marks.remove_class("here")

    def fill(self):
        """Read the tab somewhere else, and take the answer if it is still wanted.

        Every reading is stamped. Pressing along the strip faster than the
        machine answers leaves earlier readings arriving after later ones, and
        an answer about a tab you have already left is a wrong answer however
        true it was when it was asked.
        """
        self.reading += 1
        stamp, here = self.reading, self.here

        def arrived(rows):
            if stamp == self.reading and self.asking is None:
                self.remembered[here] = rows
                self.place(rows)
            return False

        def work():
            try:
                rows = self.rows_of(self.pages[here])
            except Exception:
                rows = []
            GLib.idle_add(arrived, rows)

        threading.Thread(target=work, daemon=True).start()

    def place(self, rows):
        """Put rows on the tab, keeping where you were standing."""
        for old in self.rows.get_children():
            self.rows.remove(old)

        for spec in rows:
            says, aside, does = spec[0], spec[1], spec[2]
            row = Gtk.ListBoxRow()
            row.does = does
            row.level = spec[3] if len(spec) > 3 else None
            # Whether a row can be chosen has nothing to do with whether it
            # is the one in effect. Asking for both marked the current power
            # profile and left the joined network saying the same word in a
            # different colour, so the two tabs had to be read differently.
            if aside == "now":
                row.get_style_context().add_class("now")
            row.add(self.line(says, aside, does, row.level))
            self.rows.add(row)

        self.rows.show_all()
        self.fit()

        # Stay where you were. A panel that redraws itself after every change
        # and drops you back at the top is a panel you cannot turn a volume up
        # in without counting rows again.
        staying = self.rows.get_row_at_index(min(self.at, len(self.rows.get_children()) - 1))
        if staying:
            self.rows.select_row(staying)
        self.rows.grab_focus()

    def reshaped(self, _window, _allocation):
        """The room changed shape: the keyboard went up, or came down.

        Nothing is measured here. Asking for a size from inside a size
        allocation is asking GTK to lay out while it is laying out, so this
        only says the answer is stale, and the next idle moment works out what
        it should be instead.
        """
        if not self.reshaping:
            self.reshaping = True
            GLib.idle_add(self.refit)

    def refit(self):
        self.reshaping = False
        self.fit()
        return False

    def fit(self):
        """Ask the card for the room it needs, once that is knowable.

        Before the window is on screen a row does not know how tall it is, and
        the answer given then is close enough to look right and wrong enough to
        cut the last row in half. So this is asked again once there is
        something to measure.

        The row height comes from GTK rather than from a number written here,
        so it stays right whatever the font ends up being, and the room comes
        from the compositor rather than from the screen's size, so a keyboard
        or a bar taking part of the screen takes it from the panel too.
        """
        first = self.rows.get_row_at_index(0)
        if first is None:
            return
        _, tall = first.get_preferred_height()
        _, strip = self.top.get_preferred_height()
        # One size, for every tab, always: the panel is a place on the
        # screen and a place does not change shape when you look at a different
        # part of it. A whole number of rows, too, because a list cut through
        # the middle of its last row reads as a broken panel rather than as
        # more to scroll to.
        # What the surface was given, which is the screen less everything
        # that has claimed a piece of it. Before the window is on screen there
        # is nothing to be given, and the panel's own ceiling stands alone.
        given = self.window.get_allocated_height()
        ceiling = self.tallest if given <= 1 \
            else min(self.tallest, given - 2 * BREATH)
        # Everything the card spends on something that is not a row: the tab
        # strip, the margins holding the list off the card's edges, and the
        # line drawn round the whole of it. Asked of the widgets rather than
        # written down here, because a number written twice is a number that
        # goes out of step, and out of step here means a cut row.
        frame = strip + 2 * EDGE + self.scroller.get_margin_top() \
            + self.scroller.get_margin_bottom()
        room = max(tall, ceiling - frame)
        tall_enough = frame + (room // tall) * tall
        # Asking for a size is asking the compositor for it, and every
        # tab is the same size, so asking again on every press is a
        # round trip bought for nothing.
        if tall_enough != self.asked:
            self.asked = tall_enough
            self.card.set_size_request(self.width, tall_enough)

    def hovered(self, _list, event):
        """The pointer moves the highlight, and the cursor with it.

        Selecting a row and standing on it are two different things to GTK: the
        highlight is the selection and the keys act on the cursor. Moved apart,
        the pointer highlights one row and the d-pad carries on from another.
        """
        row = self.rows.get_row_at_y(int(event.y))
        if row is not None and row is not self.rows.get_selected_row():
            self.rows.select_row(row)
            row.grab_focus()
        return False

    def moved(self, _list, row):
        if row is not None:
            self.at = row.get_index()

    def nudge(self, step):
        """Left and right, on a row that carries a level."""
        row = self.rows.get_selected_row()
        if row is None or row.level is None:
            return
        self.stepped(None, row.level, step)

    def stepped(self, _button, level, step):
        """A level moved, by whichever of the two ways there are of moving it.

        The reading is asked for again rather than worked out here. What a
        level is is the machine's answer, and a panel that adds a step to the
        number it drew last time is a panel that drifts away from the thing it
        claims to be showing.
        """
        level(step)
        self.refresh()

    def turn(self, step):
        going = min(max(self.here + step, 0), len(self.pages) - 1)
        if going != self.here:
            self.here = going
            self.at = 0
            self.draw()
            self.entered()

    def tab_clicked(self, _button, index):
        if index == self.here:
            return
        self.here = index
        self.at = 0
        self.draw()
        self.entered()

    def chose(self, _list, row):
        if row.does is None:
            return
        if callable(row.does):
            if row.does(self):
                self.window.destroy()
            return
        subprocess.Popen(row.does, start_new_session=True,
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.window.destroy()

    def pressed(self, _window, event):
        key = event.keyval
        if self.asking is not None:
            # While something is being typed, back means abandon the question,
            # not abandon the panel, and the shoulders are letters not tabs.
            if key == Gdk.KEY_Escape:
                self.asking = None
                self.draw()
                return True
            return False
        if key in (Gdk.KEY_Escape, Gdk.KEY_BackSpace):
            self.window.destroy()
        elif key in (Gdk.KEY_Return, Gdk.KEY_KP_Enter, Gdk.KEY_space):
            # A means the row that is highlighted, whichever way you came
            # to be on it. Left to GTK it means the row the cursor is on,
            # which is not the same row once a pointer is involved.
            chosen = self.rows.get_selected_row()
            if chosen is not None:
                self.chose(self.rows, chosen)
        elif key == Gdk.KEY_Page_Up:
            self.turn(-1)
        elif key == Gdk.KEY_Page_Down:
            self.turn(1)
        elif key == Gdk.KEY_Left:
            self.nudge(-1)
        elif key == Gdk.KEY_Right:
            self.nudge(1)
        else:
            return False
        return True


def show(pages, width=620, column=0, height=0, start=None):
    """Put a panel on screen and wait for it to be dismissed."""
    panel = Panel(pages, width, column, height, start)

    # Asked to stop, put the controller back before going. This goes through
    # the loop GTK is sitting in: a plain signal handler is Python, and Python
    # does not get a turn while that loop is waiting.
    for stopped in (signal.SIGTERM, signal.SIGHUP, signal.SIGINT):
        GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, stopped,
                             lambda: (panel.window.destroy(), False)[1])

    controller("tabs")
    try:
        panel.window.show_all()
        GLib.idle_add(panel.fit)
        Gtk.main()
    finally:
        controller("desktop")
