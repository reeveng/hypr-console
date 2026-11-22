//! Resolve EPUB resource links to ZIP entry names.

use console_core_never::Never;

fn percent_decode(text: &str) -> Result<String, Never> {
  let Ok(bytes) = console_core_percent_decoding::decoded(text.as_bytes());
  let Ok(decoded) = crate::utf8_with_replacement(&bytes);

  Ok(decoded.into_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relative<'a> {
  pub base: &'a str,
  pub path: &'a str,
}

pub fn resolve(link: Relative<'_>) -> Result<String, Never> {
  let Relative {
    base: from,
    path: link_2,
  } = link;
  let link_4 = match link_2.split_once('#') {
    Some((link_3, _fragment)) => link_3,
    None => link_2,
  };

  let Ok(link_5) = percent_decode(link_4);

  let folder = match from.rsplit_once('/') {
    Some((folder, _file)) => folder,
    None => "",
  };

  let mut parts: Vec<&str> = match link_5.as_bytes() {
    [b'/', ..] => Vec::new(),
    _ => folder.split('/').filter(|part| !part.is_empty()).collect(),
  };

  for part in link_5.split('/') {
    match part {
      "" | "." => {}
      ".." => {
        let _ = parts.pop();
      }
      part_2 => parts.push(part_2),
    }
  }

  Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_link_is_named_the_way_the_zip_names_it() {
    assert_eq!(
      resolve(Relative {
        base: "OEBPS/text/one.xhtml",
        path: "../images/a%20b.jpg#top"
      }),
      Ok("OEBPS/images/a b.jpg".to_string())
    );
    assert_eq!(
      resolve(Relative {
        base: "content.opf",
        path: "chapter.xhtml"
      }),
      Ok("chapter.xhtml".to_string())
    );
    assert_eq!(
      resolve(Relative {
        base: "OEBPS/content.opf",
        path: "./text/one.xhtml"
      }),
      Ok("OEBPS/text/one.xhtml".to_string())
    );
  }

  #[test]
  fn malformed_utf8_in_escaped_bytes_preserves_the_surrounding_link() {
    assert_eq!(
      resolve(Relative {
        base: "OEBPS/content.opf",
        path: "text/caf%E9%E2%82.xhtml"
      }),
      Ok("OEBPS/text/caf\u{fffd}\u{fffd}.xhtml".to_string())
    );
  }
}
