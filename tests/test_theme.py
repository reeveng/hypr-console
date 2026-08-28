"""The palette says what this desktop looks like. These are the ways it can lie.

Three things are checked, and the middle one is the reason the other two are
here. Colours can be wrong by being unreadable, which is what the ratios are
for. They can be wrong by having been changed in one file and not in another,
which is what the drift check is for. And the engine that computes both can
itself be wrong, which is what the vectors at the bottom are for: they were
produced by a different implementation in a different language, and if this one
ever stops agreeing with them then every number in the report is a number
nobody should trust.
"""

import importlib.machinery
import importlib.util
import re
import subprocess
import tomllib

import pytest

# Every way a colour is written down on this machine. A stylesheet says
# `#rrggbb`, a terminal says `0xrrggbb`, the compositor says `rgba(rrggbbaa)`,
# a shell variable says the digits bare, and KDE says three decimal numbers.
# Anchoring the last two to an assignment keeps a font size out of it: KDE
# writes `font=Noto Sans,16,-1,5,400,0,0` in the same file as its colours.
COLOUR = re.compile(
    r"#([0-9a-fA-F]{6})\b"
    r"|0x([0-9a-fA-F]{6})\b"
    r"|rgba\(([0-9a-fA-F]{6})ff\)"
    r"|^\w+=([0-9a-fA-F]{6})$"
    r"|^\w+=(\d{1,3},\s?\d{1,3},\s?\d{1,3})$", re.M)


def forms(palette):
    """A declared colour as any of the ways it may be written."""
    said = set()
    for code in palette.values():
        said.add(code.lower())
        red, green, blue = (int(code[i:i + 2], 16) for i in (0, 2, 4))
        said.add(f"{red},{green},{blue}")
    return said

@pytest.fixture(scope="module")
def theme(request):
    """`legion-theme` itself, loaded rather than run, so its pieces can be asked."""
    path = request.config.rootpath / "tools/legion-theme"
    loader = importlib.machinery.SourceFileLoader("legion_theme", str(path))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def declared(request):
    return tomllib.loads((request.config.rootpath / "theme/palette.toml").read_text())


@pytest.fixture(scope="module")
def palette(theme, declared):
    return theme.resolve(declared["colour"])


def test_every_pairing_clears_what_it_declares(theme, declared, palette):
    """The whole promise, in one assertion.

    Everything that is read clears AAA, an edge clears the 3:1 an edge needs,
    and the one colour under AAA is what a terminal means by black, which is
    declared at AA on purpose and says so.
    """
    for front, back, asked, got, _kind, where in theme.measure(declared, palette):
        assert got >= asked, f"{front} on {back} is {got:.2f}:1, wanted {asked}:1 ({where})"


def test_text_is_aaa_everywhere(theme, declared, palette):
    for front, back, asked, got, kind, where in theme.measure(declared, palette):
        if kind == "text":
            assert got >= 7.0, f"{front} on {back} is {got:.2f}:1, which is not AAA ({where})"


def test_the_quiet_one_is_still_aa(theme, declared, palette):
    """One colour is allowed under AAA, it is named, and it still clears AA."""
    quiet = [row for row in theme.measure(declared, palette) if row[4] == "quiet"]
    assert [row[0] for row in quiet] == ["ash"]
    for front, back, asked, got, _kind, where in quiet:
        assert got >= 4.5, f"{front} on {back} is {got:.2f}:1 ({where})"


def test_nothing_anywhere_is_under_aa(theme, declared, palette):
    """No two colours that meet are allowed under 4.5:1 unless they are grounds.

    A ground under another ground is a step you see rather than read, and the
    only pair on this machine allowed to be quiet.
    """
    grounds = {"night", "ground", "panel"}
    for front, back, asked, got, kind, where in theme.measure(declared, palette):
        if kind in ("text", "quiet"):
            assert got >= 4.5, f"{front} on {back} is {got:.2f}:1 ({where})"
        assert front in grounds or back in grounds or got >= 3.0


def test_the_files_say_what_the_palette_says(request):
    """Nothing has been edited in place since the palette was last spent."""
    done = subprocess.run(["python3", "tools/legion-theme", "--check"],
                          cwd=request.config.rootpath, capture_output=True, text=True)
    assert done.returncode == 0, (
        "a themed file no longer matches theme/palette.toml. Run `make theme`.\n"
        + done.stdout + done.stderr)


def carrying(root):
    """Every file under files/ that a person could have typed a colour into."""
    for path in sorted(root.glob("**/*")):
        if not path.is_file() or "__pycache__" in path.parts:
            continue
        try:
            yield path, path.read_text()
        except UnicodeDecodeError:
            continue           # the keyboard and hyprsession are compiled programs


