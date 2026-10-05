// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
use thiserror::Error;

/// Error categories returned by the private QR decoding engine.
///
/// This type derives from rxing 0.9.3, but is owned by qrcode-decode and has a
/// distinct Rust type identity from `rxing::Exceptions`.
#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum Exceptions {
    /// The private engine reported this error category.
    #[error("IllegalArgumentException{}", if .0.is_empty() { String::new() } else { format!(" - {}", .0) })]
    IllegalArgumentException(String),
    /// The private engine reported this error category.
    #[error("UnsupportedOperationException{}", if .0.is_empty() { String::new()  } else { format!(" - {}", .0) })]
    UnsupportedOperationException(String),
    /// The private engine reported this error category.
    #[error("IllegalStateException{}", if .0.is_empty() { String::new()  } else { format!(" - {}", .0) })]
    IllegalStateException(String),
    /// The private engine reported this error category.
    #[error("ArithmeticException{}", if .0.is_empty() { String::new()  } else { format!(" - {}", .0) })]
    ArithmeticException(String),
    /// The private engine reported this error category.
    #[error("NotFoundException{}", if .0.is_empty() { String::new()  } else { format!(" - {}", .0) })]
    NotFoundException(String),
    /// The private engine reported this error category.
    #[error("FormatException{}", if .0.is_empty() { String::new()  } else { format!(" - {}", .0) })]
    FormatException(String),
    /// The private engine reported this error category.
    #[error("ChecksumException{}", if .0.is_empty() { String::new()  } else { format!(" - {}", .0) })]
    ChecksumException(String),
    /// The private engine reported this error category.
    #[error("ReaderException{}", if .0.is_empty() { String::new()  } else { format!(" - {}", .0) })]
    ReaderException(String),
    /// The private engine reported this error category.
    #[error("WriterException{}", if .0.is_empty() { String::new()  } else { format!(" - {}", .0) })]
    WriterException(String),
    /// The private engine reported this error category.
    #[error("ReedSolomonException{}", if .0.is_empty() { String::new()  } else { format!(" - {}", .0) })]
    ReedSolomonException(String),
    /// The private engine reported this error category.
    #[error("IndexOutOfBoundsException{}", if .0.is_empty() { String::new()  } else { format!(" - {}", .0) })]
    IndexOutOfBoundsException(String),
    /// The private engine reported this error category.
    #[error("RuntimeException{}", if .0.is_empty() { String::new()  } else { format!(" - {}", .0) })]
    RuntimeException(String),
    /// The private engine reported this error category.
    #[error("ParseException{}", if .0.is_empty() { String::new()  } else { format!(" - {}", .0) })]
    ParseException(String),
    /// The private engine reported this error category.
    #[error("ReaderDecodeException")]
    ReaderDecodeException(),
}

impl Exceptions {
    /// An empty diagnostic for this error category.
    pub const ILLEGAL_ARGUMENT: Self = Self::IllegalArgumentException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn illegal_argument_with<I: Into<String>>(x: I) -> Self {
        Self::IllegalArgumentException(x.into())
    }

    /// An empty diagnostic for this error category.
    pub const UNSUPPORTED_OPERATION: Self = Self::UnsupportedOperationException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn unsupported_operation_with<I: Into<String>>(x: I) -> Self {
        Self::UnsupportedOperationException(x.into())
    }

    /// An empty diagnostic for this error category.
    pub const ILLEGAL_STATE: Self = Self::IllegalStateException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn illegal_state_with<I: Into<String>>(x: I) -> Self {
        Self::IllegalStateException(x.into())
    }

    /// An empty diagnostic for this error category.
    pub const ARITHMETIC: Self = Self::ArithmeticException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn arithmetic_with<I: Into<String>>(x: I) -> Self {
        Self::ArithmeticException(x.into())
    }

    /// An empty diagnostic for this error category.
    pub const NOT_FOUND: Self = Self::NotFoundException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn not_found_with<I: Into<String>>(x: I) -> Self {
        Self::NotFoundException(x.into())
    }

    /// An empty diagnostic for this error category.
    pub const FORMAT: Self = Self::FormatException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn format_with<I: Into<String>>(x: I) -> Self {
        Self::FormatException(x.into())
    }

    /// An empty diagnostic for this error category.
    pub const CHECKSUM: Self = Self::ChecksumException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn checksum_with<I: Into<String>>(x: I) -> Self {
        Self::ChecksumException(x.into())
    }

    /// An empty diagnostic for this error category.
    pub const READER: Self = Self::ReaderException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn reader_with<I: Into<String>>(x: I) -> Self {
        Self::ReaderException(x.into())
    }

    /// An empty diagnostic for this error category.
    pub const WRITER: Self = Self::WriterException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn writer_with<I: Into<String>>(x: I) -> Self {
        Self::WriterException(x.into())
    }

    /// An empty diagnostic for this error category.
    pub const REED_SOLOMON: Self = Self::ReedSolomonException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn reed_solomon_with<I: Into<String>>(x: I) -> Self {
        Self::ReedSolomonException(x.into())
    }

    /// An empty diagnostic for this error category.
    pub const INDEX_OUT_OF_BOUNDS: Self = Self::IndexOutOfBoundsException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn index_out_of_bounds_with<I: Into<String>>(x: I) -> Self {
        Self::IndexOutOfBoundsException(x.into())
    }

    /// An empty diagnostic for this error category.
    pub const RUNTIME: Self = Self::RuntimeException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn runtime_with<I: Into<String>>(x: I) -> Self {
        Self::RuntimeException(x.into())
    }

    /// An empty diagnostic for this error category.
    pub const PARSE: Self = Self::ParseException(String::new());
    /// Builds an error with the supplied diagnostic.
    pub fn parse_with<I: Into<String>>(x: I) -> Self {
        Self::ParseException(x.into())
    }
}
