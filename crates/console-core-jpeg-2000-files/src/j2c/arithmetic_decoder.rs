//! Compatibility with the codec's bit reader; MQ state lives in the workspace.

pub(crate) use console_core_mq_decoding::Context as ArithmeticDecoderContext;

pub(crate) struct ArithmeticDecoder<'a>(console_core_mq_decoding::Decoder<'a>);

impl<'a> ArithmeticDecoder<'a> {
  pub(crate) fn new(data: &'a [u8]) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(decoder) = console_core_mq_decoding::Decoder::new(data);

    Ok(Self(decoder))
  }

  #[inline(always)]
  pub(crate) fn read_bit(
    &mut self,
    context: &mut ArithmeticDecoderContext,
  ) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(bit) = self.0.read_bit(context);

    Ok(bit)
  }
}
