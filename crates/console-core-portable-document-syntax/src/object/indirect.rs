use crate::ObjectReadMode;
use crate::object::{ObjectIdentifier, ObjectLike};
use crate::reader::Reader;
use crate::reader::{IndirectReferenceError, Readable, ReaderContext, ReaderExt, Skippable};

#[derive(Debug, Clone)]
pub(crate) struct IndirectObject<T> {
  id: ObjectIdentifier,
  inner: T,
}

impl<T> IndirectObject<T> {
  pub(crate) fn get(self) -> core::result::Result<T, console_core_never::Never> {
    Ok(self.inner)
  }

  pub(crate) fn id(&self) -> core::result::Result<&ObjectIdentifier, console_core_never::Never> {
    Ok(&self.id)
  }
}

impl<'a, T> Readable<'a> for IndirectObject<T>
where
  T: ObjectLike<'a>,
{
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let mut ctx_2 = ctx.clone();
    let id = r.read_without_context::<ObjectIdentifier>()?;

    match ctx_2.push_parent(id) {
      Ok(()) => {}
      Err(IndirectReferenceError::Cycle) => {
        warn!("cycle detected in indirect object: {id:?}");
        return None;
      }
      Err(IndirectReferenceError::DepthLimit) => {
        warn!("indirect object depth limit reached for {id:?}");
        return None;
      }
    }

    let Ok(()) = ctx_2.set_obj_number(id);
    r.skip_white_spaces_and_comments();
    let inner = r.read_with_context::<T>(&ctx_2)?;
    r.skip_white_spaces_and_comments();

    let Ok(_) = r.forward_tag(b"endobj");

    Some(Self { id, inner })
  }
}

impl<T> Skippable for IndirectObject<T>
where
  T: Skippable,
{
  fn skip(r: &mut Reader<'_>, _: ObjectReadMode) -> Option<()> {
    r.skip::<ObjectIdentifier>(ObjectReadMode::Document)?;
    r.skip_white_spaces_and_comments();
    r.skip::<T>(ObjectReadMode::Document)?;
    r.skip_white_spaces_and_comments();

    let Ok(_) = r.forward_tag(b"endobj");

    Some(())
  }
}
