use console_core_font_configuration::{Bases, Error, FontConfig};

fn main() -> Result<(), Error> {
  let named = std::env::args().nth(1);

  match named {
    Some(path) => {
      let mut config = FontConfig::default();
      config.merge_config(path.as_str(), &Bases::default())?;

      println!("{config:#?}");
    }
    None => eprintln!("Usage: cargo run --example dump -- <conf file path>"),
  }

  Ok(())
}
