"""What the daemon that reads the pad does when a button is pressed.

Each of these is the whole path: a button on the front of the machine, through
the profile that says what it means, onto the devices InputPlumber publishes,
into the daemon, and out as the command it runs or the wheel it turns. Nothing
between the two ends is stood in for, so a test that passes here is a statement
about the profile as much as about the daemon.
"""

from pathlib import Path

from evdev import ecodes as e


def test_the_top_right_paddle_closes_the_window(go, controller):
    go.press("right-paddle-top")
    controller.run(ticks=2)
    assert controller.ran.dispatched() == ["hl.dsp.window.close()"]


def test_the_top_left_paddle_opens_the_menu(go, controller):
    go.press("left-paddle-top")
    controller.run(ticks=2)
    assert controller.ran.names == ["launcher"]


def test_the_bottom_right_paddle_takes_a_screenshot(go, controller):
    go.press("right-paddle-bottom")
    controller.run(ticks=2)
    assert controller.ran.names == ["legion-screenshot"]


def test_the_menu_button_opens_the_settings(go, controller):
    """The settings are behind the button that is easiest to reach. The guide
    is read once and the settings are opened every day."""
    go.press("menu")
    controller.run(ticks=2)
    assert controller.ran.names == ["settings-panel"]


def test_legion_right_opens_the_guide(go, controller):
    go.press("legion-right")
    controller.run(ticks=2)
    assert controller.ran.commands == [["/usr/local/bin/legion-buttons", "--menu"]]


def test_view_goes_back_to_the_workspace_you_were_on(go, controller):
    go.press("view")
    controller.run(ticks=2)
    assert controller.ran.dispatched() == ['hl.dsp.focus({workspace = "previous"})']


def test_the_shoulders_move_between_workspaces(go, controller):
    go.press("r1")
    go.press("l1")
    controller.run(ticks=2)
    assert controller.ran.dispatched() == [
        'hl.dsp.focus({workspace = "+1"})',
        'hl.dsp.focus({workspace = "-1"})',
    ]


def test_holding_l2_carries_the_window_with_you(go, controller):
    go.trigger("l2", 1.0)
    go.press("r1")
    controller.run(ticks=2)
    assert controller.ran.dispatched() == ['hl.dsp.window.move({workspace = "+1"})']


def test_a_trigger_short_of_held_does_not_carry(go, controller):
    go.trigger("l2", 0.4)
    go.press("r1")
    controller.run(ticks=2)
    assert controller.ran.dispatched() == ['hl.dsp.focus({workspace = "+1"})']


def test_l2_and_the_dpad_are_the_brightness(go, controller):
    go.trigger("l2", 1.0)
    go.press("dpad-right")
    go.press("dpad-left")
    controller.run(ticks=2)
    assert controller.ran.commands == [
        ["/usr/local/bin/legion-brightness", "up"],
        ["/usr/local/bin/legion-brightness", "down"],
    ]


def test_the_dpad_alone_is_not_the_brightness(go, controller):
    """It is the arrow keys, which nothing here has to act on."""
    go.press("dpad-right")
    controller.run(ticks=2)
    assert controller.ran.commands == []


def test_the_right_stick_turns_the_wheel(go, controller):
    go.stick("right-stick", 0.0, -1.0)
    controller.run(ticks=11)
    wheel = controller.output.total(e.EV_REL, e.REL_WHEEL)
    assert wheel == 4, "a second of full deflection is a known number of notches"


def test_a_half_pushed_stick_scrolls_less_than_a_quarter_as_fast(go, controller):
    """Small pushes are squared, so precision at the top of the range costs
    nothing at the bottom."""
    go.stick("right-stick", 0.0, -0.6)
    controller.run(ticks=11)
    assert controller.output.total(e.EV_REL, e.REL_WHEEL) == 1


def test_inside_the_deadzone_the_page_stays_where_it_is(go, controller):
    go.stick("right-stick", 0.0, -0.15)
    controller.run(ticks=20)
    assert controller.output.of_type(e.EV_REL, e.REL_WHEEL) == []


