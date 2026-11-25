use std::{borrow::Cow, process::Command};

use console_core_never::Never;

use super::UpdateError;

pub fn output(command: &mut Command) -> Result<String, UpdateError> {
  let result = command.output()?;

  match { ::core::primitive::bool::then_some(!result.status.success(), ()) } {
    ::core::option::Option::Some(()) => {
      let Ok(details) = diagnostic(&result.stderr);
      return Err(UpdateError::from(format!(
        "{command:?} exited with {}: {details}",
        result.status
      )));
    }
    ::core::option::Option::None => {}
  }

  let output = String::from_utf8(result.stdout)?;

  Ok(output.trim().to_owned())
}

pub fn diagnostic(bytes: &[u8]) -> Result<Cow<'_, str>, Never> {
  Ok(match std::str::from_utf8(bytes) {
    Ok(message) => Cow::Borrowed(message.trim()),
    Err(_stderr_not_utf8) => Cow::Owned(format!("{bytes:?}")),
  })
}

pub fn checked(command: &mut Command) -> Result<(), UpdateError> {
  let status = command.status()?;

  match { ::core::primitive::bool::then_some(status.success(), ()) } {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(UpdateError::from(format!(
      "{command:?} exited with {status}"
    ))),
  }
}
