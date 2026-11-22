use crate::{Document, IdOverflow, NodeId, NodeType};
use alloc::vec::Vec;

#[test]
fn every_document_has_a_root_and_keeps_node_order() -> Result<(), crate::test_support::Failure> {
  let doc = Document::parse("<root><a>x</a><!--comment--><?p data?><b/></root>")?;
  let root = doc.root_element()?;
  let Ok(descendants) = doc.descendants();
  let ids: Vec<_> = descendants
    .map(|node| {
      let Ok(id) = node.id();
      let Ok(position) = id.get();

      position
    })
    .collect();
  let Ok(children) = root.children();
  let kinds: Vec<_> = children
    .map(|node| {
      let Ok(kind) = node.node_type();

      kind
    })
    .collect();

  {
    let left = &(ids);
    let right = &([0, 1, 2, 3, 4, 5, 6]);

    crate::test_support::equal(left, right, line!())?;
  };

  {
    let left = &(kinds);

    let right = &([
      NodeType::Element,
      NodeType::Comment,
      NodeType::PI,
      NodeType::Element,
    ]);
    crate::test_support::equal(left, right, line!())?;
  };

  Ok(())
}

#[test]
fn namespaces_keep_their_uris_when_an_earlier_prefix_is_inserted()
-> Result<(), crate::test_support::Failure> {
  let doc = Document::parse(
    r#"<z:root xmlns:z="urn:z&#49;" xmlns:a="urn:a"><a:child/><z:child xmlns:z="urn:z1"/></z:root>"#,
  )?;
  let root = doc.root_element()?;
  let Ok(elements) = root.descendants();
  let namespaces: Vec<_> = elements
    .map(|element| {
      let Ok(tag) = element.tag_name();
      let Ok(namespace) = tag.namespace();

      namespace
    })
    .collect();

  crate::test_support::equal(
    &namespaces,
    &[Some("urn:z1"), Some("urn:a"), Some("urn:z1")],
    line!(),
  )
}

#[test]
fn descendants_support_mixed_forward_and_reverse_steps()
-> Result<(), crate::test_support::Failure> {
  let doc = Document::parse("<root><a/><b/><c/></root>")?;
  let root = doc.root_element()?;
  let Ok(mut descendants) = root.descendants();
  let first = descendants.next().map(|node| {
    let Ok(name) = node.tag_name();
    let Ok(local) = name.name();

    local
  });
  let last = descendants.next_back().map(|node| {
    let Ok(name) = node.tag_name();
    let Ok(local) = name.name();

    local
  });
  let next = descendants.nth(1).map(|node| {
    let Ok(name) = node.tag_name();
    let Ok(local) = name.name();

    local
  });

  {
    let left = &(first);
    let right = &(Some("root"));

    crate::test_support::equal(left, right, line!())?;
  };

  {
    let left = &(last);
    let right = &(Some("c"));

    crate::test_support::equal(left, right, line!())?;
  };

  {
    let left = &(next);
    let right = &(Some("b"));

    crate::test_support::equal(left, right, line!())?;
  };

  crate::test_support::Extract::error(descendants.next(), line!())?;

  Ok(())
}

#[test]
fn identifiers_report_the_reserved_overflow_value()
-> core::result::Result<(), crate::test_support::Failure> {
  crate::test_support::equal(&NodeId::new(u32::MAX), &Err(IdOverflow), line!())
}

#[test]
fn deeply_nested_elements_walk_and_drop_without_recursion()
-> Result<(), crate::test_support::Failure> {
  let depth = 4096u64;
  let mut xml = alloc::string::String::new();

  for _level in 0..depth {
    xml.push_str("<n>");
  }

  for _level in 0..depth {
    xml.push_str("</n>");
  }

  let doc = Document::parse(&xml)?;
  let Ok(descendants) = doc.descendants();
  let Ok(count) = console_core_number_conversion::fitted::<_, u64>(descendants.count());

  {
    let left = &(count);
    let right = &(depth.saturating_add(1));

    crate::test_support::equal(left, right, line!())?;
  };

  #[cfg(feature = "std")]
  std::thread::scope(|scope| {
    let worker = std::thread::Builder::new()
      .stack_size(131_072usize)
      .spawn_scoped(scope, move || drop(doc))
      .map_err(|_| crate::test_support::Failure::UnexpectedError { line: line!() })?;
    #[cfg_attr(
      dylint_lib = "explicit057_no_unbounded_wait",
      expect(
        explicit057_no_unbounded_wait,
        reason = "the worker drops a fixed in-memory tree with no I/O or locks, and the test must collect the destructor's result"
      )
    )]
    let dropped = worker.join();
    dropped.map_err(|_| crate::test_support::Failure::UnexpectedError { line: line!() })
  })?;

  Ok(())
}
