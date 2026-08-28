"""Every check, run here, as part of the ordinary suite.

The checks exist to be replayed against the device at the end. That only means
anything if they still run at all, and a check nobody has run since the feature
changed is a check that will fail on the device for a reason that has nothing
to do with the device. So they are also the fast suite: every one of them that
can run without a machine runs on every `make fast`.
"""

import pytest

from harness import checking
from harness.stage import Here

ALL = checking.load()


def test_there_are_checks():
    assert ALL


def test_every_check_says_what_it_is_and_when_it_arrived():
    for check in ALL:
        assert check.about, "%s says nothing about itself" % check.name
        assert check.name[:3].isdigit(), \
            "%s does not begin with when it arrived" % check.name
        assert hasattr(check.module, "SINCE"), "%s has no SINCE" % check.name


def test_one_file_to_a_feature():
    """A feature split across files is split on purpose, into parts that fail
    separately. Two files claiming the whole of one feature is the thing this
    is meant not to become."""
    for check in ALL:
        rest = check.name.partition("-")[2]
        assert rest, "%s is a number and nothing else" % check.name


@pytest.mark.parametrize("check", ALL, ids=[c.name for c in ALL])
def test_check_runs_here(check):
    stage = Here()
    try:
        how, why = checking.run(check, stage)
    finally:
        stage.close()
    if how == "skipped":
        pytest.skip(why)
    assert how == "ok", why
