use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use core::num::NonZeroU32;

const LOW_HALF: u64 = 0xffff_ffff;

#[non_exhaustive]
pub(crate) struct Hashes {
  pub(crate) g: u32,
  pub(crate) f1: u32,
  pub(crate) f2: u32,
}

#[inline]
pub(crate) fn hash<T>(x: &T, key: u64) -> Result<Hashes, Never>
where
  T: ?Sized + Hash,
{
  let (upper, lower) = x.phf_hash(key);
  let Ok(g) = fitted::<u64, u32>(lower.wrapping_shr(32));
  let Ok(f1) = fitted::<u64, u32>(lower & LOW_HALF);

  Ok(Hashes { g, f1, f2: upper })
}

#[inline]
pub(crate) fn get_unwrapped_index(
  hashes: &Hashes,
  disps: &[(u32, u32)],
) -> Result<Option<u32>, Never> {
  let Ok(buckets) = fitted::<_, u32>(disps.len());

  Ok(match NonZeroU32::new(buckets) {
    None => None,
    Some(many) => {
      let Ok(bucket) = index(hashes.g % many);

      disps.get(bucket).map(|&(d1, d2)| {
        d2.wrapping_add(hashes.f1.wrapping_mul(d1))
          .wrapping_add(hashes.f2)
      })
    }
  })
}

pub trait Hash {
  fn phf_hash(&self, key: u64) -> (u32, u64);
}

impl<T> Hash for &'_ T
where
  T: ?Sized + Hash,
{
  #[inline]
  fn phf_hash(&self, key: u64) -> (u32, u64) {
    (**self).phf_hash(key)
  }
}

impl Hash for [u8] {
  #[inline]
  fn phf_hash(&self, key: u64) -> (u32, u64) {
    const A: u32 = 2024877429;
    const B: u64 = 17099814477566751079;
    let Ok(low) = fitted::<u64, u32>(key & LOW_HALF);

    self.iter().fold((low, key), |(a, b), &x| {
      (
        (a ^ u32::from(x)).wrapping_mul(A),
        (b ^ u64::from(x)).wrapping_mul(B),
      )
    })
  }
}

impl Hash for str {
  #[inline]
  fn phf_hash(&self, key: u64) -> (u32, u64) {
    self.as_bytes().phf_hash(key)
  }
}

impl Hash for u32 {
  #[inline]
  fn phf_hash(&self, key: u64) -> (u32, u64) {
    const A: u32 = 2024877429;
    const B: u64 = 17099814477566751079;
    let Ok(low) = fitted::<u64, u32>(key & LOW_HALF);
    let a = (low ^ *self).wrapping_mul(A);
    let b = (key ^ u64::from(*self)).wrapping_mul(B);
    (a, b)
  }
}

impl Hash for char {
  #[inline]
  fn phf_hash(&self, key: u64) -> (u32, u64) {
    u32::from(*self).phf_hash(key)
  }
}
