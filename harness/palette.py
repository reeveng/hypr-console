"""The colours as the machine spends them.

Read out of the file every themed surface is themed from, so a palette that
moves moves its checks with it. A check carrying its own copy of a colour is a
check that goes red for somebody else's good reason, or worse, stays green
against a colour nothing uses any more.
"""

import re
from pathlib import Path

SPENT = Path(__file__).resolve().parent.parent \
    / "files/usr/local/lib/legion/palette.sh"


def palette():
    found = {}
    for line in SPENT.read_text().splitlines():
        named = re.match(r"(\w+)=([0-9a-fA-F]{6})\s*$", line)
        if named:
            found[named.group(1)] = named.group(2).lower()
    if not found:
        raise AssertionError("no colours in %s" % SPENT)
    return found
