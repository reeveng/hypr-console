# MPEG-4 container

The reader for MPEG-4 and QuickTime films: what tracks a film holds, where
each of their samples sits in the file and when it is shown, and the record
each track's codec is configured by. The films player opens a film through it
and hands each sample to a decoder.

`open` takes any `ByteSource` -- a slice, a `Vec`, a file -- and reads only the
top-level box headers and the `ftyp`, `moov` and `moof` boxes. The media data
stays where it is until `Track::read_sample` asks for one sample's bytes, so
opening a film costs its headers rather than its length. The one exception is
a timecode track, whose first sample is read when the film is opened because
it holds the frame the timecode starts at.

A track's samples come out in decoding order, each with its offset, size,
decoding and composition times, duration and whether it is a sync sample.
Times are in the track's own timescale. `edit_offset` is what the edit list
moves them by -- the leading empty edits, rescaled to the track, less the
first real edit's start -- so a sample is shown at its composition time plus
that offset, which is what ffprobe reports for a B-frame delay, AAC priming
and an empty edit. An edit list that does more than that is kept as it was
written for a caller to read. Uncompressed sound is kept one sample to a chunk
rather than one to a frame, and the track says so in `chunking`. A fragmented
film's samples follow the movie's own, in the order its fragments are
written.

A sample entry is read as a picture, a sound or a timecode by its four letters
first and by its track's handler when the letters are not ones the lists
know. H.264, HEVC, AV1, VP9 and APV records are parsed into their fields, and
the H.264 and HEVC records can be written back out as the bytes a decoder
reads. AAC's `esds` is followed through to its AudioSpecificConfig, inside a
QuickTime `wave` as well; PCM says its sample width, whether its samples are
integers or floats, and their byte order; Opus, FLAC, ALAC, AC-3 and E-AC-3
carry what their boxes hold; ProRes, JPEG and DNx are named by their letters.
A picture entry also reads its colour description, pixel aspect, clean
aperture, field order, gamma, mastering display, content light level and bit
rate. Anything else is `Unknown`, with its bytes.

Damage is an `Error` rather than a panic, and a count a table claims is
checked against the bytes in its box before anything is allocated for it. A
constant sample size is held to what the file could hold, each track has a
ceiling on its samples, and a header box has a ceiling on its size. A
compressed movie header, external data references and encrypted entries are
not read.

Box framing -- the size forms, user types and full-box versions -- is
`console-core-iso-media-boxes`, which the audio reader shares, and bytes are
read through `console-core-byte-sources`.

## Where it came from

It began as filmcraft's clean-room ISO base media crate, by the ArtCraft team
and the FilmCraft contributors under MIT or Apache-2.0, whose licence files are
kept beside it, and is offered here under AGPL-3.0-or-later. The muxer that
crate carried, plain and fragmented, waits in
`forks/console-core-video-decoding-mpeg-4-container-writer` for a rewrite of
its own, since nothing on the device writes a film, and the tests that wrote a
file to read it back now write their boxes by hand.
