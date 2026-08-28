"""The manifest says what this desktop is. These are the ways it can lie.

Everything here is a thing `legion apply` would happily do, and a person would
then find out about at the wrong moment: a file listed with nothing behind it,
a file kept in the tree that is never installed anywhere, a script that will
not parse, a service that starts a program the manifest does not carry.

The manifest is read with the machine's own reader rather than a second one
written here, so a change to how it is read is a change to what is checked.
"""

import ast
import importlib.machinery
import importlib.util
import json
import re
import subprocess
from pathlib import Path

import pytest
import yaml


@pytest.fixture(scope="module")
def engine(request):
    """`legion` itself, pointed at the checkout instead of at /etc/legion."""
    root = request.config.rootpath
    path = root / "files/usr/local/bin/legion"
    loader = importlib.machinery.SourceFileLoader("legion_engine", str(path))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    module.ROOT = root
    module.SOURCE = root / "files"
    module.MANIFEST = root / "desktop.conf"
    return module


@pytest.fixture(scope="module")
def manifest(engine):
    return engine.read_manifest()


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


def test_the_manifest_has_the_sections_the_engine_reads(manifest):
    assert {"packages", "files", "services", "masked"} <= set(manifest)
    assert set(manifest) <= {"packages", "files", "services", "masked",
                             "elsewhere"}


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


# The engine is the one thing the manifest does not list. It is what installs
# every other file, and a program that copies over the file it is being read
# from is a program that can be halfway through itself when it changes.
INSTALLS_THE_REST = "/usr/local/bin/legion"


def test_every_file_in_the_tree_is_listed(manifest, source):
    """A file nobody lists is a file `legion apply` never installs. It reads
    as part of the desktop and is not part of it."""
    listed = set(manifest["files"]) | {INSTALLS_THE_REST}
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


def test_everything_meant_to_be_run_will_be_installed_able_to_run(engine, source):
    for path in carried(source):
        file = source / path.lstrip("/")
        head = file.read_bytes()[:4]
        if head[:2] == b"#!" or head == b"\x7fELF":
            assert engine.mode_of(file) == 0o755, \
                "%s is a program and would be installed unrunnable" % path


def test_files_in_the_user_s_home_are_installed_as_the_user(engine, manifest):
    for path in manifest["files"]:
        expected = "player" if path.startswith("/home/player/") else "root"
        assert engine.owner_of(Path(path)) == expected


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
