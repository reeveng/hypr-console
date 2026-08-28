"""The promises the front of the machine makes.

A person holding this thing learns four buttons once and then stops thinking
about them. That only holds if the answer is the same in every program, and
what a button means is decided in four separate files, so it is checked here
rather than remembered:

    D-pad   moves between things: options, windows, pages. Never does anything.
    A       accepts. Whatever is highlighted, that one.
    B       goes back: cancels, closes, and deletes in the keyboard.
    X       shows the keyboard, and hides it again, wherever you are.
    Y       is not spoken for.

The fifth rule is not about a person at all. An event can only reach a device
the profile lists in `target_devices`, because InputPlumber builds what a
profile names and destroys the rest, so a mapping that sends a pad button from
a profile with no pad in it is a button that does nothing.
"""

import pytest
from evdev import ecodes as e

from emulator import vocabulary
from emulator.profile import Target, load_all
from emulator.targets import descriptors
from harness.daemon import Daemon

# Moving between things, and nothing else.
NAVIGATION = {e.KEY_UP, e.KEY_DOWN, e.KEY_LEFT, e.KEY_RIGHT,
              e.KEY_PAGEUP, e.KEY_PAGEDOWN, e.KEY_HOME, e.KEY_END, e.KEY_TAB}

DPAD = ["dpad-up", "dpad-down", "dpad-left", "dpad-right"]

# What A accepting looks like, which depends on what is on screen. A chooser is
# driven by the highlight, so A is Enter there. On the desktop there is no
# highlight to confirm, and accepting is clicking what the pointer is on.
ACCEPTS = {Target("key", "KeyEnter"), Target("mouse-button", "Left")}

BACK = {Target("key", "KeyEsc")}

# The signal the on-screen keyboard watches for. It reads the pad itself, so
# what X has to do is arrive on the pad as North, whatever the profile.
KEYBOARD_TOGGLE = Target("gamepad-button", "North")


@pytest.fixture(scope="module")
def profiles(request):
    return load_all(request.config.rootpath)


def mapped(profiles):
    """The profiles that map anything.

    The keyboard profile maps nothing on purpose: while the on-screen keyboard
    is up it reads the pad itself, and anything translated here would happen
    twice. There is nothing in it for these rules to be about.
    """
    return {name: p for name, p in profiles.items() if p.mappings}


def test_there_is_a_profile_for_each_word_controller_profile_takes(profiles):
    assert set(profiles) == {"desktop", "keyboard", "menu", "tabs"}


def test_a_accepts_everywhere(profiles):
    for name, profile in mapped(profiles).items():
        targets = set(profile.targets_of("a"))
        assert targets, "%s: A does nothing" % name
        assert targets <= ACCEPTS, "%s: A does more than accept: %s" % (name, targets)


def test_b_goes_back_everywhere(profiles):
    for name, profile in mapped(profiles).items():
        targets = set(profile.targets_of("b"))
        assert targets == BACK, "%s: B is %s" % (name, targets or "nothing")


def test_x_shows_and_hides_the_keyboard_everywhere(profiles):
    for name, profile in mapped(profiles).items():
        targets = profile.targets_of("x")
        assert KEYBOARD_TOGGLE in targets, \
            "%s: X does not reach the keyboard: %s" % (name, targets)


def test_the_keyboard_profile_passes_everything_through(profiles):
    """Which is how X still closes the keyboard that X opened."""
    assert profiles["keyboard"].mappings == []


def test_the_dpad_only_moves_between_things(profiles):
    for name, profile in mapped(profiles).items():
        for button in DPAD:
            for target in profile.targets_of(button):
                assert target.kind == "key", \
                    "%s: %s does something: %s" % (name, button, target)
                assert target.code in NAVIGATION, \
                    "%s: %s sends %s, which is not moving between things" \
                    % (name, button, target.name)


def test_the_dpad_moves_up_and_down_wherever_there_is_a_list(profiles):
    for name, profile in mapped(profiles).items():
        for button in ("dpad-up", "dpad-down"):
            assert profile.targets_of(button), "%s: %s does nothing" % (name, button)


def test_y_is_not_spoken_for(profiles):
    """It has no job that a person has to learn, so nothing may quietly give
    it one that another rule already owns."""
    for name, profile in mapped(profiles).items():
        targets = set(profile.targets_of("y"))
        assert not targets & (ACCEPTS | BACK), \
            "%s: Y has taken a job that belongs to A or B" % name
        assert KEYBOARD_TOGGLE not in targets, \
            "%s: Y has taken the keyboard, which is X's" % name


def test_nothing_is_sent_to_a_device_the_profile_does_not_publish(profiles):
    """InputPlumber builds the targets a profile names and destroys the rest.
    A mapping onto a device this profile has not asked for goes nowhere, and
    goes nowhere silently."""
    needs = {"key": "keyboard", "mouse-button": "mouse", "mouse-motion": "mouse",
             "gamepad-button": "xbox-elite", "gamepad-axis": "xbox-elite",
             "gamepad-trigger": "xbox-elite"}
    for name, profile in profiles.items():
        for mapping in profile.mappings:
            for target in mapping.targets:
                wanted = needs[target.kind]
                assert wanted in profile.target_devices, \
                    "%s: %r sends to %s, which %s does not publish" \
                    % (name, mapping.label, wanted, name)


