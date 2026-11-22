//! The cache a page's objects are read through, keyed by a hash of the bytes they were read from.
//!
//! A value is made outside the cache lock so a constructor can look up another object.
//! A device may decode an image here without waiting for a cache entry another caller is storing.

use crate::util::hash128;
use console_core_never::Never;
use console_core_portable_document_syntax::object::{
  Array, Dict, MaybeRef, Name, Null, ObjRef, Object, Stream,
};
use kurbo::Affine;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

type CacheMap = HashMap<u128, Option<Box<dyn Any + Send + Sync>>>;

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "deferred image callbacks retain the object cache independently of the document's font lifetime"
  )
)]
pub(crate) struct Cache(Arc<Mutex<CacheMap>>);

pub(crate) enum Cached<T> {
  Present(Option<T>),
  Absent,
}

impl Cache {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the document creates one object cache for its deferred image callbacks"
    )
  )]
  pub(crate) fn new() -> Result<Self, Never> {
    Ok(Self(Arc::new(Mutex::new(HashMap::new()))))
  }

  fn available(&self) -> Result<Option<MutexGuard<'_, CacheMap>>, Never> {
    Ok(match self.0.try_lock() {
      Ok(locked) => Some(locked),
      Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
      Err(TryLockError::WouldBlock) => None,
    })
  }

  pub(crate) fn cached<T: Clone + 'static>(&self, id: u128) -> Result<Cached<T>, Never> {
    let Ok(available) = self.available();
    let locked = match available {
      Some(locked) => locked,
      None => return Ok(Cached::Absent),
    };

    Ok(match locked.get(&id) {
      Some(known) => Cached::Present(
        known
          .as_ref()
          .and_then(|value| value.downcast_ref::<T>().cloned()),
      ),
      None => Cached::Absent,
    })
  }

  pub(crate) fn get_or_insert_with<T: Clone + Send + Sync + 'static>(
    &self,
    id: u128,
    made: impl FnOnce() -> Option<T>,
  ) -> Result<Option<T>, Never> {
    let Ok(known) = self.cached(id);

    match known {
      Cached::Present(value) => return Ok(value),
      Cached::Absent => {}
    }

    let value = made();

    let Ok(available) = self.available();

    match available {
      Some(mut writable) => {
        let owned = value.clone().map(|kept| {
          let boxed: Box<dyn Any + Send + Sync> = Box::new(kept);
          boxed
        });
        writable.insert(id, owned);
      }
      None => {}
    }

    Ok(value)
  }
}

pub trait CacheKey {
  fn cache_key(&self) -> u128;
}

impl<T: CacheKey, U: CacheKey> CacheKey for (T, U) {
  fn cache_key(&self) -> u128 {
    let Ok(key) = hash128(&(self.0.cache_key(), self.1.cache_key()));

    key
  }
}

impl CacheKey for Dict<'_> {
  fn cache_key(&self) -> u128 {
    let Ok(data) = self.data();
    let Ok(key) = hash128(data);

    key
  }
}

impl CacheKey for Stream<'_> {
  fn cache_key(&self) -> u128 {
    let Ok(dict) = self.dict();

    dict.cache_key()
  }
}

impl CacheKey for Null {
  fn cache_key(&self) -> u128 {
    let Ok(key) = hash128(self);

    key
  }
}

impl CacheKey for bool {
  fn cache_key(&self) -> u128 {
    let Ok(key) = hash128(self);

    key
  }
}

impl CacheKey for console_core_portable_document_syntax::object::Number {
  fn cache_key(&self) -> u128 {
    let Ok(value) = self.as_f64();
    let Ok(key) = hash128(&value.to_bits());

    key
  }
}

impl CacheKey for console_core_portable_document_syntax::object::String<'_> {
  fn cache_key(&self) -> u128 {
    let Ok(key) = hash128(self.as_ref());

    key
  }
}

impl CacheKey for Name<'_> {
  fn cache_key(&self) -> u128 {
    let Ok(key) = hash128(self);

    key
  }
}

impl CacheKey for Array<'_> {
  fn cache_key(&self) -> u128 {
    let Ok(data) = self.data();
    let Ok(key) = hash128(data);

    key
  }
}

impl CacheKey for Object<'_> {
  fn cache_key(&self) -> u128 {
    match self {
      Object::Null(null) => null.cache_key(),
      Object::Boolean(boolean) => bool::from(*boolean).cache_key(),
      Object::Number(number) => number.cache_key(),
      Object::String(string) => string.cache_key(),
      Object::Name(name) => name.cache_key(),
      Object::Dict(dict) => dict.cache_key(),
      Object::Array(array) => array.cache_key(),
      Object::Stream(stream) => stream.cache_key(),
    }
  }
}

impl CacheKey for ObjRef {
  fn cache_key(&self) -> u128 {
    let Ok(key) = hash128(self);

    key
  }
}

impl<T: CacheKey> CacheKey for MaybeRef<T> {
  fn cache_key(&self) -> u128 {
    match self {
      Self::Ref(reference) => reference.cache_key(),
      Self::NotRef(object) => object.cache_key(),
    }
  }
}

impl CacheKey for Affine {
  fn cache_key(&self) -> u128 {
    let [xx, yx, xy, yy, dx, dy] = self.as_coeffs();
    let Ok(key) = hash128(&[
      xx.to_bits(),
      yx.to_bits(),
      xy.to_bits(),
      yy.to_bits(),
      dx.to_bits(),
      dy.to_bits(),
    ]);

    key
  }
}

impl CacheKey for u128 {
  fn cache_key(&self) -> u128 {
    let Ok(key) = hash128(self);

    key
  }
}
