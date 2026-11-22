#![forbid(unsafe_code)]
use crate::include::dav1d::picture::Rav1dPictureDataComponent;
use crate::src::disjoint_mut::AsMutPtr;
use crate::src::disjoint_mut::DisjointMut;
use crate::src::strided::Strided;
use crate::src::strided::WithStride;
use crate::src::with_offset::WithOffset;

pub enum PicOrBuf<'a, T: AsMutPtr<Target = u8>> {
  Pic(&'a Rav1dPictureDataComponent),
  Buf(WithStride<&'a DisjointMut<T>>),
}

impl<'a, T: AsMutPtr<Target = u8>> Clone for PicOrBuf<'a, T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<'a, T: AsMutPtr<Target = u8>> Copy for PicOrBuf<'a, T> {}

impl<'a, T: AsMutPtr<Target = u8>> Strided for PicOrBuf<'a, T> {
  fn fixed_stride(&self) -> Result<i64, console_core_never::Never> {
    match self {
      Self::Pic(pic) => pic.fixed_stride(),
      Self::Buf(buf) => buf.fixed_stride(),
    }
  }
}

impl<'a, T: AsMutPtr<Target = u8>> WithOffset<PicOrBuf<'a, T>> {
  pub fn pic(
    pic: WithOffset<&'a Rav1dPictureDataComponent>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        data: PicOrBuf::Pic(pic.data),
        offset: pic.offset,
      }
    })
  }

  pub fn buf(
    buf: WithOffset<WithStride<&'a DisjointMut<T>>>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        data: PicOrBuf::Buf(buf.data),
        offset: buf.offset,
      }
    })
  }
}
