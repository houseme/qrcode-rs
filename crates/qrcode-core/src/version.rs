//! Compile-time QR version markers.

use crate::types::Version;

/// A compile-time checked normal QR version.
///
/// `N` must be in the normal QR version range `1..=40`. The range is checked
/// when [`ConstVersion::new`] or [`ConstVersion::VALUE`] is evaluated, so an
/// invalid fixed-version path fails at compile time instead of reaching the
/// encoder.
///
/// ```
/// use qrcode_core::{ConstVersion, Version};
///
/// const V5: Version = ConstVersion::<5>::VALUE;
/// assert_eq!(V5, Version::Normal(5));
/// ```
///
/// ```compile_fail
/// use qrcode_core::ConstVersion;
///
/// const INVALID: qrcode_core::Version = ConstVersion::<41>::VALUE;
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConstVersion<const N: i16>;

impl<const N: i16> ConstVersion<N> {
    /// The dynamic [`Version`] represented by this compile-time marker.
    pub const VALUE: Version = {
        assert!(N >= 1 && N <= 40, "normal QR version must be in 1..=40");
        Version::Normal(N)
    };

    /// Creates a compile-time checked fixed-version marker.
    pub const fn new() -> Self {
        let _ = Self::VALUE;
        Self
    }

    /// Returns the dynamic [`Version`] value for this marker.
    pub const fn version(self) -> Version {
        Self::VALUE
    }
}

/// A type-level fixed QR version.
pub trait StaticVersion: Copy {
    /// The dynamic [`Version`] represented by this type.
    const VERSION: Version;
}

impl<const N: i16> StaticVersion for ConstVersion<N> {
    const VERSION: Version = ConstVersion::<N>::VALUE;
}

#[cfg(test)]
mod tests {
    use super::{ConstVersion, StaticVersion};
    use crate::types::Version;

    #[test]
    fn const_version_exposes_dynamic_normal_version() {
        const V5: Version = ConstVersion::<5>::VALUE;

        assert_eq!(V5, Version::Normal(5));
        assert_eq!(ConstVersion::<5>::new().version(), Version::Normal(5));
        assert_eq!(<ConstVersion<5> as StaticVersion>::VERSION, Version::Normal(5));
    }
}
