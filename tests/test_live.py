"""The same daemon, against devices the kernel really made.

These need to be able to make an input device, which means /dev/uinput. They
are skipped where that is not open rather than failing, because everything they
prove about what the daemon decides is proved in the fast tier too. What only
these can prove is that the emulator's devices are the ones the daemon goes
looking for, and that what it writes is a real pointer moving.

Nothing here reaches the desktop this is run on: the daemon's device is grabbed
as soon as it appears.
"""

import pytest
from evdev import ecodes as e

from harness.live import READS, Running, uinput_is_open

pytestmark = pytest.mark.skipif(
    not uinput_is_open(),
    reason="no way in to /dev/uinput; see docs/emulator.md")


@pytest.fixture
def live():
    running = Running()
    if running.out is None:
        running.close()
        pytest.fail("the daemon never published a device: %s" % running.said)
    yield running
    running.close()


def test_the_daemon_finds_all_three_devices(live):
    for wanted in READS:
        assert wanted in live.said, "it did not say it had found %s" % wanted


def test_the_right_stick_really_turns_a_wheel(live):
    live.go.stick("right-stick", 0.0, -1.0)
    turned = live.total(e.EV_REL, e.REL_WHEEL, seconds=1.0)
    live.go.centre("right-stick")
    assert turned > 0


def test_a_finger_on_the_pad_really_moves_a_pointer(live):
    live.go.drag((200, 300), (500, 300), steps=6, seconds=0.12)
    moved = [(c, v) for t, c, v in live.events(0.4) if t == e.EV_REL]
    assert sum(v for c, v in moved if c == e.REL_X) > 0
    assert all(c in (e.REL_X, e.REL_Y) for c, _ in moved)


def test_a_tap_is_really_a_click(live):
    live.go.tap(500, 500)
    assert [(c, v) for t, c, v in live.events(0.4) if t == e.EV_KEY] == [
        (e.BTN_LEFT, 1), (e.BTN_LEFT, 0)]


def test_a_shoulder_really_reaches_the_compositor(live):
    live.go.press("r1")
    live.settle()
    assert live.commands == [
        ["hyprctl", "dispatch", 'hl.dsp.focus({workspace = "+1"})']]


def test_a_paddle_really_opens_the_menu(live):
    live.go.press("left-paddle-top")
    live.settle()
    assert live.names == ["launcher"]


def test_the_emulator_publishes_what_the_capture_says(live):
    """The one thing the fast tier cannot check: that a device built from the
    capture is the device the daemon is looking for, down to the axes."""
    pad = live.devices.devices["pad"].device
    axes = dict(pad.capabilities(absinfo=True)[e.EV_ABS])
    assert axes[e.ABS_RX].min == -32768 and axes[e.ABS_RX].max == 32767
    assert pad.phys == "", "a pad with a physical location is a real one"
