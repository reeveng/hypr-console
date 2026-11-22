# The MPEG-4 and QuickTime writer

This is the half of filmcraft's ISO base media crate that writes a file, taken
out of `crates/console-core-video-decoding-mpeg-4-container` when the reader
came under the workspace's rules. Nothing on the device writes a film yet, so
the writer waits here for a rewrite of its own rather than being carried
through the reader's.

Nothing builds it. `forks/` is outside the workspace, and the code is the
master copy's, unedited, so it still names the reader's internals as they were
before the reader was rewritten: the cursor and the box builder in the old
`bytes.rs`, and the sample entries and configuration records in the old
`codec.rs`.

- `src/mux.rs` is the progressive writer, `Mp4Writer`, which can also move its
  `moov` to the front, and the fragmented writer, `FragmentedWriter`, with the
  tests that read what they wrote back through the reader.
- `src/sample_entries.rs` is how a sample entry is written: the constructors
  for each kind of entry, the boxes each one is serialized into, the `esds`
  descriptors, and the round-trip tests that wrote an entry and parsed it again.
- `src/configurations.rs` is the serialization of the configuration records
  the reader parses, which only the writer called. The reader kept the two the
  films player reads, the H.264 and HEVC records.
- `src/box_buffer.rs` is the growable buffer every box was built in.

The licence files beside it are the upstream's, which offered the code under
MIT or Apache-2.0.