def test_every_profile_publishes_the_same_devices(profiles):
    """Switching profiles must not destroy one and build it again. The
    compositor does not deliver anything from a keyboard that appeared after
    it started: the device is there, it is listed, and every key it sends is
    dropped. That is what made the on-screen keyboard break over and over."""
    for name, profile in profiles.items():
        assert set(profile.target_devices) == {"mouse", "keyboard", "xbox-elite"}, \
            "%s publishes %s" % (name, profile.target_devices)


def test_every_mapping_says_what_it_does(profiles):
    """The guide is generated from these names, so a mapping that says nothing
    is a button nobody can look up."""
    for name, profile in profiles.items():
        for mapping in profile.mappings:
            assert " - " in mapping.label, \
                "%s: %r is not \"Button - what it does\"" % (name, mapping.label)
            assert mapping.does, "%s: %r does not finish the sentence" \
                % (name, mapping.label)


def test_every_key_a_profile_sends_is_a_key_the_keyboard_has(profiles):
    """The keyboard InputPlumber publishes carries a fixed set of keys. One it
    does not carry cannot be sent, however the profile spells it."""
    has = set(descriptors()["keyboard"]["capabilities"]["EV_KEY"])
    for name, profile in profiles.items():
        for mapping in profile.mappings:
            for target in mapping.targets:
                if target.kind == "key":
                    assert target.code in has, \
                        "%s: %r sends %s, which is not on the keyboard" \
                        % (name, mapping.label, target.name)


def test_every_button_the_desktop_acts_on_is_named_in_a_chooser(profiles):
    """The chooser profiles publish a pad, because the on-screen keyboard reads
    one and X has to reach it. A pad means every button arrives at one, so any
    button the controller daemon acts on has to be named here, either given a
    job or given none. Left out, it depends on whether InputPlumber passes an
    unmapped button through, which is not written down anywhere and is not
    worth a desktop resting on.

    The daemon's own tables are read for the answer, so a button given a job
    there and forgotten here is a failure rather than a surprise.
    """
    daemon = Daemon("stick-scroll")
    acts_on = set(daemon.module.BUTTONS) | set(daemon.module.SHOULDERS)
    for name in ("menu", "tabs"):
        named = {vocabulary.GAMEPAD_CODES[m.source_name]
                 for m in profiles[name].mappings
                 if m.source_kind == "button"
                 and m.source_name in vocabulary.GAMEPAD_CODES}
        forgotten = sorted(vocabulary.spoken_for(n)
                           for c in acts_on - named
                           for n, code in vocabulary.GAMEPAD_CODES.items()
                           if code == c for n in [n])
        assert not forgotten, \
            "%s: %s would reach the desktop behind an open chooser" \
            % (name, ", ".join(forgotten))


def test_a_button_with_nothing_to_do_here_says_so(profiles):
    """Named and given nothing, which means the same thing whether an unmapped
    button is passed through or dropped."""
    for name in ("menu", "tabs"):
        silent = {mapping.button for mapping in profiles[name].mappings
                  if not mapping.targets}
        assert {"menu", "view", "legion-left"} <= silent, \
            "%s: %s" % (name, sorted(silent))
    assert {"l1", "r1"} <= {m.button for m in profiles["menu"].mappings
                            if not m.targets}, \
        "menu: the shoulders still change workspace behind the chooser"


def test_the_shoulders_move_between_pages_where_there_are_pages(profiles):
    for button, key in (("l1", e.KEY_PAGEUP), ("r1", e.KEY_PAGEDOWN)):
        targets = profiles["tabs"].targets_of(button)
        assert [t.code for t in targets] == [key], \
            "tabs: %s is %s" % (button, targets)


def test_a_chooser_leaves_no_button_to_chance(profiles):
    """Every button the desktop names, a chooser names too.

    InputPlumber passes a source it was told nothing about straight through to
    whatever pad the profile publishes. So a button left out of a chooser's
    profile does not stop working: it arrives as the pad's own button, on a
    device nothing is reading it from, and does nothing for a reason nobody can
    see from either file. Three of the four paddles were in that state, showing
    up in the journal as BTN_TRIGGER_HAPPY while a chooser was up and answered
    by no one.

    Named and sent nowhere is a decision. Left out is an accident that reads
    the same.
    """
    named = {name for name in vocabulary.BUTTONS
             if profiles["desktop"].for_button(name)}
    for where in ("menu", "tabs"):
        missing = sorted(n for n in named if not profiles[where].for_button(n))
        assert not missing, "%s says nothing about %s" % (where, ", ".join(missing))
