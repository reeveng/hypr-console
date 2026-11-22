//! The distinct quantities the codec's arithmetic must not interchange.

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the inner value is a ratio for scaling a normalized CELT band"
  )
)]
pub(crate) struct BandGain(pub(crate) f32);

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the inner value is the amplitude ratio used during packet-loss concealment"
  )
)]
pub(crate) struct LossFade(pub(crate) f32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Accumulator(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct AllocationFloor(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct AllocationStep(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct BandIndex(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct BandWidth(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct BitBalance(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct BitBudget(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockCount(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockExponent(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockOrder(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ByteLength(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct CacheOffset(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct CacheRow(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ChannelCount(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ChannelIndex(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ChannelLogDimensions(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct CodebookIndex(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct CoefficientIndex(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct CollapseMask(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct CurrentValue(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct DegreesOfFreedom(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Denominator(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Extent(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct FractionalBits(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameCount(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameSize(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct HalfOrder(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct InputRate(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct InverseGain(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct LeftShift(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct LogDimensions(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct LogValue(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct LowWord(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct MaxPulseCount(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the inner value is the mid-band amplitude ratio"
  )
)]
pub(crate) struct MidGain(pub(crate) f32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Numerator(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct OtherCoefficient(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct OutputRate(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct OwnCoefficient(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Padding(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Parity(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct PitchPeriod(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct PreviousValue(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct PulseCount(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct PulseIndex(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Q31Coefficient(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the inner value is the upper angular-frequency bound in radians per sample"
  )
)]
pub(crate) struct RangeEnd(pub(crate) f64);

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the inner value is the lower angular-frequency bound in radians per sample"
  )
)]
pub(crate) struct RangeStart(pub(crate) f64);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReflectionCoefficient(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct RemainingBits(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the inner value is an offset from the resampler output phase in samples"
  )
)]
pub(crate) struct SamplePosition(pub(crate) f64);

#[derive(Clone, Copy, Debug)]
pub(crate) struct SampleWord(pub(crate) i32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShellWidth(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the inner value is the side-band amplitude ratio"
  )
)]
pub(crate) struct SideGain(pub(crate) f32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct SignalType(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct StartOffset(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Stride(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct TimeFrequencyColumn(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Total(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct VectorSize(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct WideWord(pub(crate) i32);
