// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
use std::borrow::Cow;

use crate::engine::{Exceptions, LuminanceSource, common::Result};

/// Checked, immutable, row-contiguous view of the caller's luma8 frame.
#[derive(Debug)]
pub struct Luma8Source<'a> {
    data: &'a [u8],
    width: usize,
    height: usize,
}

impl<'a> Luma8Source<'a> {
    pub fn new_with_slice(data: &'a [u8], width: u32, height: u32) -> Result<Self> {
        let width = width as usize;
        let height = height as usize;
        if width.checked_mul(height) != Some(data.len()) {
            return Err(Exceptions::illegal_argument_with("Dimensions do not match the data length."));
        }
        Ok(Self { data, width, height })
    }
}

impl LuminanceSource for Luma8Source<'_> {
    fn get_row(&self, y: usize) -> Option<Cow<'_, [u8]>> {
        if y >= self.height {
            return None;
        }
        // Construction checked the full product. This bounded row index and
        // its end therefore fit in both usize and the borrowed data slice.
        let start = y * self.width;
        Some(Cow::Borrowed(&self.data[start..start + self.width]))
    }

    fn get_matrix(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(self.data)
    }

    fn get_width(&self) -> usize {
        self.width
    }

    fn get_height(&self) -> usize {
        self.height
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::engine::{Luma8Source, LuminanceSource};

    #[test]
    fn rows_and_matrix_borrow_exact_input_without_modifying_it() {
        let bytes = [0, 10, 100, 255, 11, 22];
        let source = Luma8Source::new_with_slice(&bytes, 3, 2).unwrap();
        assert_eq!((source.get_width(), source.get_height()), (3, 2));
        let Cow::Borrowed(matrix) = source.get_matrix() else { panic!("matrix must borrow input") };
        assert_eq!(matrix.as_ptr(), bytes.as_ptr());
        assert_eq!(matrix, bytes);
        for (y, expected) in [[0, 10, 100], [255, 11, 22]].iter().enumerate() {
            let Some(Cow::Borrowed(row)) = source.get_row(y) else { panic!("row must borrow input") };
            assert_eq!(row, expected);
            assert_eq!(row.as_ptr(), bytes[y * 3..].as_ptr());
        }
        assert_eq!(bytes, [0, 10, 100, 255, 11, 22]);
    }

    #[test]
    fn short_and_long_buffers_return_dimension_errors() {
        for bytes in [&[0; 3][..], &[0; 5][..]] {
            assert!(Luma8Source::new_with_slice(bytes, 2, 2).is_err());
        }
        // This product wraps to zero on a 32-bit target if not checked.
        assert!(Luma8Source::new_with_slice(&[], 65_536, 65_536).is_err());
    }

    #[test]
    fn zero_area_views_are_empty_and_rows_keep_declared_bounds() {
        for (width, height) in [(0, 0), (0, 2), (2, 0), (0, u32::MAX), (u32::MAX, 0)] {
            let source = Luma8Source::new_with_slice(&[], width, height).unwrap();
            assert!(matches!(source.get_matrix(), Cow::Borrowed([])));
            if height > 0 {
                assert!(matches!(source.get_row(0), Some(Cow::Borrowed([]))));
            } else {
                assert!(source.get_row(0).is_none());
            }
            assert!(source.get_row(height as usize).is_none());
        }
        assert!(Luma8Source::new_with_slice(&[0], 0, 1).is_err());
    }

    #[test]
    fn out_of_bounds_rows_return_none_before_index_arithmetic() {
        let source = Luma8Source::new_with_slice(&[1, 2, 3, 4], 2, 2).unwrap();
        assert_eq!(source.get_row(1).unwrap().as_ref(), &[3, 4]);
        assert!(source.get_row(2).is_none());
        assert!(source.get_row(usize::MAX).is_none());
    }
}
