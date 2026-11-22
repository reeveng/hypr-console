# Video planes

Owns samples from a decoded video plane as `Plane::U8` or `Plane::U16`.
HEVC and VP9 re-export the same type, and film playback consumes it directly.
Sample access and little-endian rawvideo output preserve the stored precision.

Picture dimensions, strides, bit depth, chroma layout and colour metadata stay
with each decoder's picture. This crate is `no_std` with `alloc`.

The sample type comes from Filmcraft's HEVC and VP9 decoders. Their MIT and
Apache notices are retained; this copy is offered under AGPL-3.0-or-later.
