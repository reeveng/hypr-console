//! POSIX shell arguments for boundaries that accept command text.

use console_core_never::Never;

pub fn quote(word: &str) -> Result<String, Never> {
  Ok(format!("'{}'", word.replace('\'', r"'\''")))
}

pub fn join(words: impl IntoIterator<Item = impl AsRef<str>>) -> Result<String, Never> {
  let arguments: Vec<String> = words
    .into_iter()
    .map(|word| {
      let text = word.as_ref();
      let bare = !text.is_empty()
        && text
          .bytes()
          .all(|byte| byte.is_ascii_alphanumeric() || b"._-/+".contains(&byte));

      match bare.then_some(text) {
        Some(unquoted) => unquoted.to_owned(),
        None => {
          let Ok(quoted) = quote(text);

          quoted
        }
      }
    })
    .collect();

  Ok(arguments.join(" "))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn remote_arguments_quote_shell_metacharacters_and_preserve_empty_words() {
    assert_eq!(
      join(["tool", "", "two words", "it's", "$HOME", "one\ntwo", "*"]),
      Ok("tool '' 'two words' 'it'\\''s' '$HOME' 'one\ntwo' '*'".to_owned())
    );
  }
}
