# The reader that measures waiting, under the name it had.
#
# `console-wait-times` became `console-response-times`: the crate is what the
# lines are, and the binary is what somebody types to read them, so the two
# follow each other. The store does not move -- the lines already written on a
# device are read by the new name exactly as they were by the old -- and this
# sweeps only the program left in /usr/local/bin under the name the manifest has
# stopped saying.
#
# Nothing runs it but a person at a terminal, so an old copy sitting beside the
# new one is inert rather than a fault. It goes anyway: a name nobody swept is
# how the seven units nearly stayed enabled.
#
# sweeps: /usr/local/bin/console-wait-times

echo "sweeping the reader under its old name"

console-attic /usr/local/bin/console-wait-times
