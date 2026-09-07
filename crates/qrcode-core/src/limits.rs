//! Resource budgets applied at QR-code construction boundaries.

use crate::types::{QrError, QrResult, Version};

/// The largest input accepted by the default resource budget.
///
/// This is deliberately a conservative upper bound for a QR byte payload.
/// Individual versions and error-correction levels usually have a smaller
/// capacity and remain the final authority during encoding.
pub const DEFAULT_MAX_DATA_LENGTH: usize = 7_089;

/// The largest rendered dimension allowed by the default resource budget.
pub const DEFAULT_MAX_RENDER_SIZE: (u32, u32) = (4_096, 4_096);

/// Explicit resource budgets for bounded QR-code construction.
///
/// [`QrCode::with_limits`](https://docs.rs/qrcode-rs/latest/qrcode_rs/struct.QrCode.html#method.with_limits)
/// applies these limits before allocating encoder state and after selecting
/// the resulting symbol dimensions. With the facade crate's `std` feature,
/// `encoding_timeout` is checked at synchronous construction boundaries. It is
/// not a preemptive interrupt for an in-flight CPU step. The dimensions are the
/// maximum width and height of the symbol passed to a renderer; a renderer may
/// impose a stricter pixel budget of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResourceLimits {
    /// Maximum number of input bytes accepted before parsing or allocation.
    pub max_data_length: usize,
    /// Maximum normal QR version considered by automatic version selection.
    pub max_version: Version,
    /// Maximum `(width, height)` accepted for the generated module symbol.
    pub max_render_size: (u32, u32),
    /// Optional synchronous encoding timeout budget in milliseconds.
    ///
    /// `None` disables timeout checks. `Some(0)` is rejected as a malformed
    /// budget because it cannot describe useful work.
    pub encoding_timeout: Option<u64>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_data_length: DEFAULT_MAX_DATA_LENGTH,
            max_version: Version::Normal(40),
            max_render_size: DEFAULT_MAX_RENDER_SIZE,
            encoding_timeout: None,
        }
    }
}

impl ResourceLimits {
    /// Creates an explicit resource budget.
    #[must_use]
    pub const fn new(max_data_length: usize, max_version: Version, max_render_size: (u32, u32)) -> Self {
        Self { max_data_length, max_version, max_render_size, encoding_timeout: None }
    }

    /// Returns this budget with an encoding timeout in milliseconds.
    ///
    /// The facade crate enforces this budget at synchronous construction
    /// boundaries when `std` is enabled.
    #[must_use]
    pub const fn with_encoding_timeout_millis(mut self, timeout_ms: u64) -> Self {
        self.encoding_timeout = Some(timeout_ms);
        self
    }

    /// Validates the shape of this budget without inspecting input data.
    ///
    /// Automatic construction currently targets normal QR versions, so Micro
    /// versions are rejected here rather than being silently interpreted as a
    /// normal-version cap. Zero render dimensions cannot describe a symbol.
    pub fn validate(self) -> QrResult<()> {
        let valid_version = matches!(self.max_version, Version::Normal(1..=40));
        let valid_render_size = self.max_render_size.0 != 0 && self.max_render_size.1 != 0;
        let valid_timeout = self.encoding_timeout != Some(0);
        if valid_version && valid_render_size && valid_timeout { Ok(()) } else { Err(QrError::InvalidResourceLimits) }
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_MAX_DATA_LENGTH, DEFAULT_MAX_RENDER_SIZE, ResourceLimits};
    use crate::types::Version;

    #[test]
    fn default_budget_is_bounded() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_data_length, DEFAULT_MAX_DATA_LENGTH);
        assert_eq!(limits.max_version, Version::Normal(40));
        assert_eq!(limits.max_render_size, DEFAULT_MAX_RENDER_SIZE);
        assert_eq!(limits.encoding_timeout, None);
        assert!(limits.validate().is_ok());
    }

    #[test]
    fn invalid_budget_is_rejected() {
        assert!(ResourceLimits::new(1, Version::Micro(4), (1, 1)).validate().is_err());
        assert!(ResourceLimits::new(1, Version::Normal(1), (0, 1)).validate().is_err());
        assert!(ResourceLimits::new(1, Version::Normal(1), (1, 1)).with_encoding_timeout_millis(0).validate().is_err());
    }

    #[test]
    fn timeout_budget_is_optional() {
        let limits = ResourceLimits::new(1, Version::Normal(1), (1, 1)).with_encoding_timeout_millis(10);

        assert_eq!(limits.encoding_timeout, Some(10));
        assert!(limits.validate().is_ok());
    }
}
