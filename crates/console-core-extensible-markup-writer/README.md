# XML writer

Streaming XML output with element auto-closing, configurable indentation,
attribute quote escaping and text escaping. Element names stay as ranges in
the output buffer rather than separate allocated strings.

Construction and other infallible operations return `Result<_, Never>`.
Invalid call order, formatting failures, raw buffer corruption and invalid
UTF-8 return a typed `Fault`. `Whitespace::Preserve` and `Whitespace::Indent`
select how text is laid out.

The implementation began as xmlwriter 0.1.0 by Yevhenii Reizner. Its original
MIT license remains here. Output for valid writer operations is preserved.
