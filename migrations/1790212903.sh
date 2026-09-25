# The profile the pad wore while the card that asks for a button was up.

# `console apply` wrote it out of what the device answered it could send, the
# same way it writes `router.yaml` now, and so it was never a line under
# `[files]`. That is why the gate never asked for it: the gate reads the
# manifest's history, and a file the engine generated has no history there. It
# went in 923a14d8, when a button's meaning moved out of the profile and into
# the daemon, and the question it answered became one asked of the kernel.
#
# Left, it lies in the voice the zzz-session file did: anybody reading /etc to
# find out what the pad does while a card is asking is told about a profile
# nothing loads.

echo "sweeping the profile the pad wore while a card was asking"

console-attic /etc/inputplumber/profiles/asking.yaml
