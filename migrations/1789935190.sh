# The picture press, under the word it was a press by.

# `wallpaper-press` is `wallpaper-render`. Nothing about what it does changed:
# it decodes a source loop, grades it through a cube, cuts it to this screen and
# writes a WebP. What changed is that this tree had one word for two things --
# a thumb on a button and ffmpeg turning frames into a picture -- and only the
# first of them is what anybody else calls a press.
#
# `[build]` names the new binary, so an apply installs
# `/usr/local/bin/wallpaper-render` and leaves the old one exactly where it is.
# It is not dangerous to have there and it is wrong to find. The old binary
# still runs, still reads `theme/sky.toml`'s sources and still writes into
# `/usr/share/backgrounds` and the frame cache beside it, so a year from now
# somebody typing the name they remember gets a program built from a commit
# nobody has looked at since, quietly writing the pictures the live one is
# reading. Two generations of one program over one directory is the fault the
# rename of every `legion-*` unit was about, one binary smaller.
#
# Nothing is stopped first, because nothing starts it. No unit runs the presser:
# `console-wallpaper.service` starts the daemon that chooses, `console apply`
# runs the renderer itself in `[build]`, and the Wallpaper tab and the Files
# panel both spell the name at the moment somebody presses a row. All three of
# those now say `wallpaper-render`, and what is left on the machine is a file on
# the path that nothing reaches for.
#
# sweeps: /usr/local/bin/wallpaper-press

echo "sweeping the picture press under its old name"

console-attic /usr/local/bin/wallpaper-press
