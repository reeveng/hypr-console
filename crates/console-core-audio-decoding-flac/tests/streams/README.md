These ten streams exercise the lossless decoder's channel assignments, predictor
orders, sample depths, block sizes and unusual channel counts and sample rates.
The integration tests verify decoded audio against each stream's stored digest.
Packet, sample, metadata and seek expectations were checked against the original
reader and decoder at repository revision `b9253b74`, with only adaptations to
shared interface signatures.

`stereo16.flac` also contains tags, a picture, seek points and a cuesheet. Its
unfinished non-disc cuesheet was normalized: lead-in samples are zero and the
lead-out track number is 255, as described in
[RFC 9639, section 8.7](https://www.rfc-editor.org/rfc/rfc9639.html#section-8.7).
Only four metadata bytes changed; the encoded audio is byte-for-byte identical.
Its encoded audio starts at byte 36,862 and has SHA-256
`816b44ed9f2be78cc08a56efa722a7e20a2a9e989c22c89f2e38579ab6e99b9a`.
The corruption test flips bytes inside this audio region.
