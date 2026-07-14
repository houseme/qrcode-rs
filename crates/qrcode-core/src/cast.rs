//! Checked narrowing conversions and truncation helpers used across the encoder.

/// Truncate a value to u8 by masking the lower 8 bits.
pub trait Truncate {
    /// Truncates this value to its low 8 bits as a `u8`.
    fn truncate_as_u8(self) -> u8;
}

impl Truncate for u16 {
    #[allow(clippy::cast_possible_truncation)]
    fn truncate_as_u8(self) -> u8 {
        (self & 0xff) as u8
    }
}

/// Checked narrowing conversions with debug-mode overflow assertions.
///
/// In debug builds, these use `TryFrom` + `unwrap()` to catch overflow bugs.
/// In release builds, they compile to plain `as` casts (zero cost).
///
/// Standard traits can't replace this: `From` only supports lossless widening,
/// `TryFrom` returns `Result` which would require changing all call sites.
#[allow(clippy::wrong_self_convention)]
pub trait As {
    /// Narrows to `u16` (panics on overflow in debug, `as` in release).
    fn as_u16(self) -> u16;
    /// Narrows to `i16` (panics on overflow in debug, `as` in release).
    fn as_i16(self) -> i16;
    /// Widens/narrows to `u32` (panics on overflow in debug, `as` in release).
    fn as_u32(self) -> u32;
    /// Widens/narrows to `usize` (panics on overflow in debug, `as` in release).
    fn as_usize(self) -> usize;
    /// Widens/narrows to `isize` (panics on overflow in debug, `as` in release).
    fn as_isize(self) -> isize;
}

macro_rules! impl_as {
    ($ty:ty) => {
        #[cfg(debug_assertions)]
        impl As for $ty {
            fn as_u16(self) -> u16 {
                u16::try_from(self).unwrap()
            }

            fn as_i16(self) -> i16 {
                i16::try_from(self).unwrap()
            }

            fn as_u32(self) -> u32 {
                u32::try_from(self).unwrap()
            }

            fn as_usize(self) -> usize {
                usize::try_from(self).unwrap()
            }

            fn as_isize(self) -> isize {
                isize::try_from(self).unwrap()
            }
        }

        #[cfg(not(debug_assertions))]
        impl As for $ty {
            fn as_u16(self) -> u16 {
                self as u16
            }
            fn as_i16(self) -> i16 {
                self as i16
            }
            fn as_u32(self) -> u32 {
                self as u32
            }
            fn as_usize(self) -> usize {
                self as usize
            }
            fn as_isize(self) -> isize {
                self as isize
            }
        }
    };
}

impl_as!(i16);
impl_as!(u32);
impl_as!(usize);
impl_as!(isize);
