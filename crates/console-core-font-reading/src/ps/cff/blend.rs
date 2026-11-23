//! Support for the dictionary and charstring blend operator.

use font_types::{BigEndian, F2Dot14, Fixed};

use crate::ps::error::Error;
use crate::tables::variations::{ItemVariationData, ItemVariationStore};

pub struct BlendState<'a> {
  store: ItemVariationStore<'a>,
  coords: &'a [F2Dot14],
  store_index: u16,

  data: Option<ItemVariationData<'a>>,
  region_indices: &'a [BigEndian<u16>],
  scalars: [Fixed; 16],
}

impl<'a> BlendState<'a> {
  pub fn new(
    store: ItemVariationStore<'a>,
    coords: &'a [F2Dot14],
    store_index: u16,
  ) -> Result<Self, Error> {
    let mut state = Self {
      store,
      coords,
      store_index,
      data: None,
      region_indices: &[],
      scalars: Default::default(),
    };
    state.update_precomputed_scalars()?;
    Ok(state)
  }

  pub fn set_store_index(&mut self, store_index: u16) -> Result<(), Error> {
    match ::core::primitive::bool::then_some(self.store_index != store_index, ()) {
      ::core::option::Option::Some(()) => {
        self.store_index = store_index;
        self.update_precomputed_scalars()?;
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  pub fn region_count(&self) -> Result<u64, Error> {
    let Ok(value_2) = console_core_number_conversion::fitted::<_, u64>(self.region_indices.len());

    Ok(value_2)
  }

  pub fn scalars(&self) -> Result<impl Iterator<Item = Result<Fixed, Error>> + '_, Error> {
    let cached = self
      .scalars
      .iter()
      .copied()
      .take(self.region_indices.len())
      .map(Ok);
    let remaining = self.region_indices.iter().skip(self.scalars.len());

    Ok(cached.chain(remaining.map(|region_index| {
      let Ok(index) = region_index.get();
      self.region_scalar(index)
    })))
  }

  fn update_precomputed_scalars(&mut self) -> Result<(), Error> {
    self.data = None;
    self.region_indices = &[];
    let store = &self.store;
    let Ok(variation_data) = store.item_variation_data();
    let data = variation_data.get(u64::from(self.store_index))?;
    let data_2 = data.ok_or(Error::InvalidVariationStoreIndex(self.store_index))?;
    let Ok(region_indices) = data_2.region_indexes();
    let regions = {
      let variation_region_list = self.store.variation_region_list()?;
      let Ok(explicit_value) = variation_region_list.variation_regions();
      explicit_value
    };

    for (region_ix, scalar) in region_indices.iter().zip(&mut self.scalars) {
      let Ok(value_4) = region_ix.get();

      let region = regions.get({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_4);
        migrated_value
      })?;
      *scalar = region.compute_scalar(self.coords);
    }

    self.data = Some(data_2);
    self.region_indices = region_indices;
    Ok(())
  }

  fn region_scalar(&self, index: u16) -> Result<Fixed, Error> {
    let map_err = ({
      let variation_region_list = self.store.variation_region_list()?;
      let Ok(explicit_value) = variation_region_list.variation_regions();
      explicit_value
    })
    .get({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index);
      migrated_value
    })
    .map_err(Error::Read)?;

    Ok(map_err.compute_scalar(self.coords))
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::{FontData, FontRead};

  #[test]
  fn example_blends() -> Result<(), crate::test_support::Failure> {
    for (coord, expected) in [
      (-1.0, [0.0, 1.0]),
      (-0.25, [0.5, 0.0]),
      (-0.5, [1.0, 0.0]),
      (-0.75, [0.5, 0.5]),
      (0.0, [0.0, 0.0]),
      (0.5, [0.0, 0.0]),
      (1.0, [0.0, 0.0]),
    ] {
      let variation_bytes = font_test_data::cff2::EXAMPLE
        .get(18..)
        .ok_or(crate::ReadError::OutOfBounds)?;
      let Ok(variation_data) = FontData::new(variation_bytes);
      let store = ItemVariationStore::read(variation_data)?;
      let Ok(coordinate) = F2Dot14::from_f32(coord);
      let coordinates = [coordinate];
      let blender = BlendState::new(store, &coordinates, 0)?;
      let scalars = blender.scalars()?;
      let actual = scalars.collect::<Result<Vec<_>, _>>()?;
      let expected_scalars = expected.map(|value| {
        let Ok(scalar) = Fixed::from_f64(value);
        scalar
      });
      assert_eq!(actual, expected_scalars);
    }

    Ok(())
  }
}
