use crate::*;
use roxmltree::Node;

pub fn parse_config<'a>(
  xml_doc: &'a roxmltree::Document,
) -> Result<impl Iterator<Item = Result<ConfigPart>> + 'a> {
  let fontconfig = xml_doc.root_element().map_err(Error::Xml)?;
  let Ok(root_tag) = fontconfig.tag_name();
  let Ok(root_name) = root_tag.name();

  match root_name {
    "fontconfig" => {}
    _ => return Err(Error::NoFontconfig),
  }

  Ok(
    ({
      let Ok(explicit_value) = fontconfig.children();
      explicit_value
    })
    .filter_map(|c| parse_config_part(c).transpose()),
  )
}

fn required_text<'a>(node: Node<'a, '_>) -> Result<&'a str> {
  let Ok(content) = node.text();

  content.ok_or_else(|| Error::InvalidFormat(String::from("Can't get text")))
}

fn required_next<T>(items: &mut impl Iterator<Item = T>, missing: &str) -> Result<T> {
  items
    .next()
    .ok_or_else(|| Error::InvalidFormat(String::from(missing)))
}

fn next_expression(items: &mut impl Iterator<Item = Result<Expression>>) -> Result<Expression> {
  required_next(items, "Expect expression").flatten()
}

fn parse_config_part(child: Node) -> Result<Option<ConfigPart>> {
  let Ok(child_tag) = child.tag_name();
  let Ok(child_name) = child_tag.name();
  let part = match child_name {
    "description" => {
      let content = required_text(child)?;

      ConfigPart::Description(String::from(content))
    }
    "alias" => {
      let mut alias = Alias::default();

      for alias_member in {
        let Ok(explicit_value) = child.children();
        explicit_value
      } {
        let families = ({
          let Ok(explicit_value) = alias_member.children();
          explicit_value
        })
        .filter_map(|family| {
          let Ok(family_tag) = family.tag_name();
          let Ok(family_name) = family_tag.name();

          match family_name {
            "family" => ({
              let Ok(explicit_value) = family.text();
              explicit_value
            })
            .map(String::from),
            _ => None,
          }
        });

        let Ok(child_tag_2) = alias_member.tag_name();
        let Ok(child_name_2) = child_tag_2.name();

        match child_name_2 {
          "family" => {
            let content = required_text(alias_member)?;
            alias.alias = String::from(content);
          }
          "prefer" => {
            alias.prefer.extend(families);
          }
          "accept" => {
            alias.accept.extend(families);
          }
          "default" => {
            alias.default.extend(families);
          }
          _ => {}
        }
      }

      ConfigPart::Alias(alias)
    }
    "dir" => {
      let mut dir = Dir::default();

      parse_attrs!(child, {
          "prefix" => dir.prefix,
      }, {
          "salt" => dir.salt,
      });

      let content = required_text(child)?;
      dir.path = String::from(content);

      ConfigPart::Dir(dir)
    }
    "reset-dirs" => ConfigPart::ResetDirs,
    "remap-dir" => {
      let mut dir = RemapDir::default();

      parse_attrs!(child, {
          "prefix" => dir.prefix,
      }, {
          "salt" => dir.salt,
          "as-path" => dir.as_path,
      });

      let content = required_text(child)?;
      dir.path = String::from(content);

      ConfigPart::RemapDir(dir)
    }
    "cachedir" => {
      let mut dir = CacheDir::default();

      parse_attrs!(child, {
          "prefix" => dir.prefix,
      });

      let content = required_text(child)?;
      dir.path = String::from(content);

      ConfigPart::CacheDir(dir)
    }
    "include" => {
      let mut dir = Include::default();
      let mut ignore_missing = String::new();

      parse_attrs!(child, {
          "prefix" => dir.prefix,
      }, {
          "ignore_missing" => ignore_missing,
      });

      dir.ignore_missing = match ignore_missing.as_str() {
        "yes" => IncludeFailure::Ignore,
        _ => IncludeFailure::Report,
      };
      let content = required_text(child)?;
      dir.path = String::from(content);

      ConfigPart::Include(dir)
    }
    "config" => {
      let config = parsed_config(child)?;

      ConfigPart::Configuration(config)
    }
    "selectfont" => {
      let Ok(selected) = select_fonts(child);

      ConfigPart::SelectFont(selected)
    }
    "match" => {
      let rule = parsed_match(child)?;

      ConfigPart::Match(rule)
    }
    _ => {
      return Ok(None);
    }
  };

  Ok(Some(part))
}

