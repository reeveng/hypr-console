# Getting something off the net

A name is typed, what that name found comes back as a list with pictures on it,
and the row taken is fetched into a folder this device already plays out of.
Two tabs: **Audio** puts the sound of a thing into Music, **Video** puts the
whole of it into Videos.

`download-panel` is the card, and it is the same card as everything else here.
[`docs/panels.md`](panels.md) is how one is built; this is what is decided
inside this one.

## One search, two tabs

The two tabs ask the same question. A song and a video are the same thing on
the same site, and the tab decides only what is asked for out of it, which is
why they are two tabs of one panel rather than two programs: the search is not
typed twice, and the shoulders are the whole of the difference.

They are drawn differently, because they are chosen differently. A song is
chosen by whose it is, so the Audio tab says the artist and the length. A video
is chosen by whether it is the one everybody means, so the Video tab says the
length and how many have watched it. A thing already in the folder it would land
in says **have it** at the end of the line, which is the one thing worth knowing
before pressing A and the one thing the site cannot say.

Y over a row offers the other kind of the same thing, and the thing itself in
the browser. Somebody in Video who wanted the song is one press from it rather
than a shoulder, a second search and a press.

## The search is a row, not a keystroke

The line at the top of the tab narrows nothing. What is typed is taken and the
row under the line asks for it: **Look for toto africa**, and the row above the
list says **Looking for** it until the answer arrives.

The other way round -- a search as the letters arrive -- is a question to a site
for every letter, nine answers in ten thrown away before they land, and a list
moving under a thumb that is still typing. A is already the press that walks off
the line onto the first row under it, so asking costs the press that was going
to be made anyway.

A link is taken as well as words. Pasting one in beats typing a title with a
thumb, and the browser is the other way anybody arrives at one of these.

## What is fetched, and why nothing is asked

Nothing on this panel asks about formats. A person who typed a song's name has
said what they want, and a list of codecs is a question about containers put to
somebody holding a handheld.

So the file is chosen by a rule written once, in `console_download::getting`:

**Sound** is the best the site has, unwrapped rather than re-encoded. The best
audio a site keeps is already the small one -- four minutes of opus is four
megabytes -- and everything under it is the stream that sounds like a telephone.
opus is what arrives, what the music player here plays, and what its library
already lists.

**A film** is the smallest file at the height worth having, which is
`res:1080,+size` and no higher. The screen is 2560 by 1600 and this is well
under it, and the size above it is three times the file and three times the
battery to decode for a difference nobody can see at arm's length. `TALL` in
that module is the one number here worth arguing about, which is why it is a
name rather than a number inside a string.

The picture goes inside the file either way, though not by the same hand: yt-dlp
attaches it to an mkv with ffmpeg and writes it into an opus with mutagen, which
is why `python-mutagen` is in the manifest beside yt-dlp. It is an optional
dependency of yt-dlp and so is not installed with it, and without it a song
arrives with no cover and a warning nobody is standing in front of.

A song with no cover is a grey square in the music panel, and the picture is on
the page the thing came from anyway. It is converted to jpg first, because the
site's own is a webp that half the players on this machine draw as nothing.

The name is `%(title)s [%(id)s].%(ext)s`, which is yt-dlp's own default said out
loud rather than left implied: the music library reads it back, taking the id in
square brackets off the end to get the title of a song, and two programs
agreeing about a filename by accident is two programs that disagree the day one
of them changes.

A picture is fetched for every row, and it is converted before it is kept. The
name a site gives one is not what it is: YouTube's all end in `.jpg` and every
one of them arrives as a webp, which the GTK here has no loader for and draws as
nothing -- a row keeping room for a picture that was fetched, kept, and never
seen. ffmpeg is asked what it actually is, and writes it out at 128 across,
which is four times the size a row draws it at and a fortieth of what the site
offered.

## Asking for it twice

A row whose thing is already in the folder says **have it**, and pressing A on
it refuses rather than fetches: the corner says it is already in Music, and no
program is started.