def test_pushing_up_scrolls_up(go, controller):
    go.stick("right-stick", 0.0, -1.0)
    controller.run(ticks=11)
    assert controller.output.total(e.EV_REL, e.REL_WHEEL) > 0


def test_pushing_down_scrolls_down(go, controller):
    go.stick("right-stick", 0.0, 1.0)
    controller.run(ticks=11)
    assert controller.output.total(e.EV_REL, e.REL_WHEEL) < 0


def test_a_finger_on_the_pad_moves_the_pointer(go, controller):
    go.drag((200, 200), (400, 200), steps=4)
    controller.run(ticks=2)
    moved_x = controller.output.total(e.EV_REL, e.REL_X)
    moved_y = controller.output.total(e.EV_REL, e.REL_Y)
    assert moved_x > 0 and moved_y == 0
    assert moved_x == 280, "1.4 screen pixels for each unit the finger travelled"


def test_the_pointer_does_not_jump_to_where_the_finger_landed(go, controller):
    """Position in, movement out. The first report of a touch is where it
    started, and starting somewhere is not moving."""
    go.touch_down(900, 900)
    go.touch_up()
    controller.run(ticks=2)
    assert controller.output.of_type(e.EV_REL, e.REL_X) == []


def test_a_quick_touch_is_a_click(go, controller):
    go.tap(500, 500)
    controller.run(ticks=2)
    assert controller.output.of_type(e.EV_KEY, e.BTN_LEFT) == [
        (e.EV_KEY, e.BTN_LEFT, 1),
        (e.EV_KEY, e.BTN_LEFT, 0),
    ]


def test_a_drag_across_the_pad_is_not_a_click(go, controller):
    go.drag((100, 100), (900, 900), steps=8)
    controller.run(ticks=2)
    assert controller.output.of_type(e.EV_KEY, e.BTN_LEFT) == []


def test_pressing_the_pad_in_holds_the_button_down(go, controller):
    """Not a tap. The button stays down for as long as the pad is pressed, so
    a window can be dragged with it."""
    go.touch_click(1)
    controller.run(ticks=4, script={2: lambda: go.touch_click(0)})
    assert controller.output.of_type(e.EV_KEY, e.BTN_LEFT) == [
        (e.EV_KEY, e.BTN_LEFT, 1),
        (e.EV_KEY, e.BTN_LEFT, 0),
    ]


def test_the_pad_going_away_does_not_take_the_daemon_with_it(go, world, controller):
    """A profile switch destroys the virtual pad and builds another. Reading
    from what was left used to end this process, and the workspace buttons
    went with it."""
    controller.run(ticks=6, script={
        1: world.devices["pad"].unplug,
        2: lambda: go.press("left-paddle-top"),
    })
    assert controller.ran.names == ["launcher"], "the keyboard side kept working"


def test_the_pad_is_picked_up_again_when_it_comes_back(go, world, controller):
    """Which is what happens every time a menu opens and closes."""
    pad = world.devices["pad"]
    controller.run(ticks=200, script={
        1: pad.unplug,
        60: pad.plug,
        90: lambda: go.press("r1"),
    })
    assert controller.ran.dispatched() == ['hl.dsp.focus({workspace = "+1"})']


def test_the_keyboard_stops_the_daemon_and_nothing_the_daemon_started():
    """The on-screen keyboard takes the pad by signalling this daemon's unit.
    A signal to a unit reaches every process in its control group unless it is
    told otherwise, and the menu, the panel and anything opened from the menu
    are all in that group: a control group is inherited by every child and
    nothing a program can do to itself leaves one. Named wrongly, raising the
    keyboard over a panel stopped the panel."""
    hook = Path(__file__).resolve().parent.parent / "files/usr/local/bin/osk-hook"
    lines = [line for line in hook.read_text().splitlines()
             if "systemctl" in line and "kill" in line and not line.startswith("#")]
    assert lines, "osk-hook no longer signals the daemon at all"
    for line in lines:
        assert "--kill-whom=main" in line, \
            "osk-hook signals the whole control group: %s" % line.strip()
