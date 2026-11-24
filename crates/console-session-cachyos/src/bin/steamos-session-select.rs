//! Compatibility entry point for Steam's session switcher.

fn main() -> Result<(), console_session_cachyos::Failure> {
  console_session_cachyos::switch(std::env::args_os().skip(1).collect())
}