Not politeness. Asked twice, yt-dlp downloads the whole thing again, hands it to
a converter that will not write over what is there, and fails at the last step
with `Conversion failed!` -- which is a minute of somebody's tether spent to
produce a card that says nothing they can act on. Worse, the converter that died
leaves what it had made: a `.meta` and a half-written `.temp.opus`, and that
second one ends in an extension the music panel lists, so the folder quietly
grows a broken second copy of the song.

So the folder is asked first, where the answer is one look at a listing.
`download-get` asks it too rather than trusting the panel, because a link typed
into the line is one nothing has looked up yet, and it says so in a
notification: the panel is a layer over everything on this screen, so a
notification raised while it is up would be drawn behind it, and the corner is
what a card that is on the screen has instead.

A fetch that fails for some other reason takes its own leavings away, and only
its own: anything of that shape older than the run it started belongs to
something else.

## Three programs, because two of them are slow

`download-panel` draws and holds nothing but where each tab is standing.

`download-find` does the looking: one call to yt-dlp with `--flat-playlist`,
which is the whole of why a search takes a second rather than a minute, then the
pictures with curl, and then it writes what came back into the cache. The panel
starts it with `later`, goes on answering the buttons, and reads the file when
it ends.

`download-get` does the fetching, and says so when it lands. A film is minutes,
by which time the card that started it has probably been closed, so the arrival
is a notification rather than a row: the panel's word in the corner says it was
set going, and this says it is done. A fetch that fails says so through
`console-say`, which counts, so a tether that has gone cannot become a wall of
cards, and it says it under the name of the thing that was not fetched -- the
title travels with the link for that reason alone.

Neither of them holds anything the panel needs. A search lives in
`~/.cache/console/download`, which is why walking to the other tab and back
leaves a list exactly as it was, and why nothing is lost when the card is put
away.

## Making what is already there one format

`one-format` is the same decision applied backwards: to what is in the folder
already rather than to what is arriving. Everything that plays becomes opus,
every film becomes mkv, and what it replaces goes to the wastebasket. It is
under Y in the Files panel, on the row that is the folder you are standing in,
and it asks before it starts.

    one-format                     the music folder and the videos folder
    one-format /run/media/stick    whatever is in there

It is there because the panel is not the only way a file arrives. Things come
off a laptop, out of localsend, down from syncthing, and a folder of nine
extensions is a folder you need nine programs to be sure of.

The two halves are not the same operation and the difference is worth knowing.
A film is **remuxed**: the streams are lifted out of one container and put into
another, nothing is decoded, nothing is lost, and a gigabyte takes about a
second. Anything that will not go in whole is left alone rather than
re-encoded, because half an hour of a handheld's battery is not a thing to
spend on a file extension without being asked.

Sound is **re-encoded**, at 128k, and that is a real loss: an mp3 made opus has
been through two lossy encoders. It is done anyway, because 128k opus off a
320k mp3 is a thing nobody picks out on a handheld's speakers, because the file
gets smaller, and because the alternative is the folder staying nine formats
for ever. A flac made opus is the one that costs something somebody may care
about, which is the other reason the original goes to the wastebasket rather
than being unlinked.

What is written on a song is carried over, and so is the cover -- but not the
same way. Tags come with `-map_metadata`; a picture cannot, because the opus
muxer refuses a picture stream outright and writes no file at all. So the cover
is taken out, and put back as `METADATA_BLOCK_PICTURE`, which is the comment
the format actually keeps a picture in and what everything else that writes one
writes.

One folder, not the tree under it: a folder is what somebody is standing in and
what they asked about, and a tree is a thing that runs for an hour over places
they were not thinking of. A name that is already taken is left alone, because
two files that would become one name are two files somebody chose to keep.

## What is not here

No queue, and nothing that says how far along a fetch is. What is on the screen
while a film arrives is the corner saying it was started and, later, a
notification saying where it went. That is the same offer the wallpapers make,
and it is enough for one thing at a time; a tab that lists what is in flight is
the thing to build if it stops being.
