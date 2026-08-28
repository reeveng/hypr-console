"""The wallpaper is the one surface that cannot be read for its colours.

A stylesheet can be searched for a hex that should not be there; a picture is
pixels and gives nothing up. So it is held to its palette from both ends: a
stamp saying what it was drawn from, and the file itself read back for the
things the palette decided.

Everything about the palette itself now lives in `crates/legion-theme`, where
it is tested in Rust. What is left here is the drawing, until that moves too.
"""

import importlib.machinery
import importlib.util
import re
import tomllib

import pytest

from harness.screen import Screen


@pytest.fixture(scope="module")
def declared(request):
    return tomllib.loads((request.config.rootpath / "theme/palette.toml").read_text())


@pytest.fixture(scope="module")
def palette(request):
    """Every colour as it was solved, read out of the report.

    Read rather than computed, because the engine that computes it is Rust now
    and this is the last of the python. `theme/report.md` is the palette
    written down, and `legion-theme --check` is what keeps it current; a Rust
    test fails if it has fallen behind.
    """
    report = (request.config.rootpath / "theme/report.md").read_text()
    rows = re.findall(r"^\| `(\w+)` \| `#([0-9a-f]{6})` \|", report, re.M)
    assert rows, "theme/report.md holds no colours; run `make theme`"
    return dict(rows)


# ------------------------------------------------------------------ the garden
# The wallpaper is the one surface that cannot be read for its colours. A
# stylesheet can be searched for a hex that should not be there; a picture is
# pixels and gives nothing up. So it is held to its palette from both ends: a
# stamp saying what it was drawn from, and the file itself read back for the
# things the palette decided.


@pytest.fixture(scope="module")
def garden(request):
    pytest.importorskip("cairo", reason="the garden is drawn where it is drawn")
    path = request.config.rootpath / "tools/legion-garden"
    loader = importlib.machinery.SourceFileLoader("legion_garden", str(path))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


def frames(path):
    """Every frame of an animated WebP: where it sits, and how long it lasts.

    The container is read here rather than asked of a library, because what is
    being checked is the thing this repository wrote into the file.
    """
    data = path.read_bytes()
    assert data[:4] == b"RIFF" and data[8:12] == b"WEBP"
    at, out = 12, []
    while at < len(data):
        tag = data[at:at + 4]
        size = int.from_bytes(data[at + 4:at + 8], "little")
        body = data[at + 8:at + 8 + size]
        if tag == b"ANMF":
            out.append({
                "x": int.from_bytes(body[0:3], "little") * 2,
                "y": int.from_bytes(body[3:6], "little") * 2,
                "width": int.from_bytes(body[6:9], "little") + 1,
                "height": int.from_bytes(body[9:12], "little") + 1,
                "ms": int.from_bytes(body[12:15], "little"),
            })
        at += 8 + size + (size & 1)
    return out


def test_the_garden_is_drawn_from_the_palette_as_it_stands(garden, declared, palette):
    """Change a colour and the picture is a lie until it is drawn again.

    Every other surface is checked by reading the colour back out of it. This
    one is checked by the stamp the drawing left behind, which is the same
    promise made the only way a picture can make it.
    """
    assert garden.STAMP.exists(), "the garden has never been drawn"
    stamped = tomllib.loads(garden.STAMP.read_text())
    assert stamped["palette"] == garden.stamp(declared, palette), \
        "the wallpaper is older than the palette; run `make garden`"
    assert (stamped["width"], stamped["height"]) == (garden.WIDTH, garden.HEIGHT)
    assert re.fullmatch(r"#[0-9a-f]{6}", stamped["resting"])


def test_every_colour_the_garden_paints_with_is_in_the_palette(declared):
    for part, said in declared["garden"]["paint"].items():
        assert said["colour"] in declared["colour"], \
            "the garden paints %s with %s, which is not a colour" % (part, said["colour"])
        assert 0 < said["alpha"] <= 1.0


def test_the_garden_is_the_shape_of_the_screen(garden):
    """The panel is mounted portrait and the compositor turns it, so the
    wallpaper is landscape and the mode in the compositor's file is not.

    Read off the picture and not off the tool that drew it. The tool takes its
    size from the same place this does, so asking it would only prove that a
    number equals itself; what is worth knowing is whether the file somebody
    would install is that shape. A screen that changes and a wallpaper nobody
    redrew is exactly the way this comes back.

    It is here because it was wrong. The picture was drawn the shape of the
    panel, the daemon cropped it to the shape of the desktop, and because what
    it held was a gradient there was nothing on screen to say so.
    """
    canvas = frames(garden.CANVAS)[0]
    assert (canvas["width"], canvas["height"]) == Screen().pixels


def test_the_garden_rests_for_as_long_as_it_says(garden, declared):
    """The whole reason this costs nothing is the first frame's duration. If it
    were ever written as milliseconds where seconds were meant, the wallpaper
    would run a wind every few seconds on a machine held in two hands."""
    said = declared["garden"]
    shown = frames(garden.CANVAS)
    assert shown[0]["ms"] == round(said["rest_seconds"] * 1000)
    assert len(shown) - 1 == round(said["gust_seconds"] * said["frames_per_second"])
    blowing = sum(frame["ms"] for frame in shown[1:]) / 1000
    assert blowing == pytest.approx(said["gust_seconds"], abs=0.1)


def test_the_wind_redraws_a_band_and_not_the_picture(garden):
    """A gust that redraws the whole picture every frame is a wallpaper that
    costs its own size over and over. The first frame is the picture; every
    frame after it is a strip of the picture and nothing else."""
    shown = frames(garden.CANVAS)
    assert (shown[0]["width"], shown[0]["height"]) == (garden.WIDTH, garden.HEIGHT)
    for frame in shown[1:]:
        assert frame["width"] == garden.WIDTH
        assert frame["height"] < garden.HEIGHT
        assert frame["y"] + frame["height"] <= garden.HEIGHT


def test_the_wind_puts_the_picture_back(garden, declared, palette):
    """The last frame of a gust holds no petals, so what loops round to the
    resting picture is the resting picture. Without it the wallpaper would
    creep: every wind would leave its blossom somewhere it had blown to."""
    import cairo
    paint = garden.paints(declared, palette)
    seed = 20260828
    band, ctx = garden.sheet(garden.WIDTH, garden.HEIGHT)
    garden.scene(ctx, paint, seed)
    settled = band.get_data().tobytes()

    band, ctx = garden.sheet(garden.WIDTH, garden.HEIGHT)
    tips = garden.scene(ctx, paint, seed)
    garden.blown(ctx, paint, garden.flight(tips, __import__("random").Random(seed + 11), 170), 1.0)
    assert band.get_data().tobytes() == settled


def test_no_probe_could_pass_against_a_bare_screen(garden, palette):
    """The compositor's own background is the picture's darkest colour, so that
    a wallpaper daemon dying costs the right colour rather than a grey nobody
    chose. That kindness is also a blindness: a check that samples the dark
    part of the sky reads the same thing whether the picture is there or not.

    So every probe has to sit somewhere the picture has a colour of its own.
    This is a test rather than only a guard in the drawing, because the way it
    comes back is somebody moving the composition and not redrawing, and then
    the check that is meant to prove the wallpaper is painting proves nothing
    at all while staying green.
    """
    stamped = tomllib.loads(garden.STAMP.read_text())
    probes = [(tuple(one["at"]), one["colour"].lstrip("#")) for one in stamped["probe"]]
    assert len(probes) >= 3, "one probe cannot say a picture is the right way up"
    assert not garden.blind(probes, palette["night"])
