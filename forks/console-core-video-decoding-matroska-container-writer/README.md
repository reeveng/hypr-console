# The Matroska and WebM writer

This is the half of filmcraft's Matroska crate that writes a file, and the
streaming packet reader that only the writer's tests read through, taken out
of `crates/console-core-video-decoding-matroska-container` when the reader came
under the workspace's rules. Nothing on the device writes a film yet, and the
films player reads a Matroska file through its sample tables rather than as a
stream of packets, so both wait here for a rewrite of their own rather than
being carried through the reader's.

Nothing builds it. `forks/` is outside the workspace, and the code is the
master copy's, unedited, so it still names the reader's internals as they were
before the reader was rewritten: the window the old `demux.rs` read through,
its cluster scan and its block expansion, and the element writers below.

- `src/mux.rs` is `MkvWriter`, with its track and lacing options, the small
  element writers the synthetic tests built their files with, and the tests
  that wrote a file under each lacing mode and read it back.
- `src/streaming.rs` is `Demuxer`, which walked a file's clusters in order and
  handed out each frame as a `Packet`, and seeked through the sample index or
  the Cues, with the `SeekPoint` and `Keyframe` it answered in.
- `src/ebml_writing.rs` is how an element ID and a size were written, which
  only the writer called.
- `src/synthetic_tests.rs` is the master copy's hand-built file test, which
  read the file both ways. The reader kept its own copy of the half that reads
  sample tables, with the file built by hand.

The licence files beside it are the upstream's, which offered the code under
MIT or Apache-2.0.
