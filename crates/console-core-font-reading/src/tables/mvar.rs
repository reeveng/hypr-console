//! The [MVAR (Metrics Variation)](https://docs.microsoft.com/en-us/typography/opentype/spec/mvar) table

use super::variations::{DeltaSetIndex, ItemVariationStore};

pub mod tags {
  use font_types::Tag;

  pub const HASC: Tag = {
    let Ok(value) = Tag::new(b"hasc");
    value
  };
  pub const HDSC: Tag = {
    let Ok(value) = Tag::new(b"hdsc");
    value
  };
  pub const HLGP: Tag = {
    let Ok(value) = Tag::new(b"hlgp");
    value
  };

  pub const HCLA: Tag = {
    let Ok(value) = Tag::new(b"hcla");
    value
  };
  pub const HCLD: Tag = {
    let Ok(value) = Tag::new(b"hcld");
    value
  };

  pub const VASC: Tag = {
    let Ok(value) = Tag::new(b"vasc");
    value
  };
  pub const VDSC: Tag = {
    let Ok(value) = Tag::new(b"vdsc");
    value
  };
  pub const VLGP: Tag = {
    let Ok(value) = Tag::new(b"vlgp");
    value
  };

  pub const HCRS: Tag = {
    let Ok(value) = Tag::new(b"hcrs");
    value
  };
  pub const HCRN: Tag = {
    let Ok(value) = Tag::new(b"hcrn");
    value
  };
  pub const HCOF: Tag = {
    let Ok(value) = Tag::new(b"hcof");
    value
  };

  pub const VCRS: Tag = {
    let Ok(value) = Tag::new(b"vcrs");
    value
  };
  pub const VCRN: Tag = {
    let Ok(value) = Tag::new(b"vcrn");
    value
  };
  pub const VCOF: Tag = {
    let Ok(value) = Tag::new(b"vcof");
    value
  };

  pub const XHGT: Tag = {
    let Ok(value) = Tag::new(b"xhgt");
    value
  };
  pub const CPHT: Tag = {
    let Ok(value) = Tag::new(b"cpht");
    value
  };

  pub const SBXO: Tag = {
    let Ok(value) = Tag::new(b"sbxo");
    value
  };
  pub const SBYO: Tag = {
    let Ok(value) = Tag::new(b"sbyo");
    value
  };
  pub const SBXS: Tag = {
    let Ok(value) = Tag::new(b"sbxs");
    value
  };
  pub const SBYS: Tag = {
    let Ok(value) = Tag::new(b"sbys");
    value
  };

  pub const SPXO: Tag = {
    let Ok(value) = Tag::new(b"spxo");
    value
  };
  pub const SPYO: Tag = {
    let Ok(value) = Tag::new(b"spyo");
    value
  };
  pub const SPXS: Tag = {
    let Ok(value) = Tag::new(b"spxs");
    value
  };
  pub const SPYS: Tag = {
    let Ok(value) = Tag::new(b"spys");
    value
  };

  pub const STRS: Tag = {
    let Ok(value) = Tag::new(b"strs");
    value
  };
  pub const STRO: Tag = {
    let Ok(value) = Tag::new(b"stro");
    value
  };

  pub const UNDS: Tag = {
    let Ok(value) = Tag::new(b"unds");
    value
  };
  pub const UNDO: Tag = {
    let Ok(value) = Tag::new(b"undo");
    value
  };

  pub const GSP0: Tag = {
    let Ok(value) = Tag::new(b"gsp0");
    value
  };
  pub const GSP1: Tag = {
    let Ok(value) = Tag::new(b"gsp1");
    value
  };
  pub const GSP2: Tag = {
    let Ok(value) = Tag::new(b"gsp2");
    value
  };
  pub const GSP3: Tag = {
    let Ok(value) = Tag::new(b"gsp3");
    value
  };
  pub const GSP4: Tag = {
    let Ok(value) = Tag::new(b"gsp4");
    value
  };
  pub const GSP5: Tag = {
    let Ok(value) = Tag::new(b"gsp5");
    value
  };
  pub const GSP6: Tag = {
    let Ok(value) = Tag::new(b"gsp6");
    value
  };
  pub const GSP7: Tag = {
    let Ok(value) = Tag::new(b"gsp7");
    value
  };
  pub const GSP8: Tag = {
    let Ok(value) = Tag::new(b"gsp8");
    value
  };
  pub const GSP9: Tag = {
    let Ok(value) = Tag::new(b"gsp9");
    value
  };
}

include!("../../generated/generated_mvar.rs");

impl Mvar<'_> {
  pub fn metric_delta(&self, tag: Tag, coords: &[F2Dot14]) -> Result<Fixed, ReadError> {
    use std::cmp::Ordering;
    let Ok(records) = self.value_records();
    let mut lo = 0u64;
    let Ok(mut hi) = console_core_number_conversion::fitted::<_, u64>(records.len());

    for _ in 0..u16::BITS {
      match lo.cmp(&hi) {
        Ordering::Less => {}
        Ordering::Equal | Ordering::Greater => break,
      }

      let i = lo.midpoint(hi);
      let Ok(index) = console_core_number_conversion::index(i);
      let record = records.get(index).ok_or(ReadError::OutOfBounds)?;
      let Ok(value_tag) = record.value_tag();

      match tag.cmp(&value_tag) {
        Ordering::Less => {
          hi = i;
        }
        Ordering::Greater => {
          lo = i.wrapping_add(1);
        }
        Ordering::Equal => {
          let item_variation_store = self.item_variation_store()?;
          let ivs = item_variation_store.ok_or(ReadError::NullOffset)?;
          let Ok(outer) = record.delta_set_outer_index();
          let Ok(inner) = record.delta_set_inner_index();
          let compute_delta = ivs.compute_delta(DeltaSetIndex { outer, inner }, coords)?;
          let Ok(value_1) = Fixed::from_i32(compute_delta);

          return Ok(value_1);
        }
      }
    }

    Err(ReadError::MetricIsMissing(tag))
  }
}
