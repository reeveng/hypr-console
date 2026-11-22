#[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))]
use bytemuck::cast;
#[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))]
use core::arch::aarch64::uint16x8_t;

#[allow(
    non_camel_case_types,
    reason = "the name is the lane type and count, as core::simd and every other lane in this module spell them"
)]
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct u16x16(pub [u16; 16]);

#[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))]
#[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
        explicit080_macro_budget,
        reason = "the eight NEON operations share two register splits and a packed result while preserving direct calls to the selected intrinsic"
    )
)]
macro_rules! impl_aarch64_call {
    ($f:ident, $a:expr, $b:expr) => {
        let a = $a.split();
        let b = $b.split();
        // SAFETY: this branch is compiled only for AArch64 with NEON enabled.
        Self(bytemuck::cast(unsafe {
            [
                core::arch::aarch64::$f(a.0, b.0),
                core::arch::aarch64::$f(a.1, b.1),
            ]
        }))
    };
}

impl u16x16 {
    #[inline]
    fn combined(
        &self,
        rhs: &Self,
        operation: impl Fn(u16, u16) -> u16,
    ) -> Result<Self, console_core_never::Never> {
        let mut lanes = self.0;

        for (left, right) in lanes.iter_mut().zip(rhs.0) {
            *left = operation(*left, right);
        }

        Ok(Self(lanes))
    }

    #[inline]
    pub fn splat(n: u16) -> core::result::Result<Self, console_core_never::Never> {
        Ok(Self([n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n]))
    }

    #[inline]
    pub fn as_slice(&self) -> core::result::Result<&[u16; 16], console_core_never::Never> {
        Ok(&self.0)
    }

    #[inline]
    pub fn min(&self, rhs: &Self) -> core::result::Result<Self, console_core_never::Never> {
        Ok({
            cfg_if::cfg_if! {
                if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
                    impl_aarch64_call!(vminq_u16, self, rhs)
                } else {
                    let Ok(lanes) = self.combined(rhs, u16::min);

                  lanes
                }
            }
        })
    }

    #[inline]
    pub fn max(&self, rhs: &Self) -> core::result::Result<Self, console_core_never::Never> {
        Ok({
            cfg_if::cfg_if! {
                if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
                    impl_aarch64_call!(vmaxq_u16, self, rhs)
                } else {
                    let Ok(lanes) = self.combined(rhs, u16::max);

                  lanes
                }
            }
        })
    }

    #[inline]
    pub fn cmp_le(&self, rhs: &Self) -> core::result::Result<Self, console_core_never::Never> {
        Ok({
            cfg_if::cfg_if! {
                if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
                    impl_aarch64_call!(vcleq_u16, self, rhs)
                } else {
                    let Ok(lanes) = self.combined(rhs, |left, right| match left.cmp(&right) {
                      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => u16::MAX,
                      core::cmp::Ordering::Greater => 0,
                    });

                    lanes
                }
            }
        })
    }

    #[inline]
    pub fn blend(self, t: Self, e: Self) -> core::result::Result<Self, console_core_never::Never> {
        Ok((t & self) | (e & !self))
    }

    #[inline]
    #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))]
    pub fn split(self) -> (uint16x8_t, uint16x8_t) {
        let [first, second]: [uint16x8_t; 2] = cast(self.0);

        (first, second)
    }
}

impl core::ops::Add<u16x16> for u16x16 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        cfg_if::cfg_if! {
            if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
                impl_aarch64_call!(vaddq_u16, self, rhs)
            } else {
                let Ok(lanes) = self.combined(&rhs, u16::wrapping_add);

                lanes
            }
        }
    }
}

impl core::ops::Sub<u16x16> for u16x16 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        cfg_if::cfg_if! {
            if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
                impl_aarch64_call!(vsubq_u16, self, rhs)
            } else {
                let Ok(lanes) = self.combined(&rhs, u16::wrapping_sub);

                lanes
            }
        }
    }
}

impl core::ops::Mul<u16x16> for u16x16 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        cfg_if::cfg_if! {
            if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
                impl_aarch64_call!(vmulq_u16, self, rhs)
            } else {
                let Ok(lanes) = self.combined(&rhs, u16::wrapping_mul);

                lanes
            }
        }
    }
}

impl core::ops::BitAnd<u16x16> for u16x16 {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        cfg_if::cfg_if! {
            if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
                impl_aarch64_call!(vandq_u16, self, rhs)
            } else {
                let Ok(lanes) = self.combined(&rhs, |left, right| left & right);

                lanes
            }
        }
    }
}

impl core::ops::BitOr<u16x16> for u16x16 {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        cfg_if::cfg_if! {
            if #[cfg(all(feature = "simd", target_arch = "aarch64", target_feature = "neon"))] {
                impl_aarch64_call!(vorrq_u16, self, rhs)
            } else {
                let Ok(lanes) = self.combined(&rhs, |left, right| left | right);

                lanes
            }
        }
    }
}

impl core::ops::Not for u16x16 {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        Self(self.0.map(|lane| !lane))
    }
}

impl core::ops::Shr for u16x16 {
    type Output = Self;

    #[inline]
    fn shr(self, rhs: Self) -> Self::Output {
        let Ok(lanes) = self.combined(&rhs, |left, right| left.wrapping_shr(u32::from(right)));

        lanes
    }
}