def test_no_file_anywhere_carries_a_colour_from_outside_the_palette(theme, declared, palette, request):
    """Every hex installed on the machine is one the palette declares.

    Not only the files the generator writes: the whole tree, so that a colour
    typed in by hand is caught wherever somebody types it. That is the drift
    this was built to stop. A hex put in by hand is invisible until somebody
    looks at the screen in the right light, and by then it has been there for
    months.
    """
    known = forms(palette)
    known |= forms({slot: theme.lift(code, declared["terminal"]["bright_lift"])
                    for slot, code in palette.items()})
    for path, text in carrying(request.config.rootpath / "files"):
        for match in COLOUR.finditer(text):
            found = next(group for group in match.groups() if group)
            found = found.lower().replace(" ", "")
            assert found in known, (
                f"{path.relative_to(request.config.rootpath)} carries #{found}, "
                "which is not a colour theme/palette.toml declares")


def test_only_the_palette_holds_a_colour(request):
    """And the rest of the desktop imports it.

    A stylesheet, a terminal, a keyboard and a browser can each import a file
    written in their own language, so each of them does, and the hex lives in
    one place per language rather than in every file that spends it. The ones
    that cannot import anything are KDE's ini format, which has no include, a
    user.js, which is a list of literals, the compositor, whose config is
    written rather than imported because a Lua file that fails to load takes
    the session with it, and a picture.
    """
    allowed = {
        "home/player/.config/hypr/hyprland.lua",
        "home/player/.config/kdeglobals",
        "home/player/.config/legion/palette.css",
        "home/player/.config/legion/palette.toml",
        "home/player/.mozilla/firefox/legion/chrome/palette.css",
        "home/player/.mozilla/firefox/legion/user.js",
        "usr/local/lib/legion/palette.sh",
        "usr/share/icons/legion-placeholder.svg",
    }
    root = request.config.rootpath / "files"
    holding = {str(path.relative_to(root)) for path, text in carrying(root) if COLOUR.search(text)}
    assert holding == allowed, (
        "a file outside the palette has grown a colour, or one inside it has lost "
        f"the only colour it had: {sorted(holding ^ allowed)}")


def test_every_colour_is_spent(palette, request):
    """A colour nothing uses is a colour nobody maintains."""
    written = "".join(text for _path, text in carrying(request.config.rootpath / "files"))
    for name, code in palette.items():
        assert code.lower() in written.lower(), f"{name} (#{code}) is declared and never used"


def test_every_colour_says_what_it_is_for(declared):
    for name, spec in declared["colour"].items():
        assert spec.get("spent"), f"{name} does not say what it is spent on"


# Produced by Codincod.Design.Oklch, which is the same arithmetic written
# independently in Elixir for the site's themes and checked by its own tests.
# Two implementations agreeing on a colour and a ratio is worth more than one
# implementation agreeing with itself, and these are the numbers that agreement
# was recorded at.
VECTORS = [
    ((0.125, 0.014, 318), "08050a", 1.3119),
    ((0.215, 0.020, 318), "1d1720", 1.1372),
    ((0.290, 0.026, 318), "312734", 1.0814),
    ((0.480, 0.030, 318), "655969", 2.3433),
    ((0.560, 0.038, 318), "7e6e83", 3.2692),
    ((0.860, 0.022, 335), "dbccd7", 10.0245),
    ((0.760, 0.038, 332), "c0a9bc", 7.0917),
    ((0.855, 0.105, 342), "ffb5e2", 9.5312),
    ((0.855, 0.080, 178), "95e1cf", 10.2588),
    ((0.855, 0.095, 20), "ffbbba", 9.6117),
    ((0.930, 0.085, 238), "d1ecff", 12.6153),
]


@pytest.mark.parametrize("oklch,expected,ratio", VECTORS)
def test_the_engine_agrees_with_the_other_implementation(theme, oklch, expected, ratio):
    lightness, chroma, hue = oklch
    got = theme.col.hexcode(lightness, chroma, hue)
    assert got == expected
    assert theme.col.contrast(got, "2b212e") == pytest.approx(ratio, abs=1e-4)


def test_lightness_is_found_and_not_chosen(theme, palette):
    """The floor is the floor: a hair below it and the colour would not clear.

    Every solved colour sits at the softest shade that clears what it was asked
    for, so taking any light out of it drops it under. Without this the palette
    could be passing because somebody picked pale colours, which is a different
    thing from the engine having worked.
    """
    lightness, chroma, hue = theme.to_oklch(palette["soft"])
    lower = theme.col.hexcode(lightness - 0.01, chroma, hue)
    assert theme.col.contrast(palette["soft"], palette["panel"]) >= 7.0
    assert theme.col.contrast(lower, palette["panel"]) < 7.0
