mod recipes;
mod release;

use std::process::ExitCode;

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  match words.as_slice() {
    [mode, arguments @ ..] if mode == "publish" => {
      let Ok(code) = release::publish_main(arguments);
      code
    }
    [mode, arguments @ ..] if mode == "release-tag" => {
      let Ok(code) = release::tag_main(arguments);
      code
    }
    arguments => match recipes::run(arguments) {
      Ok(()) => ExitCode::SUCCESS,
      Err(fault) => {
        eprintln!("just: {fault}");
        recipes::exit_code(&fault)
      }
    },
  }
}
