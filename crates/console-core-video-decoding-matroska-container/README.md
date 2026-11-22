# console-core-video-decoding-matroska-container

A clean-room Matroska (MKV) and WebM reader, written from the public specifications: RFC 8794
(EBML), RFC 9559 (Matroska), the Matroska codec mappings and the WebM container guidelines. Byte
access is owned by [`console-core-byte-sources`](../console-core-byte-sources/src/lib.rs), shared
with the MPEG-4 reader. The crate has no `unsafe`. No GPL/LGPL code (FFmpeg, libmatroska,
mkvtoolnix) was consulted; FFmpeg was used only as an external test oracle upstream.

The writer and the streaming packet reader that only its tests read through are not here: they
wait in [`forks/console-core-video-decoding-matroska-container-writer`](../../forks/console-core-video-decoding-matroska-container-writer/README.md)
for a rewrite of their own.

## Reading

`ByteSource` and `ReadSeekSource` are re-exported from `console-core-byte-sources`.
Length queries return `std::io::Result<u64>` so a failed measurement reaches the
caller as an I/O error.

```rust
use console_core_video_decoding_matroska_container::{TrackKind, open};

let file = std::fs::File::open("clip.mkv")?;
let mkv = open(&file)?;
let Ok(video) = mkv.track_of_kind(TrackKind::Video);

let frame = match video {
    Some(track) => {
        let Ok(keyframe) = mkv.keyframe_before(track, 2_000_000_000);

        match keyframe {
            Some(sample) => Some(mkv.read_sample(&file, track, sample)?),
            None => None,
        }
    }
    None => None,
};
```

- **Open** (`open`, `open_with(src, &OpenOptions { index, verify_crc })`): EBML header (DocType
  `matroska`/`webm`), the first Segment, and its level-1 elements: SeekHead (followed for elements
  after the clusters), Info, Tracks, Cues, Chapters, Tags, Attachments (listed; payload located by
  offset/size, never read). With `index: true` (default) every Cluster is scanned -- element and
  block headers only, never frame payloads -- to build per-track `Sample` tables (`offset`, `size`,
  `pts`, `duration`, `keyframe`, cluster, block offset, lace index) and keyframe lists. With
  `index: false` open stops at the first Cluster.
- **Faults and warnings.** A file that cannot be opened is an `Error` whose reason is a type --
  `NotMatroska`, `Invalid`, `Unsupported` -- rather than a sentence. Damage the reader recovers from
  is a `Warning` in `MkvFile::warnings`, each naming where it was met.
- **Positions.** A track is asked for by its position in `MkvFile::tracks` as a `u64`, and a sample
  by its position in `Track::samples` as a `u32`, the width the keyframe list holds one at and the
  MPEG-4 reader hands one out at.
- **Timestamps.** Matroska ticks are `TimestampScale` ns (usually 1 ms); `Track::timebase` is that
  as a reduced fraction of a second. `pts` is in ticks. `CodecDelay` is subtracted (RFC 9559), as
  FFmpeg does (Opus/Vorbis priming gives negative pts). Laced frames get
  `block ts + i × (BlockDuration or DefaultDuration)/n`; if neither is known, later laces repeat the
  block timestamp and have `duration == 0`.
- **Keyframes:** SimpleBlock keyframe flag; BlockGroup = keyframe iff it has no ReferenceBlock.
- **Lacing:** Xiph, fixed-size, EBML. **Unknown sizes:** Segment (live/streamed output) and
  Clusters end at the next top-level element. **CRC-32:** optional verification of level-1
  elements and clusters; mismatches go to `MkvFile::crc_errors` and a `Warning`, never a fault.
  **Void** skipped.
- **Damage:** undecodable or oversized elements trigger a byte scan for the next plausible level-1
  element (a Cluster whose first child is Timestamp, or a sized metadata element); a truncated final
  block is dropped.
