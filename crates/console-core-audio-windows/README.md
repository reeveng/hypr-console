# Audio windows

Generates the coefficients used to join blocks of decoded audio. AAC uses sine
and Kaiser–Bessel windows; Vorbis and Opus CELT share the Vorbis half-window.

`generate_window` retains the AAC generator's size, half-window, and scale
arguments, the half-window now said as an `Extent`. It writes the first `size`
coefficients of the supplied slice, as many as fit when the slice is shorter. The existing Kaiser–Bessel calculation normalizes its
coefficients independently of the sine window's scale argument.

`vorbis_half` fills the supplied slice with the left half of a Vorbis window;
the slice length is the half-window length. It allocates no memory. Vorbis
and Opus use this same generator with their own block and overlap lengths.

The decoders still choose their window shape and retain their transition and
overlap state. The generator has no packet reader, decoder state, or device.
Tests check the complementary squared coefficients needed for alias
cancellation and the caller's output-buffer boundaries.

Extracted from the Symphonia code carried in this workspace. Its MPL-2.0 source
notices and `LICENSE` are retained; this workspace offers its copy under
AGPL-3.0-or-later.
