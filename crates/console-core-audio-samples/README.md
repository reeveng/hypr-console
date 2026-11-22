# Audio samples

Represents the value of an audio sample and converts it to another numerical
format. This owns the signed and unsigned 24-bit types, the `Sample` contract,
and `FromSample`/`IntoSample`, which never dither: a conversion to fewer bits
drops the low ones.

The crate is `no_std`. It knows nothing about a codec,
container, channel layout, device, or audio buffer. `console-core-audio-decoding-interfaces`
re-exports its `sample` and `conv` modules at their existing paths;
the re-exported types and conversion traits are the same types, not wrappers.
PCM decoding uses this crate directly. Audio buffer layout stays with the
interfaces that define it.

Extracted from the Symphonia code carried in this workspace. Its MPL-2.0 source
notices and `LICENSE` are retained; this workspace offers its copy under
AGPL-3.0-or-later.