- **Tracks:** number/UID/type/flags, `DefaultDuration`, name, language (+BCP 47), CodecID,
  CodecPrivate, CodecName, `CodecDelay`, `SeekPreRoll`; `VideoInfo` (pixel/display size, crop,
  interlacing, stereo, alpha mode, `pixel_aspect()`), `Colour` (matrix, range, transfer, primaries,
  subsampling/siting, MaxCLL/MaxFALL, SMPTE 2086 mastering metadata); `AudioInfo` (rate, output
  rate, channels, bit depth); `ContentEncoding`s.
- **Content encodings:** header stripping is undone in `read_sample`. zlib/bzlib/lzo compression
  and encryption are reported (`Track::frames_readable()` is `FrameAccess::Unavailable`), and
  `read_sample` fails with `Unsupported::EncodedFrames`.

### Codec mapping (`Codec`)

| CodecID | `Codec` | payload |
|---|---|---|
| `V_MPEG4/ISO/AVC` | `Avc` | avcC (CodecPrivate) |
| `V_MPEGH/ISO/HEVC` | `Hevc` | hvcC |
| `V_VP8`, `V_VP9` | `Vp8`, `Vp9` | VP9 CodecPrivate features |
| `V_AV1` | `Av1` | av1C |
| `V_PRORES` | `ProRes` | FourCC; frames get the 8-byte `size`+`icpf` header re-added |
| `V_MJPEG`, `V_MS/VFW/FOURCC` (MJPG) | `Mjpeg` | |
| `V_MS/VFW/FOURCC` (other) | `VfwFourcc` | BITMAPINFOHEADER |
| `A_AAC`, legacy `A_AAC/MPEG{2,4}/…` | `Aac` | AudioSpecificConfig (synthesised for legacy IDs, incl. SBR) |
| `A_OPUS` | `Opus` | OpusHead |
| `A_VORBIS` | `Vorbis` | three headers split from Xiph-laced CodecPrivate |
| `A_FLAC` | `Flac` | `fLaC` + metadata blocks |
| `A_PCM/INT/LIT`, `A_PCM/INT/BIG`, `A_PCM/FLOAT/IEEE` | `Pcm` | bit depth from Audio |
| `A_AC3`, `A_EAC3`, `A_MPEG/L3`, `A_MPEG/L2` | `Ac3`, `Eac3`, `Mp3`, `Mp2` | |
| `S_TEXT/UTF8` | `SubRip` | |
| `S_TEXT/WEBVTT`, `D_WEBVTT/*` (WebM) | `WebVtt` | WebM blocks keep `id\nsettings\npayload` |
| `S_TEXT/ASS`, `S_TEXT/SSA` | `Ass` | script header |
| anything else | `Other(id)` | `Track::codec_private` |

`Codec::name()` matches FFmpeg's `codec_name` for these.

## Tests

- Unit: CRC-32, codec mapping, Xiph-laced CodecPrivate, display rotation, pixel aspect.
- A hand-built file (unknown-size Segment and Clusters, header stripping, a BlockGroup, Colour and
  mastering metadata, a projection, Chapters, Tags, an Attachment, CRC-32 pass and fail, damage
  inside a cluster) read back through the sample tables and `read_sample`.
- EBML variable integers, typed values and lacing are tested where they are owned, in
  `console-core-extensible-binary-meta-language` and `console-core-matroska-blocks`.

## Limitations

- Only the first Segment is read (no linked/chained segments, no ordered-chapter editions playback).
- zlib/bzlib/lzo compressed and encrypted tracks are detected but their frames are not decoded.
- BlockAdditions (e.g. VP9 alpha, WebVTT settings in Matroska) are skipped; `EncryptedBlock` ignored.
- `TrackTimestampScale` (deprecated) is ignored.
- Lace timestamps without any duration information repeat the block timestamp (FFmpeg derives them
  from codec parsers).

## Shared format rules

`console-core-extensible-binary-meta-language` owns variable integers and element values.
`console-core-matroska-blocks` owns block headers and laced frame boundaries.
This reader owns track metadata, recovery, timestamps and the sample tables.
