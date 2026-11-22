//! Progression iterators, defined in Section B.12.
//!
//! A progression iterator essentially yields tuples of
//! (`layer_num`, resolution, component, precinct) in a specific order that
//! determines in which order the data appears in the codestream.

use alloc::vec;

use super::tile::{ComponentTile, ResolutionTile, Tile};
use console_core_number_conversion::{index, wrapped};
use core::cmp::Ordering;
use core::ops::Range;

#[derive(Default, Copy, Clone, Debug, PartialEq, Hash, Eq)]
pub(crate) struct ProgressionData {
  pub(crate) layer_num: u8,
  pub(crate) resolution: u8,
  pub(crate) component: u8,
  pub(crate) precinct: u64,
}

pub(crate) struct IteratorInput<'a> {
  layers: Range<u8>,
  tile: &'a Tile<'a>,
  resolutions: Range<u8>,
  components: Range<u8>,
}

impl<'a> IteratorInput<'a> {
  pub(crate) fn new(tile: &'a Tile<'a>) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(iterator_input) =
      Self::new_with_custom_bounds(tile, 0..u8::MAX, 0..u8::MAX, 0..u8::MAX);

    Ok(iterator_input)
  }

  pub(crate) fn new_with_custom_bounds(
    tile: &'a Tile<'a>,
    mut resolutions: Range<u8>,
    mut layers: Range<u8>,
    mut components: Range<u8>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let max_resolution = match tile
      .component_infos
      .iter()
      .map(|c| c.coding_style.parameters.num_resolution_levels)
      .max()
    {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0,
    };
    let max_layer = tile.num_layers;
    let Ok(max_component) =
      console_core_number_conversion::wrapped::<_, u8>(tile.component_infos.len());

    resolutions.end = resolutions.end.min(max_resolution);
    layers.end = layers.end.min(max_layer);
    components.end = components.end.min(max_component);

    Ok(Self {
      layers,
      tile,
      resolutions,
      components,
    })
  }
}

fn precinct_count<'a>(
  tile: &'a Tile<'a>,
  packet: ComponentResolution,
) -> core::result::Result<u64, console_core_never::Never> {
  let Ok(position) = index(packet.component);
  let info = match tile.component_infos.get(position) {
    Some(info) => info,
    None => return Ok(0),
  };
  let Ok(component_tile) = ComponentTile::new(tile, info);
  let resolution = packet.resolution;
  let Ok(max_resolution) = info.num_resolution_levels();

  match resolution.cmp(&max_resolution) {
    Ordering::Less => {
      let Ok(resolution_tile) = ResolutionTile::new(component_tile, resolution);
      let Ok(count) = resolution_tile.num_precincts();
      Ok(count)
    }
    Ordering::Equal | Ordering::Greater => Ok(0),
  }
}

struct ComponentResolution {
  component: u8,
  resolution: u8,
}

pub(crate) fn layer_resolution_component_position_progression<'a>(
  input: IteratorInput<'a>,
) -> core::result::Result<impl Iterator<Item = ProgressionData> + 'a, console_core_never::Never> {
  let components = input.components.clone();
  let resolutions = input.resolutions.clone();
  let layers = input.layers.clone();

  Ok(layers.flat_map(move |layer_num| {
    let source = input.tile;
    let components_2 = components.clone();

    resolutions.clone().flat_map(move |resolution| {
      components_2.clone().flat_map(move |component| {
        let Ok(count) = precinct_count(
          source,
          ComponentResolution {
            component,
            resolution,
          },
        );
        (0..count).map(move |precinct| ProgressionData {
          layer_num,
          resolution,
          component,
          precinct,
        })
      })
    })
  }))
}

pub(crate) fn resolution_layer_component_position_progression<'a>(
  input: IteratorInput<'a>,
) -> core::result::Result<impl Iterator<Item = ProgressionData> + 'a, console_core_never::Never> {
  let components = input.components.clone();
  let resolutions = input.resolutions.clone();
  let layers = input.layers.clone();

  Ok(resolutions.flat_map(move |resolution| {
    let source = input.tile;
    let components_2 = components.clone();

    layers.clone().flat_map(move |layer_num| {
      components_2.clone().flat_map(move |component| {
        let Ok(count) = precinct_count(
          source,
          ComponentResolution {
            component,
            resolution,
          },
        );
        (0..count).map(move |precinct| ProgressionData {
          layer_num,
          resolution,
          component,
          precinct,
        })
      })
    })
  }))
}

