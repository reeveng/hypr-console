"""The manifest says what this desktop is. These are the ways it can lie.

Everything here is a thing `legion apply` would happily do, and a person would
then find out about at the wrong moment: a file listed with nothing behind it,
a file kept in the tree that is never installed anywhere, a script that will
not parse, a service that starts a program the manifest does not carry.

What the engine does with the manifest is tested beside the engine, in
`crates/legion-manifest`. What is tested here is the manifest against the tree.
"""

import ast
import json
import re
import subprocess
from pathlib import Path

import pytest
import yaml


@pytest.fixture(scope="module")
def manifest(request):
    """`desktop.conf` as {section: [entry]}.

    Read here rather than asked of the engine, which is a compiled program now.
    What the engine does with the manifest is tested beside the engine; what is
    tested from here is the manifest against the tree.
    """
    sections, current = {}, None
    for line in (request.config.rootpath / "desktop.conf").read_text().splitlines():
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            current = line[1:-1]
            sections.setdefault(current, [])
        elif current:
            sections[current].append(line)
    return sections


@pytest.fixture(scope="module")
def source(request):
    return request.config.rootpath / "files"


def carried(source):
    """Every file in the tree, as the path it is installed to.

    Bytecode is not one of them. Python writes it beside whatever it
    imports, git is already told to ignore it, and this tree is worked in
    by more than one person at once: a stray .pyc from somebody else's
    test run is not a desktop file nobody installs.
    """
    return sorted("/" + str(p.relative_to(source))
                  for p in source.rglob("*")
                  if p.is_file() and "__pycache__" not in p.parts)


def carried_or_declared(manifest):
    """Everything this desktop is allowed to reach for.

    A program the manifest does not carry is normally a mistake: apply installs
    half of a working pair and the missing half is found later, by somebody
    holding the device. `[elsewhere]` is how the other case is said out loud.
    It exists for a program that is somebody else's to publish, which is why
    the public copy of this repository has one and this does not.
    """
    return set(manifest["files"]) | set(manifest.get("elsewhere", []))


def test_every_file_the_manifest_lists_is_in_the_tree(manifest, source):
    for path in manifest["files"]:
        assert (source / path.lstrip("/")).is_file(), \
            "%s is listed and there is nothing behind it" % path


def test_every_file_in_the_tree_is_listed(manifest, source):
    """A file nobody lists is a file `legion apply` never installs. It reads
    as part of the desktop and is not part of it."""
    listed = set(manifest["files"])
    for path in carried(source):
        assert path in listed, "%s is in the tree and nothing installs it" % path


def test_every_service_has_a_unit_the_manifest_carries(manifest):
    listed = set(manifest["files"])
    for service in manifest["services"]:
        assert "/etc/systemd/user/" + service in listed, \
            "%s is enabled and its unit is not carried" % service


def test_the_target_pulls_in_exactly_the_services_that_are_enabled(manifest, source):
    """The target is what the compositor starts, and the only thing it starts.
    A service enabled but not wanted by it never runs; one wanted by it and not
    enabled is a unit systemd will not have."""
    wanted = set()
    for service in manifest["services"]:
        unit = (source / "etc/systemd/user" / service).read_text()
        if re.search(r"^WantedBy=legion\.target$", unit, re.M):
            wanted.add(service)
    assert wanted == set(manifest["services"])


def test_every_program_a_unit_starts_is_carried(manifest, source):
    listed = carried_or_declared(manifest)
    for unit in sorted(p for p in (source / "etc/systemd/user").glob("*")
                       if p.is_file()):
        for command in re.findall(r"^Exec\w+=-?(\S+)", unit.read_text(), re.M):
            if command.startswith("/usr/local/"):
                assert command in listed, \
                    "%s starts %s, which is not carried" % (unit.name, command)


def test_the_paper_service_throws_away_the_frames_of_the_last_background(source):
    """awww names its cache after the picture's path and not after anything
    inside the file, so a redrawn background at the same path is played as the
    old one's frames over the new one's still. That is a screen full of blocks
    of two pictures mixed, and the only thing that was ever wrong with it was a
    file nobody thought to delete."""
    unit = (source / "etc/systemd/user/legion-paper.service").read_text()
    assert "ExecStartPre=-/usr/bin/rm -rf %h/.cache/awww" in unit


def test_every_program_a_carried_script_reaches_for_is_carried(manifest, source):
    """A script that calls another by its full path is a dependency the
    manifest has to know about, or apply installs half of a working pair."""
    listed = carried_or_declared(manifest)
    for path in sorted(p for p in (source / "usr/local/bin").glob("*")
                       if p.is_file()):
        try:
            text = path.read_text()
        except UnicodeDecodeError:
            continue          # a compiled program, which reaches for nothing
        for command in set(re.findall(r"/usr/local/bin/[\w-]+", text)):
            assert command in listed, \
                "%s runs %s, which is not carried" % (path.name, command)


def test_every_python_script_parses(source):
    for path in carried(source):
        file = source / path.lstrip("/")
        try:
            text = file.read_text()
        except UnicodeDecodeError:
            continue
        if text.startswith("#!") and "python" in text.splitlines()[0]:
            ast.parse(text, filename=str(file))


def test_every_shell_script_parses(source):
    for path in carried(source):
        file = source / path.lstrip("/")
        try:
            text = file.read_text()
        except UnicodeDecodeError:
            continue
        first = text.splitlines()[0] if text else ""
        if first.startswith("#!") and ("/sh" in first or "bash" in first):
            done = subprocess.run(["sh", "-n", str(file)], capture_output=True,
                                  text=True)
            assert done.returncode == 0, "%s: %s" % (path, done.stderr.strip())


