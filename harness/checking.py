"""Reading the checks, and running them somewhere.

A check is one file, and one feature. It says what somebody did and what should
have happened, and it is edited in place when the feature changes rather than
joined by a second file saying something different. Running them in order walks
everything this desktop has grown, oldest first, and says which of it still
works.

Large features are split, because "the d-pad works" is not a thing that fails:
left works or right works, and a check that presses both and asserts once tells
you neither which failed nor that only one did.
"""

import importlib.machinery
import importlib.util
import traceback
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CHECKS = REPO / "checks"


class Check:
    """One file, loaded."""

    def __init__(self, path):
        self.path = Path(path)
        self.name = self.path.stem
        loader = importlib.machinery.SourceFileLoader(
            "legion_check_" + self.name.replace("-", "_"), str(self.path))
        spec = importlib.util.spec_from_loader(loader.name, loader)
        self.module = importlib.util.module_from_spec(spec)
        loader.exec_module(self.module)

    @property
    def about(self):
        return (self.module.__doc__ or "").strip().splitlines()[0]

    @property
    def feature(self):
        return getattr(self.module, "FEATURE", self.name)

    def body(self, stage):
        """The most particular version of this check that stage can run."""
        return (getattr(self.module, stage.name, None)
                or getattr(self.module, "check", None))

    def why_not(self, stage):
        """Why this cannot run here, or nothing.

        Only whether anything is written for this stage. What a check needs to
        be able to see is not declared anywhere: it asks the stage for it, and
        a stage that cannot answer does not have the method. Declaring it as
        well would be the same fact written twice, and the second copy would be
        the one that went stale.
        """
        if self.body(stage) is None:
            return "nothing written for %s" % stage.name
        return None


def load(only=None):
    """Every check, oldest first, since the name begins with when it arrived."""
    found = [Check(p) for p in sorted(CHECKS.glob("*.py"))]
    if only:
        found = [c for c in found
                 if any(word in c.name or word == c.feature for word in only)]
    return found


def run(check, stage):
    """One check. Returns (how, why): ok, skipped, failed, or would.

    Nothing is judged on a dry run. The machine answers nothing, so every
    assertion in the check is about emptiness and would fail for a reason that
    is not about the desktop. What a dry run is for is reading the commands
    before they are sent.
    """
    why = check.why_not(stage)
    if why:
        return "skipped", why
    # One check's picture is not another's, and one check's idea of what
    # should be running is its own. A stage is used by all of them in turn.
    if hasattr(stage, "fresh"):
        stage.fresh()
    if getattr(stage, "dry", False):
        try:
            check.body(stage)(stage, stage)
        except NotImplementedError as exc:
            return "skipped", str(exc)
        except AttributeError as exc:
            if getattr(exc, "obj", None) is stage:
                return "skipped", "%s cannot see %s" % (stage.name, exc.name)
        except Exception:                  # noqa: BLE001 - nothing answered
            pass
        return "would", ""
    try:
        check.body(stage)(stage, stage)
    except AssertionError as exc:
        return "failed", str(exc) or "no reason given"
    except NotImplementedError as exc:
        return "skipped", str(exc)
    except AttributeError as exc:
        if getattr(exc, "obj", None) is stage:
            return "skipped", "%s cannot see %s" % (stage.name, exc.name)
        raise
    except Exception:                      # noqa: BLE001 - a check may do anything
        return "failed", traceback.format_exc(limit=3).strip().splitlines()[-1]
    return "ok", ""