struct PrecinctStore {
  resolution: u8,
  precinct_y: u32,
  precinct_x: u32,
  component_idx: u8,
  precinct_idx: u64,
}

fn position_progression_common<'a>(
  input: IteratorInput<'a>,
  sort: impl FnMut(&PrecinctStore, &PrecinctStore) -> Ordering,
) -> core::result::Result<
  Option<impl Iterator<Item = ProgressionData> + 'a>,
  console_core_never::Never,
> {
  let mut elements = vec![];

  let Ok(component_tiles) = input.tile.component_tiles();

  let Ok(min_component) = index(input.components.start);
  let Ok(component_count) = index(input.components.end.saturating_sub(input.components.start));
  let Ok(min_resolution) = index(input.resolutions.start);
  let Ok(resolution_count) = index(
    input
      .resolutions
      .end
      .saturating_sub(input.resolutions.start),
  );

  for (component_idx, component) in component_tiles
    .enumerate()
    .skip(min_component)
    .take(component_count)
  {
    let Ok(resolution_tiles) = component.resolution_tiles();

    for (resolution, resolution_tile) in resolution_tiles
      .enumerate()
      .skip(min_resolution)
      .take(resolution_count)
    {
      let Ok(precincts) = resolution_tile.precincts();
      elements.extend(
        (match precincts {
          Some(value) => value,
          None => return Ok(None),
        })
        .map(|d| PrecinctStore {
          precinct_y: d.r_y,
          precinct_x: d.r_x,
          component_idx: (match wrapped::<_, u8>(component_idx) {
            Ok(converted) => converted,
          }),
          resolution: (match wrapped::<_, u8>(resolution) {
            Ok(converted) => converted,
          }),
          precinct_idx: d.idx,
        }),
      );
    }
  }

  elements.sort_by(sort);

  Ok(Some(elements.into_iter().flat_map(move |e| {
    input.layers.clone().map(move |layer| ProgressionData {
      layer_num: layer,
      resolution: e.resolution,
      component: e.component_idx,
      precinct: e.precinct_idx,
    })
  })))
}

pub(crate) fn resolution_position_component_layer_progression<'a>(
  input: IteratorInput<'a>,
) -> core::result::Result<
  Option<impl Iterator<Item = ProgressionData> + 'a>,
  console_core_never::Never,
> {
  Ok(
    match position_progression_common(input, |p, s| {
      p.resolution
        .cmp(&s.resolution)
        .then_with(|| p.precinct_y.cmp(&s.precinct_y))
        .then_with(|| p.precinct_x.cmp(&s.precinct_x))
        .then_with(|| p.component_idx.cmp(&s.component_idx))
        .then_with(|| p.precinct_idx.cmp(&s.precinct_idx))
    }) {
      Ok(value) => value,
    },
  )
}

pub(crate) fn position_component_resolution_layer_progression<'a>(
  input: IteratorInput<'a>,
) -> core::result::Result<
  Option<impl Iterator<Item = ProgressionData> + 'a>,
  console_core_never::Never,
> {
  Ok(
    match position_progression_common(input, |p, s| {
      p.precinct_y
        .cmp(&s.precinct_y)
        .then_with(|| p.precinct_x.cmp(&s.precinct_x))
        .then_with(|| p.component_idx.cmp(&s.component_idx))
        .then_with(|| p.resolution.cmp(&s.resolution))
        .then_with(|| p.precinct_idx.cmp(&s.precinct_idx))
    }) {
      Ok(value) => value,
    },
  )
}

pub(crate) fn component_position_resolution_layer_progression<'a>(
  input: IteratorInput<'a>,
) -> core::result::Result<
  Option<impl Iterator<Item = ProgressionData> + 'a>,
  console_core_never::Never,
> {
  Ok(
    match position_progression_common(input, |p, s| {
      p.component_idx
        .cmp(&s.component_idx)
        .then_with(|| p.precinct_y.cmp(&s.precinct_y))
        .then_with(|| p.precinct_x.cmp(&s.precinct_x))
        .then_with(|| p.resolution.cmp(&s.resolution))
        .then_with(|| p.precinct_idx.cmp(&s.precinct_idx))
    }) {
      Ok(value) => value,
    },
  )
}
