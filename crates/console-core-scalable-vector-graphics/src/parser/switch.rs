use super::svgtree::{AId, SvgNode};
use super::{Options, converter};
use crate::{Group, Node};

static FEATURES: &[&str] = &[
  "http://www.w3.org/TR/SVG11/feature#SVGDOM-static",
  "http://www.w3.org/TR/SVG11/feature#SVG-static",
  "http://www.w3.org/TR/SVG11/feature#CoreAttribute",
  "http://www.w3.org/TR/SVG11/feature#Structure",
  "http://www.w3.org/TR/SVG11/feature#BasicStructure",
  "http://www.w3.org/TR/SVG11/feature#ContainerAttribute",
  "http://www.w3.org/TR/SVG11/feature#ConditionalProcessing",
  "http://www.w3.org/TR/SVG11/feature#Image",
  "http://www.w3.org/TR/SVG11/feature#Style",
  "http://www.w3.org/TR/SVG11/feature#Shape",
  "http://www.w3.org/TR/SVG11/feature#Text",
  "http://www.w3.org/TR/SVG11/feature#BasicText",
  "http://www.w3.org/TR/SVG11/feature#PaintAttribute",
  "http://www.w3.org/TR/SVG11/feature#BasicPaintAttribute",
  "http://www.w3.org/TR/SVG11/feature#OpacityAttribute",
  "http://www.w3.org/TR/SVG11/feature#GraphicsAttribute",
  "http://www.w3.org/TR/SVG11/feature#BasicGraphicsAttribute",
  "http://www.w3.org/TR/SVG11/feature#Marker",
  "http://www.w3.org/TR/SVG11/feature#Gradient",
  "http://www.w3.org/TR/SVG11/feature#Pattern",
  "http://www.w3.org/TR/SVG11/feature#Clip",
  "http://www.w3.org/TR/SVG11/feature#BasicClip",
  "http://www.w3.org/TR/SVG11/feature#Mask",
  "http://www.w3.org/TR/SVG11/feature#Filter",
  "http://www.w3.org/TR/SVG11/feature#BasicFilter",
  "http://www.w3.org/TR/SVG11/feature#XlinkAttribute",
];

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The selected switch child enters the shared depth guard in convert_element"
  )
)]
pub(crate) fn convert(
  node: SvgNode,
  state: &converter::State,
  cache: &mut converter::Cache,
  parent: &mut Group,
) -> core::result::Result<Option<()>, console_core_never::Never> {
  let Ok(mut children) = node.children();
  let child = match children.find(|n| {
    matches!(
      {
        let Ok(explicit_value) = is_condition_passed(*n, state.opt);
        explicit_value
      },
      crate::IsConditionPassedAnswer::Yes
    )
  }) {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(convert_group) =
    converter::convert_group(node, state, converter::GroupRequirement::AsNeeded, cache, parent, &|cache_2, g| {
      let Ok(()) = converter::convert_element(child, state, cache_2, g);
    });

  match convert_group {
    Some(g) => {
      parent.children.push(Node::Group(Box::new(g)));
    }
    None => {}
  }

  Ok(Some(()))
}

pub(crate) fn is_condition_passed(
  node: SvgNode,
  opt: &Options,
) -> core::result::Result<crate::IsConditionPassedAnswer, console_core_never::Never> {
  use crate::{IsConditionPassedAnswer, IsValidSysLangAnswer, NodeMatch, Presence};

  match node.is_element() {
    Ok(NodeMatch::Matches) => {}
    Ok(NodeMatch::Different) => return Ok(IsConditionPassedAnswer::No),
  }

  match node.has_attribute(AId::RequiredExtensions) {
    Ok(Presence::Present) => return Ok(IsConditionPassedAnswer::No),
    Ok(Presence::Absent) => {}
  }

  let Ok(features) = node.attribute::<&str>(AId::RequiredFeatures);
  let unsupported = features.and_then(|list| list.split(' ').find(|feature| !FEATURES.contains(feature)));

  match unsupported {
    Some(_) => return Ok(IsConditionPassedAnswer::No),
    None => {}
  }

  match is_valid_sys_lang(node, opt) {
    Ok(IsValidSysLangAnswer::Yes) => Ok(IsConditionPassedAnswer::Yes),
    Ok(IsValidSysLangAnswer::No) => Ok(IsConditionPassedAnswer::No),
  }
}

fn is_valid_sys_lang(
  node: SvgNode,
  opt: &Options,
) -> Result<crate::IsValidSysLangAnswer, console_core_never::Never> {
  use crate::IsValidSysLangAnswer;

  let Ok(authored) = node.attribute::<&str>(AId::SystemLanguage);
  let list = match authored {
    Some(languages) => languages,
    None => return Ok(IsValidSysLangAnswer::Yes),
  };
  let configured: std::collections::HashSet<_> = opt.languages.iter().map(String::as_str).collect();

  for language in list.split(',').map(str::trim) {
    match configured.get(language) {
      Some(_) => return Ok(IsValidSysLangAnswer::Yes),
      None => {}
    }

    let prefix = language.split_once('-').and_then(|(prefix, _)| configured.get(prefix));

    match prefix {
      Some(_) => return Ok(IsValidSysLangAnswer::Yes),
      None => {}
    }
  }

  Ok(IsValidSysLangAnswer::No)
}
