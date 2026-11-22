# Data URLs

Reads `data:` URLs directly from text using the Fetch Standard's MIME,
percent-decoding, Base64 and fragment rules. The streaming decoder keeps
invalid Base64 separate from an output writer's failure.

`DataUrl::process` and `decode_to_vec` return typed failures. `mime_type`,
`Mime::get_parameter`, `FragmentIdentifier::to_percent_encoded` and
`forgiving_base64::Decoder::new` return `Result<_, Never>`.

The implementation started as data-url 0.3.2 by Simon Sapin, under MIT or
Apache-2.0. Its original license files remain here. This version uses the
workspace's explicit rules, shares percent decoding with the other URL
readers, and preserves the original streaming byte spans and parsing behavior.