def test_every_yaml_file_parses(source):
    for file in sorted(source.rglob("*.yaml")):
        yaml.safe_load(file.read_text())


def test_every_json_file_parses(source):
    for file in sorted(source.rglob("*.json")):
        json.loads(file.read_text())


# Programs the bar may reach for that come from a package rather than the tree.
OUTSIDE = {"wpctl", "activate"}


def bar_commands(source):
    """What every on-click in the bar runs, as (module, first word, argument)."""
    config = source / "home/player/.config/waybar/config.jsonc"
    text = re.sub(r"^\s*//.*$", "", config.read_text(), flags=re.M)
    for module, about in json.loads(text).items():
        if not isinstance(about, dict):
            continue
        for key, command in about.items():
            if not key.startswith("on-"):
                continue
            words = command.split()
            yield module, words[0], words[1] if len(words) > 1 else ""


def test_every_program_the_bar_runs_is_carried(manifest, source):
    """The bar is the one place a program is named where nothing will complain
    if it is gone: the button simply does nothing, and a person decides the
    machine is broken. A script that gets renamed has to be renamed here too."""
    listed = carried_or_declared(manifest)
    for module, command, _ in bar_commands(source):
        if command in OUTSIDE:
            continue
        assert "/usr/local/bin/" + command in listed, \
            "the bar's %s runs %s, which is not carried" % (module, command)


def tabs_of(source):
    """The words on the settings panel's tabs, read out of the panel itself."""
    tree = ast.parse((source / "usr/local/bin/settings-panel").read_text())
    for node in tree.body:
        if not isinstance(node, ast.FunctionDef) or node.name != "pages":
            continue
        for inner in ast.walk(node):
            if isinstance(inner, ast.Return) and isinstance(inner.value, ast.List):
                return {page.elts[0].value for page in inner.value.elts}
    return set()


def test_every_tab_the_bar_asks_for_exists(source):
    """The bar opens the panel at the tab that stands for the thing tapped. A
    name nothing answers to opens the first tab, which is a wrong place rather
    than an error, so it has to be caught here."""
    tabs = tabs_of(source)
    assert tabs, "no tabs found in the settings panel"
    for module, command, argument in bar_commands(source):
        if command != "settings-panel" or not argument:
            continue
        assert argument in tabs, \
            "the bar's %s opens the %s tab, which does not exist" % (module, argument)


# ---------------------------------------------------------------- by finger

# The screen is a touchscreen, and the device is put down as often as it is
# held. Everything below is something a hand with no controller in it could
# not do at all until it was there, so each of these is a way back to that.


def test_the_bar_has_a_door_for_the_menu_and_for_the_keyboard(source):
    """The two things a finger has no other road to. Every other button on the
    pad has an icon on the bar or a row in a panel; these two had neither, so
    a person holding nothing could not open an application or type a letter."""
    runs = {command for _, command, _ in bar_commands(source)}
    assert "launcher" in runs, "there is no way to open the menu by hand"
    assert "osk" in runs, "there is no way to ask for the keyboard by hand"


def panel_names(source):
    """Every name the panel gives a widget, which is how it is styled and,
    for these, whether it exists at all."""
    tree = ast.parse((source / "usr/local/lib/legion/panel.py").read_text())
    return {call.args[0].value for call in ast.walk(tree)
            if isinstance(call, ast.Call)
            and isinstance(call.func, ast.Attribute)
            and call.func.attr == "set_name"
            and call.args and isinstance(call.args[0], ast.Constant)}


def test_a_panel_can_be_closed_without_a_button(source):
    """B closes a panel and a finger has no B. Four of the bar's icons open
    one, so without the mark on the strip a tap could put a panel on the
    screen that only the controller could take off again."""
    assert "shut" in panel_names(source), "a panel has no way out but a button"


def test_a_level_draws_the_two_ends_of_itself(source):
    """A level is the one thing on a panel that left and right do and a tap
    cannot: tapping the row it is on silences it. Without these the volume is
    a reading a person can look at and not change."""
    tree = ast.parse((source / "usr/local/lib/legion/panel.py").read_text())
    line = next(node for node in ast.walk(tree)
                if isinstance(node, ast.FunctionDef) and node.name == "line")
    assert "level" in [a.arg for a in line.args.args], \
        "a row is drawn without knowing whether it carries a level"
    marks = {node.value for node in ast.walk(line)
             if isinstance(node, ast.Constant) and node.value in ("+", "−")}
    assert marks == {"+", "−"}, "a level row draws no steps: %s" % marks
    assert "step" in panel_names(source)


def levels_in(source, program):
    """Every row of a panel that carries a level, by the word it opens with."""
    tree = ast.parse((source / "usr/local/bin" / program).read_text())
    return {node.elts[0].value for node in ast.walk(tree)
            if isinstance(node, ast.Tuple) and len(node.elts) == 4
            and isinstance(node.elts[0], ast.Constant)}


def test_the_two_things_that_are_held_at_a_level_are_on_a_panel(source):
    """Sound was on a panel and the screen was not: brightness lived on the
    d-pad held under L2 and nowhere else, which is two buttons at once for the
    setting a person changes when the room gets dark."""
    assert {"Screen", "Speakers"} <= levels_in(source, "settings-panel")


def test_something_answers_when_a_password_is_asked_for(manifest, source):
    """polkitd asks the session for a password and gives up if nothing
    answers. With no agent running, installing something is not a refusal, it
    is a button that does nothing and says nothing about why."""
    assert "legion-polkit.service" in manifest["services"]
    unit = (source / "etc/systemd/user/legion-polkit.service").read_text()
    starts = re.search(r"^ExecStart=(\S+)", unit, re.M).group(1)
    assert "polkit" in starts, "the polkit service starts %s" % starts
