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

STYLE = f"""
@import url("{PALETTE}");

window {{ background-color: @panel; border: 3px solid @pink; border-radius: 16px; }}
* {{ font-family: "Noto Sans"; font-size: 18px; transition: none; }}

#strip {{ background-color: @ground; border-radius: 12px; padding: 6px; margin: 14px 14px 0 14px; }}
#tab {{ background-image: none; background-color: transparent; border: none; box-shadow: none;
       color: @soft; padding: 10px 22px; border-radius: 9px; font-weight: bold; }}
#tab:hover {{ color: @text; }}
#tab.here {{ background-color: @pink; color: @night; }}

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

#panel {{ padding: 10px 14px 14px 14px; }}
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
    subprocess.run(["controller-profile", profile],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


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

        self.window = Gtk.Window()
        GtkLayerShell.init_for_window(self.window)
        GtkLayerShell.set_layer(self.window, GtkLayerShell.Layer.OVERLAY)
        GtkLayerShell.set_keyboard_mode(self.window, GtkLayerShell.KeyboardMode.EXCLUSIVE)
        # As tall as its longest tab, and no taller than it is allowed to be.
        # Sized to each tab in turn it would jump about as you moved along the
        # strip; sized to one fixed number it leaves a floor of empty panel
        # under a short tab, which reads as a page still loading.
        self.width = width
        self.tallest = height
        self.window.connect("destroy", Gtk.main_quit)
        self.window.connect("key-press-event", self.pressed)

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.window.add(box)

        # The tabs share the width between them. There is room for it, and a
        # tab the width of its word is a small thing to hit with a thumb.
        self.strip = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        self.strip.set_name("strip")
        self.strip.set_homogeneous(True)
        box.pack_start(self.strip, False, False, 0)

        self.rows = Gtk.ListBox()
        self.rows.set_name("panel")
        self.rows.set_activate_on_single_click(True)
        self.rows.connect("row-activated", self.chose)
        self.rows.connect("row-selected", self.moved)

        # The rows are held off the border by the scroller's own margins
        # rather than by padding inside it. Padding scrolls away with the
        # content, so a long list ran its last row over the panel's edge and
        # rubbed out the line it was drawn in.
        scroller = Gtk.ScrolledWindow()
        scroller.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        scroller.set_propagate_natural_height(False)
        scroller.set_margin_start(14)
        scroller.set_margin_end(14)
        scroller.set_margin_bottom(14)
        scroller.set_margin_top(10)
        scroller.add(self.rows)
        box.pack_start(scroller, True, True, 0)

        for index, (name, _) in enumerate(self.pages):
            button = Gtk.Button(label=name)
            button.set_name("tab")
            button.connect("clicked", self.tab_clicked, index)
            self.strip.pack_start(button, True, True, 0)

        style = Gtk.CssProvider()
        style.load_from_data(STYLE)
        Gtk.StyleContext.add_provider_for_screen(
            Gdk.Screen.get_default(), style, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

        self.draw()

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
        for index, (says, _) in enumerate(self.pages):
            if says.lower() == wanted:
                return index
        return 0

    def line(self, says, aside, does):
        """One row, laid out for reading or for picking."""
        line = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        label = Gtk.Label(label=says, xalign=0)
        if self.column:
            label.set_size_request(self.column, -1)
            line.pack_start(label, False, False, 0)
            said = Gtk.Label(label=aside, xalign=0)
            said.set_name("said")
            said.set_line_wrap(True)
            line.pack_start(said, True, True, 0)
            return line

        line.pack_start(label, True, True, 0)
        if aside:
            note = Gtk.Label(label=aside)
            note.set_name("aside")
            line.pack_end(note, False, False, 0)
        return line

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
        """Put the tab that is open in front, and its rows underneath."""
        for index, button in enumerate(self.strip.get_children()):
            marks = button.get_style_context()
            marks.add_class("here") if index == self.here else marks.remove_class("here")

        for old in self.rows.get_children():
            self.rows.remove(old)

        for spec in self.pages[self.here][1]:
            says, aside, does = spec[0], spec[1], spec[2]
            row = Gtk.ListBoxRow()
            row.does = does
            row.level = spec[3] if len(spec) > 3 else None
            if does is not None and aside == "now":
                row.get_style_context().add_class("now")
            row.add(self.line(says, aside, does))
            self.rows.add(row)

        self.rows.show_all()
        self.fit()

        # Stay where you were. A panel that redraws itself after every change
        # and drops you back at the top is a panel you cannot turn a volume up
        # in without counting rows again.
        staying = self.rows.get_row_at_index(min(self.at, len(self.pages[self.here][1]) - 1))
        if staying:
            self.rows.select_row(staying)
        self.rows.grab_focus()

    def fit(self):
        """Ask the window for the room its longest tab needs, once it is known.

        The row height comes from GTK rather than from a number written here,
        so it stays right whatever the font ends up being.
        """
        first = self.rows.get_row_at_index(0)
        if first is None:
            return
        _, tall = first.get_preferred_height()
        _, strip = self.strip.get_preferred_height()
        most = max(len(page[1]) for page in self.pages)
        self.window.set_size_request(
            self.width, min(strip + most * tall + 24, self.tallest))

    def moved(self, _list, row):
        if row is not None:
            self.at = row.get_index()

    def nudge(self, step):
        """Left and right, on a row that carries a level."""
        row = self.rows.get_selected_row()
        if row is None or row.level is None:
            return
        row.level(step)
        self.refresh()

    def turn(self, step):
        going = min(max(self.here + step, 0), len(self.pages) - 1)
        if going != self.here:
            self.here = going
            self.at = 0
            self.draw()

    def tab_clicked(self, _button, index):
        self.here = index
        self.at = 0
        self.draw()

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
        Gtk.main()
    finally:
        controller("desktop")