fn parsed_match(node: Node) -> Result<Match> {
  let mut rule = Match::default();

  parse_attrs!(node, {
      "target" => rule.target,
  });

  let Ok(members) = node.children();

  for member in members {
    let Ok(tag) = member.tag_name();
    let Ok(name) = tag.name();

    match name {
      "test" => {
        let mut test = Test::default();
        let mut kind = PropertyKind::default();

        parse_attrs!(member, {
            "name" => kind,
            "qual" => test.qual,
            "target" => test.target,
            "compare" => test.compare,
        });

        let Ok(first_element) = member.first_element_child();
        let value =
          first_element.ok_or_else(|| Error::InvalidFormat(String::from("Empty test value")))?;
        let expression = parse_expr(value)?;
        let Ok(property) = kind.make_property(expression);
        test.value = property;

        rule.tests.push(test);
      }

      "edit" => {
        let mut edit = Edit::default();
        let mut kind = PropertyKind::default();

        parse_attrs!(member, {
            "name" => kind,
            "mode" => edit.mode,
            "binding" => edit.binding,
        });

        let Ok(first_element) = member.first_element_child();
        let value =
          first_element.ok_or_else(|| Error::InvalidFormat(String::from("Empty edit value")))?;
        let expression = parse_expr(value)?;
        let Ok(property) = kind.make_property(expression);
        edit.value = property;

        rule.edits.push(edit);
      }
      _ => {}
    }
  }

  Ok(rule)
}

fn parsed_config(node: Node) -> Result<Configuration> {
  let mut config = Configuration::default();
  let Ok(children) = node.children();

  for child in children {
    let Ok(tag) = child.tag_name();
    let Ok(name) = tag.name();
    let Ok(first) = child.first_element_child();

    match (name, first) {
      ("rescan", Some(interval)) => {
        let Ok(interval_tag) = interval.tag_name();
        let Ok(format) = interval_tag.name();

        match format {
          "int" => {
            let content = required_text(interval)?;
            let rescan = content.parse()?;

            config.rescans.push(rescan);
          }
          _ => {}
        }
      }
      ("blank", Some(rule)) => {
        let blank = parse_int_or_range(rule)?;

        config.blanks.push(blank);
      }
      (_, _) => {}
    }
  }

  Ok(config)
}

fn pattern_match(node: Node) -> core::result::Result<FontMatch, console_core_never::Never> {
  let Ok(children) = node.children();
  let properties = children.filter_map(|element| {
    let Ok(tag) = element.tag_name();
    let Ok(name) = tag.name();

    match name {
      "patelt" => {
        let mut kind = PropertyKind::default();
        parse_attrs_opt!(element, {
            "name" => kind,
        });
        let Ok(first) = element.first_element_child();
        let expression = first?;

        match parse_expr(expression) {
          Ok(expression_2) => {
            let Ok(property) = kind.make_property(expression_2);

            Some(property)
          }
          Err(_invalid_pattern) => None,
        }
      }
      _ => None,
    }
  });

  Ok(FontMatch::Pattern(properties.collect()))
}

fn select_fonts(node: Node) -> core::result::Result<SelectFont, console_core_never::Never> {
  let mut selected = SelectFont::default();
  let Ok(lists) = node.children();

  for list in lists {
    let Ok(children) = list.children();
    let matches = children.filter_map(|element| {
      let Ok(tag) = element.tag_name();
      let Ok(name) = tag.name();

      match name {
        "pattern" => {
          let Ok(matched) = pattern_match(element);

          Some(matched)
        }
        "glob" => {
          let Ok(content) = element.text();

          content.map(String::from).map(FontMatch::Glob)
        }
        _ => None,
      }
    });

    let Ok(list_tag) = list.tag_name();
    let Ok(list_name) = list_tag.name();

    match list_name {
      "acceptfont" => {
        selected.accepts.extend(matches);
      }
      "rejectfont" => {
        selected.rejects.extend(matches);
      }
      _ => {}
    }
  }

  Ok(selected)
}

