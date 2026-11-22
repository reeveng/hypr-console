# Audio from MPEG-4 containers

Reads audio tracks, codec configuration, metadata and sample timing from base
media containers, including fragmented files. `IsoMp4Reader` implements the
audio decoding interfaces for seekable files and forward-only streams.

`console-core-iso-media-boxes` owns box headers, extended sizes, user types and
full-box version fields. This reader composes those rules with track tables,
codec configuration, metadata and seeking.

The parser uses concrete movie and fragment segments, checked sample offsets,
and distinct types for track numbers, channel counts and sample rates. Unused
boxes are skipped; audio decoding remains the responsibility of each codec.

`tests/the_demuxer.rs` reads every file in `tests/fixtures` seekable, forward
only and cut short, and compares the packets, tags and seeks that come out with
the reports in `tests/golden`, which were written by the reader before it was
rewritten.

Derived from Project Symphonia. Its Mozilla Public License 2.0 notice is
preserved in [LICENSE](LICENSE) and the source files.
