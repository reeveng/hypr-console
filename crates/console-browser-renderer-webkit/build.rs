fn main() -> Result<(), String> {
  pkg_config::Config::new()
    .atleast_version("2.54")
    .probe("wpe-webkit-2.0")
    .and_then(|_| pkg_config::probe_library("wpe-platform-headless-2.0"))
    .map(|_| ())
    .map_err(|error| format!("WebKit renderer requires WPE WebKit 2.54 and its headless platform: {error}"))
}