fn parse_int_or_range(node: Node) -> Result<IntOrRange> {
  let mut texts = {
    let Ok(explicit_value) = get_texts(&node);
    explicit_value
  };

  let Ok(node_tag) = node.tag_name();
  let Ok(node_name) = node_tag.name();

  match node_name {
    "int" => {
      let content = required_text(node)?;
      let value = content.parse()?;
      Ok(IntOrRange::Int(value))
    }
    "range" => {
      let first_text = required_next(&mut texts, "Expect int")?;
      let first = first_text.parse()?;
      let last_text = required_next(&mut texts, "Expect int")?;
      let last = last_text.parse()?;
      Ok(IntOrRange::Range(first, last))
    }
    _ => Err(Error::InvalidFormat(String::from("Expect IntOrRange"))),
  }
}

fn parse_expr(node: Node) -> Result<Expression> {
  let Ok(descendants) = node.descendants();
  let nodes = descendants.filter(|node_2| {
    let Ok(element) = node_2.is_element();

    matches!(element, roxmltree::NodeMatch::Matches)
  });
  let mut computed = std::collections::HashMap::new();

  for current in nodes.rev() {
    let Ok(children) = current.children();
    let (remaining, expressions) = children
      .filter(|child| {
        let Ok(element) = child.is_element();

        matches!(element, roxmltree::NodeMatch::Matches)
      })
      .fold(
        (computed, Vec::new()),
        |(mut computed_2, mut expressions), child| {
          let Ok(id) = child.id();
          let expression = match computed_2.remove(&id) {
            Some(expression) => expression,
            None => Err(Error::InvalidFormat(String::from(
              "missing parsed expression",
            ))),
          };
          expressions.push(expression);
          (computed_2, expressions)
        },
      );
    computed = remaining;
    let expression = parse_one_expr(current, expressions.into_iter());
    let Ok(id) = current.id();
    let _previous_expression = computed.insert(id, expression);
  }

  let Ok(id) = node.id();

  match computed.remove(&id) {
    Some(expression) => expression,
    None => Err(Error::InvalidFormat(String::from(
      "missing parsed expression",
    ))),
  }
}

fn parse_one_expr(
  node: Node,
  mut exprs: impl Iterator<Item = Result<Expression>>,
) -> Result<Expression> {
  let mut texts = {
    let Ok(explicit_value) = get_texts(&node);
    explicit_value
  };

  let Ok(node_tag) = node.tag_name();
  let Ok(node_name) = node_tag.name();

  match node_name {
    "string" => {
      let content = required_text(node)?;

      Ok(Expression::from(Value::String(String::from(content))))
    }
    "langset" => {
      let content = required_text(node)?;

      Ok(Expression::from(Value::LangSet(String::from(content))))
    }
    "double" => {
      let content = required_text(node)?;
      let value = content.parse()?;
      Ok(Expression::from(Value::Double(value)))
    }
    "int" => {
      let content = required_text(node)?;
      let value = content.parse()?;
      Ok(Expression::from(Value::Int(value)))
    }
    "bool" => {
      let content = required_text(node)?;
      let value = content.parse()?;
      Ok(Expression::from(Value::Bool(value)))
    }
    "const" => {
      let content = required_text(node)?;
      let value = content.parse()?;
      Ok(Expression::from(Value::Constant(value)))
    }
    "matrix" => {
      let first = next_expression(&mut exprs)?;
      let second = next_expression(&mut exprs)?;
      let third = next_expression(&mut exprs)?;
      let fourth = next_expression(&mut exprs)?;
      Ok(Expression::Matrix(Box::new([first, second, third, fourth])))
    }
    "charset" => {
      let charset = ({
        let Ok(explicit_value) = node.children();
        explicit_value
      })
      .filter_map(|child| match parse_int_or_range(child) {
        Ok(range) => Some(range),
        Err(_invalid_range) => None,
      })
      .collect();

      Ok(Expression::from(Value::CharSet(charset)))
    }
    "range" => {
      let first_text = required_next(&mut texts, "Expect expression")?;
      let first = first_text.parse()?;
      let last_text = required_next(&mut texts, "Expect expression")?;
      let last = last_text.parse()?;
      Ok(Expression::from(Value::Range(first, last)))
    }
    "name" => {
      let mut target = PropertyTarget::default();
      parse_attrs!(node, {
          "target" => target,
      });
      let content = required_text(node)?;
      let kind = content.parse()?;

      Ok(Expression::from(Value::Property(target, kind)))
    }
    name => parse_operator(node, name, exprs),
  }
}

