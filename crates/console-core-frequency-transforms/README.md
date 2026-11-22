# Frequency transforms

Transforms sample blocks between time and frequency representations. It owns
the inverse modified discrete cosine transform (IMDCT) and the complex FFT it is
made of, which stays private, along with the reusable transform buffers. AAC and
Vorbis use the IMDCT directly.

The transform is scalar. The SIMD one it began with was behind features nothing
turned on, and was removed with them. A transform is made for a size and refuses
a size it has no kernel for, and a block whose length is not the one it was made
for, with a `Fault` rather than a panic. The largest size is 65536 spectral
samples: the permutation table is 16-bit, and the original's largest FFT was an
empty one.

Window coefficients come from `console-core-audio-windows`. Block sizes,
scaling, short/long transitions, and overlap history belong to each decoder.
Opus CELT retains its mixed-radix transform and folded, low-overlap output:
that has a different numerical contract from this power-of-two transform.

Extracted from the Symphonia code carried in this workspace. Its MPL-2.0 source
notices and `LICENSE` are retained; this workspace offers its copy under
AGPL-3.0-or-later.
