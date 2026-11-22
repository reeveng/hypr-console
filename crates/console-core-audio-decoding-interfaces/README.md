# Audio decoding interfaces

The words every demuxer and decoder in the audio family is written to: the
`FormatReader` and `Decoder` traits, the codec parameters, packets, tracks,
tags and audio buffers they hand each other, the time bases they count in, the
probe that picks a reader for a stream, and the errors they fail with.

The sample formats and their conversions are `console-core-audio-samples`, and
the byte and bit readers are `console-core-byte-streams`. They are re-exported
as `sample`, `conv` and `io`, which are the paths the family names them by.

It began as `symphonia-core`, by the Project Symphonia developers, offered
under the Mozilla Public License 2.0, whose text is `LICENSE`.
