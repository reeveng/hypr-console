"""Only one chooser is ever up, and it is the last one asked for.

The menu is on a button, on a paddle and on a key; the settings are on a button
and on four of the bar's icons. Every one of those roads used to start a
process that knew nothing about the others, and two choosers at once take each
other's controller profile: the second to open claims it, the first to close
hands the desktop's buttons back while the other is still on screen. Since both
are drawn in the same place, backing out of one leaves you looking at what
appears to be the same chooser refusing to close.

Turning the second one away instead would be worse in the one case that
matters: the bar is reachable with a finger while a chooser is up, and an icon
that does nothing at all reads as a broken bar. So the one on screen goes. Ask
through the door it came out of and nothing replaces it, which is how a finger
closes a panel it opened from the bar.
"""

import importlib.machinery
import importlib.util
import os
import subprocess
import sys
from pathlib import Path

# A chooser that takes the screen and keeps it, so that a test can be the
# second one. It says so on the way in: waiting for a line is waiting for the
# lock to be held, where waiting for a moment is a test that fails on a busy
# machine.
HOLDS_IT = ("import sys, time; sys.path.insert(0, %r); import chooser; "
            "assert chooser.alone(%r); print('held', flush=True); "
            "time.sleep(30)")


def module(request, name):
    """A fresh import of the real chooser, not a copy of it written here."""
    source = request.config.rootpath / "files/usr/local/lib/legion/chooser.py"
    loader = importlib.machinery.SourceFileLoader(name, str(source))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    fresh = importlib.util.module_from_spec(spec)
    loader.exec_module(fresh)
    return fresh


def already_up(request, runtime, name):
    """Another process holding the screen, by the door called `name`."""
    source = request.config.rootpath / "files/usr/local/lib/legion"
    child = subprocess.Popen(
        [sys.executable, "-c", HOLDS_IT % (str(source), name)],
        stdout=subprocess.PIPE, text=True,
        env={**os.environ, "XDG_RUNTIME_DIR": str(runtime)})
    assert child.stdout.readline().strip() == "held"
    return child


def test_no_chooser_takes_the_screen_from_itself(request, tmp_path, monkeypatch):
    """Two of them inside one process is a program asking twice, and there is
    nobody to ask to leave."""
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(tmp_path))
    first, second = module(request, "one"), module(request, "two")
    assert first.alone()
    assert not second.alone()


def test_the_one_that_holds_it_may_ask_twice(request, tmp_path, monkeypatch):
    """Asking is not taking. A chooser that checks again part way through
    would otherwise refuse itself."""
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(tmp_path))
    only = module(request, "only")
    assert only.alone()
    assert only.alone()


def test_the_door_that_opened_it_closes_it(request, tmp_path, monkeypatch):
    """The bar's speaker tapped twice: out, and away again. There is no B
    under a finger, so this is the whole of how a panel opened from the bar is
    put back."""
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(tmp_path))
    up = already_up(request, tmp_path, "settings Sound")
    assert not module(request, "same").alone("settings Sound")
    assert up.wait(timeout=5) != 0, "the panel that was up is still up"


def test_another_door_takes_its_place(request, tmp_path, monkeypatch):
    """The battery tapped while the sound is up is one panel showing the
    battery, not two panels or a tap that did nothing."""
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(tmp_path))
    up = already_up(request, tmp_path, "settings Sound")
    assert module(request, "other").alone("settings Battery")
    assert up.wait(timeout=5) != 0, "two panels are up at once"


def test_the_screen_is_taken_before_it_is_drawn_on(request, tmp_path, monkeypatch):
    """The one going hands the controller back on its way out, and the one
    arriving takes it. In the wrong order that leaves a panel on screen with
    the desktop's buttons under it. The lock is not free until the process
    holding it has ended, so waiting for the lock is waiting for the hand-back
    to have happened."""
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(tmp_path))
    up = already_up(request, tmp_path, "menu")
    assert module(request, "next").alone("settings ")
    assert up.poll() is not None, "it drew before the last one had gone"


def test_a_chooser_that_dies_does_not_keep_the_lock(request, tmp_path):
    """A lock outliving the process it was taken for is worse than the fault it
    fixes: a menu that cannot be opened again until the session ends. The kernel
    drops it when the process does, killed or not."""
    source = request.config.rootpath / "files/usr/local/lib/legion"
    script = ("import sys; sys.path.insert(0, %r); import chooser; "
              "assert chooser.alone(); print('held')" % str(source))
    for _ in range(2):
        done = subprocess.run(
            [sys.executable, "-c", script], capture_output=True, text=True,
            env={**os.environ, "XDG_RUNTIME_DIR": str(tmp_path)})
        assert done.stdout.strip() == "held", done.stderr


def test_every_chooser_asks_before_it_draws(request):
    """The lock is worth nothing if only some of them take it."""
    root = request.config.rootpath / "files/usr/local/bin"
    for name in ("launcher", "legion-buttons", "settings-panel"):
        text = (root / name).read_text()
        assert "import chooser" in text, "%s does not know about the lock" % name
        assert "chooser.alone(" in text, "%s never asks for it" % name
