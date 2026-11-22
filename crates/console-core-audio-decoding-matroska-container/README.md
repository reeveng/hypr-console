# Symphonia MKV/WebM Demuxer

MKV/WebM demuxer for Project Symphonia.

**Note:** This crate is part of Symphonia. Please use the [`symphonia`](https://crates.io/crates/symphonia) crate instead of this one directly.

## License

Symphonia is provided under the MPL v2.0 license. Please refer to the LICENSE file for more details.

## Contributing

Symphonia is a free and open-source project that welcomes contributions! To get started, please read our [Contribution Guidelines](https://github.com/pdeljanov/Symphonia/tree/master/CONTRIBUTING.md).

## Shared format rules

`console-core-extensible-binary-meta-language` owns variable integers and element values.
`console-core-matroska-blocks` owns block headers and laced frame boundaries.
This reader owns track metadata, recovery, timestamps and packet delivery.
