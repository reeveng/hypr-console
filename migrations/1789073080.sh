# mako's configuration file, on a machine that no longer runs mako.

# The card is drawn here now. `console-notify` owns
# org.freedesktop.Notifications and draws a layer surface out of the same
# palette every other surface on this desktop is drawn from, so the one file
# mako read at startup -- its colours, its size, where it sat, how long a card
# stayed -- is a file nothing reads.
#
# It is worth taking rather than leaving. What it holds is a second opinion
# about what a notification looks like, written in a language nothing here
# speaks any more, and the day somebody finds it while wondering why a card is
# the wrong colour is a day spent on a file that has not been read since this
# commit.
#
# The package itself stays, and so does the mask on its unit: an apply installs
# a name and has never removed one, and a machine that applied any earlier
# commit still has mako on it. The mask is what keeps that copy from taking the
# bus name back, because a user manager lets one unit watch a name and refuses
# the second to load.
#
# sweeps: /home/@user@/.config/mako/config

echo "sweeping mako's configuration, which nothing reads now that the card is ours"

console-attic "$CONSOLE_HOME/.config/mako/config"
console-attic "$CONSOLE_HOME/.config/mako"
