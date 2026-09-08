# The player, under the name it was somebody else's program by.
#
# `/usr/local/bin/kew` was a fork carried as a binary, for the same reason the
# session keeper was: what it is built from is upstream's, and this tree does
# not publish other people's work as its own. What plays music now is
# `music-player`, built here out of `crates/console-music-player` like
# everything else, so `[files]` stopped naming the fork -- and every machine
# that ever applied the old manifest is still holding it.
#
# It has to go, for a plainer reason than the session keeper's. That one was
# dangerous to type. This one is only wrong to find: a binary at
# /usr/local/bin/kew sits in front of the packaged program on the path, so
# whoever types kew a year from now gets a fork whose whole purpose was two
# answers on a bus that nothing asks any more, playing out of a library nothing
# tells it about.
#
# The package leaves [packages] in the same commit and is deliberately not
# swept here. pacman keeps the better answer -- a package the manifest stops
# asking for is held as a dependency or by nothing, and `pacman -Qdtq` is what
# collects it -- and `docs/migrations.md` argues that at length.
#
# What kew was told is carried across before it is swept. ~/.config/kew/kewrc
# held `path=`, which is where the music is, and it was the only place this
# desktop ever wrote that down: `console_music::library` read it back out.
# There is nobody to tell now, because the player is handed the folder on its
# command line, so the answer moves to a file of this desktop's own holding one
# path and nothing else. A person who had told kew where their music was does
# not have to say it again, which is the whole reason this is fourteen lines
# rather than one `console-attic`.
#
# Stopping the old player first is not ceremony. A kew still running holds the
# MPRIS name, and the name is what the panel asks for; the apply that follows
# this one starts nothing, because the player is started by the first song
# somebody presses. So the old one is ended here and the new one is started by
# a thumb, which is also the first honest test that any of this worked.
#
# sweeps: /usr/local/bin/kew

echo "sweeping the player under its old name"

kewrc="$CONSOLE_HOME/.config/kew/kewrc"
ours="$CONSOLE_HOME/.config/console/music"

pkill -x kew >/dev/null 2>&1 || true

if [ -r "$kewrc" ] && [ ! -e "$ours" ]; then
    said=$(sed -n 's/^[[:space:]]*path=[[:space:]]*//p' "$kewrc" | head -1)

    if [ -n "$said" ]; then
        mkdir -p "$(dirname "$ours")"
        printf '%s\n' "$said" > "$ours"
        chown "$(basename "$CONSOLE_HOME")" "$ours" 2>/dev/null || true
        echo "the music folder kew was told is now this desktop's own: $said"
    fi
fi

console-attic /usr/local/bin/kew
console-attic "$CONSOLE_HOME/.config/kew"
