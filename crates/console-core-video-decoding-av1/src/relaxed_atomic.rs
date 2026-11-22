#![forbid(unsafe_code)]
use atomig::Atom;
use atomig::Atomic;
use std::sync::atomic::Ordering;

#[derive(Default)]
pub struct RelaxedAtomic<T: Atom> {
  inner: Atomic<T>,
}

impl<T: Atom> RelaxedAtomic<T> {
  pub fn new(value: T) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        inner: Atomic::new(value),
      }
    })
  }

  pub fn get(&self) -> core::result::Result<T, console_core_never::Never> {
    Ok(self.inner.load(Ordering::Relaxed))
  }

  pub fn set(&self, value: T) -> core::result::Result<(), console_core_never::Never> {
    {
      self.inner.store(value, Ordering::Relaxed);
    };

    Ok(())
  }

  pub fn update(
    &self,
    f: impl Fn(T) -> T,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.set(f({
        let Ok(explicit_value) = self.get();
        explicit_value
      }));
    };

    Ok(())
  }

  pub fn inner(&self) -> core::result::Result<&Atomic<T>, console_core_never::Never> {
    Ok(&self.inner)
  }
}

impl<T: Atom + Copy> RelaxedAtomic<T> {
  pub fn get_update(
    &self,
    f: impl Fn(T) -> T,
  ) -> core::result::Result<T, console_core_never::Never> {
    Ok({
      let Ok(old) = self.get();
      let Ok(()) = self.set(f(old));
      old
    })
  }
}

impl<T: Atom> Clone for RelaxedAtomic<T> {
  fn clone(&self) -> Self {
    let Ok(relaxed_atomic) = Self::new({
      let Ok(explicit_value) = self.get();
      explicit_value
    });

    relaxed_atomic
  }
}

impl<T: Atom> From<T> for RelaxedAtomic<T> {
  fn from(value: T) -> Self {
    let Ok(relaxed_atomic) = Self::new(value);

    relaxed_atomic
  }
}