fn parse_operator(
  node: Node,
  name: &str,
  mut exprs: impl Iterator<Item = Result<Expression>>,
) -> Result<Expression> {
  match name.parse() {
    Ok(list_op) => {
      let expressions = exprs.collect::<Result<Vec<_>>>()?;
      Ok(Expression::List(list_op, expressions))
    }
    Err(_not_list) => match name.parse() {
      Ok(unary_op) => {
        let expression = next_expression(&mut exprs)?;
        Ok(Expression::Unary(unary_op, Box::new(expression)))
      }
      Err(_not_unary) => match name.parse() {
        Ok(binary_op) => {
          let first = next_expression(&mut exprs)?;
          let second = next_expression(&mut exprs)?;
          Ok(Expression::Binary(binary_op, Box::new([first, second])))
        }
        Err(_not_binary) => match name.parse() {
          Ok(ternary_op) => {
            let first = next_expression(&mut exprs)?;
            let second = next_expression(&mut exprs)?;
            let third = next_expression(&mut exprs)?;
            Ok(Expression::Ternary(
              ternary_op,
              Box::new([first, second, third]),
            ))
          }
          Err(_not_ternary) => Err(Error::InvalidFormat(format!(
            "Unknown expression: {:?}",
            ({
              let Ok(explicit_value) = node.tag_name();
              explicit_value
            }),
          ))),
        },
      },
    },
  }
}

fn get_texts<'a>(
  node: &'a Node,
) -> core::result::Result<impl Iterator<Item = &'a str>, console_core_never::Never> {
  Ok(
    ({
      let Ok(explicit_value) = node.children();
      explicit_value
    })
    .filter_map(|n| {
      let Ok(element) = n.is_element();

      match element {
        roxmltree::NodeMatch::Matches => {
          let Ok(explicit_value) = n.text();
          explicit_value
        }
        roxmltree::NodeMatch::Different => None,
      }
    }),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
      explicit080_macro_budget,
      reason = "the XML and parser are the only differences between these named rejection tests; the macro supplies the parse harness and failure assertion"
    )
  )]
  macro_rules! make_parse_failed_test {
    ($name:ident, $test_fn:ident, $text:expr,) => {
      #[test]
      fn $name() {
        let attempted = (|text: &str| -> crate::Result<_> {
          let doc = roxmltree::Document::parse(text).map_err(Error::Xml)?;
          let node = doc.root_element().map_err(Error::Xml)?;

          $test_fn(node)
        })($text);

        assert!(matches!(attempted, Err(_refusal)));
      }
    };
  }

  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
      explicit080_macro_budget,
      reason = "each named success test supplies XML, a parser and its expected value; the macro keeps document and root setup identical across cases"
    )
  )]
  macro_rules! make_parse_test {
    ($name:ident, $test_fn:ident, $text:expr, $value:expr,) => {
      #[test]
      fn $name() -> crate::Result<()> {
        let doc = roxmltree::Document::parse($text).map_err(Error::Xml)?;
        let node = doc.root_element().map_err(Error::Xml)?;
        let ret = $test_fn(node)?;
        let expected = $value;

        assert_eq!(expected, ret);

        Ok(())
      }
    };
  }

  make_parse_test!(
    test_parse_charset,
    parse_expr,
    "<charset><range><int>0</int><int>123</int></range></charset>",
    Expression::from(vec![IntOrRange::Range(0, 123)]),
  );

  make_parse_test!(
    test_parse_int,
    parse_expr,
    "<int>123</int>",
    Expression::from(123),
  );

  make_parse_failed_test!(test_parse_invalid_int, parse_expr, "<int>123f</int>",);

  make_parse_test!(
    test_parse_range,
    parse_expr,
    "<range><int>0</int><int>10</int></range>",
    Expression::from(Value::Range(0, 10)),
  );

  make_parse_failed_test!(
    test_parse_invalid_range,
    parse_expr,
    "<range>0<int>10</int></range>",
  );

  make_parse_test!(
    test_langset,
    parse_expr,
    "<langset>ko-KR</langset>",
    Expression::from(Value::LangSet(String::from("ko-KR"))),
  );
}
