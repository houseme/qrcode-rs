/*
 * Copyright 2016 Nu-book Inc.
 * Copyright 2016 ZXing authors
 * Copyright 2020 Axel Waggershauser
 * Copyright 2023 gitlost
 */
// SPDX-License-Identifier: Apache-2.0

//! Bounded finder discovery using rxing's ratio and concentric-pattern helpers.
//! The row traversal follows rxing 0.9.3's QR detector with minimum module size
//! fixed to zero, matching the adapter's detection settings.

use alloc::vec::Vec;

use ::rxing::common::BitMatrix;
use ::rxing::common::cpp_essentials::{
    ConcentricPattern, FindLeftGuardBy, FixedPattern, GetPatternRowTP, IsPattern, LocateConcentricPattern, PatternRow,
    PatternType, PatternView,
};
use ::rxing::{Point, point};

use super::DecodeError;

const PATTERN: FixedPattern<5, 7, false> = FixedPattern::new([1, 1, 3, 1, 1]);

fn find_pattern(view: PatternView<'_>) -> ::rxing::common::Result<PatternView<'_>> {
    FindLeftGuardBy::<5, _>(view, 5, |view: &PatternView<'_>, space_in_pixel: Option<f32>| {
        if i32::from(view[2]) < 3
            || view[2] < 2 as PatternType * core::cmp::max(view[0], view[4])
            || view[2] < core::cmp::max(view[1], view[3])
        {
            return false;
        }
        IsPattern::<true, 5, 7, false>(view, &PATTERN, space_in_pixel, 0.1, 0.0, 0.0) != 0.0
    })
}

pub(super) fn find_bounded(
    image: &BitMatrix,
    try_harder: bool,
    max_patterns: usize,
) -> Result<Vec<ConcentricPattern>, DecodeError> {
    const MAX_MODULES_FAST: u32 = 20 * 4 + 17;
    let height = image.height();
    let skip = if try_harder { 3 } else { core::cmp::max(3, 3 * height / (4 * MAX_MODULES_FAST)) };
    let mut patterns: Vec<ConcentricPattern> = Vec::new();
    let mut y = skip - 1;
    let mut row = PatternRow::default();
    while y < height {
        GetPatternRowTP(image, y, &mut row, false);
        let mut next = PatternView::new(&row);
        while let Ok(found) = find_pattern(next) {
            next = found;
            if !next.isValid() {
                break;
            }
            let center = point(
                next.pixelsInFront() as f32 + f32::from(next[0]) + f32::from(next[1]) + f32::from(next[2]) / 2.0,
                y as f32 + 0.5,
            );
            if !patterns.iter().any(|old| Point::distance(center, old.p) < old.size as f32 / 2.0)
                && let Some(pattern) = LocateConcentricPattern::<true, 5, 7>(
                    image,
                    &PATTERN.into(),
                    center,
                    i32::from(next.iter().sum::<u16>()) * 3,
                )
            {
                if patterns.len() == max_patterns {
                    return Err(DecodeError::FinderLimit { actual: patterns.len() + 1, max: max_patterns });
                }
                patterns.push(pattern);
            }
            next.skipPair();
            next.skipPair();
            next.extend();
        }
        y += skip;
    }
    Ok(patterns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::rxing::qrcode::cpp_port::detector::FindFinderPatterns;
    use qrcode_core::bits::Bits;
    use qrcode_core::canvas::Canvas;
    use qrcode_core::{Color, EcLevel, Version};

    fn finder_raster(columns: u32, rows: u32) -> BitMatrix {
        let module = 4;
        let margin = 4 * module;
        let stride = 11 * module;
        let mut image = BitMatrix::new(columns * stride + margin, rows * stride + margin).unwrap();
        for row in 0..rows {
            for column in 0..columns {
                let left = margin + column * stride;
                let top = margin + row * stride;
                for y in 0..7 {
                    for x in 0..7 {
                        let dark =
                            x == 0 || y == 0 || x == 6 || y == 6 || ((2..=4).contains(&x) && (2..=4).contains(&y));
                        if dark {
                            image.setRegion(left + x * module, top + y * module, module, module).unwrap();
                        }
                    }
                }
            }
        }
        image
    }

    #[test]
    fn discovery_accepts_max_minus_one_and_max_then_stops_on_max_plus_one() {
        let max = 10;
        for (columns, rows, expected) in [(3, 3, max - 1), (5, 2, max), (11, 1, max + 1)] {
            let image = finder_raster(columns, rows);
            let reference = FindFinderPatterns(&image, true, 0);
            assert_eq!(reference.len(), expected);
            if expected <= max {
                assert_eq!(find_bounded(&image, true, max).unwrap(), reference);
            } else {
                assert_eq!(find_bounded(&image, true, max), Err(DecodeError::FinderLimit { actual: max + 1, max }));
            }
        }
    }

    #[test]
    fn limit_error_reports_the_observed_prefix_not_the_full_raster_count() {
        let image = finder_raster(10, 4);
        assert_eq!(FindFinderPatterns(&image, true, 0).len(), 40);
        assert_eq!(find_bounded(&image, true, 2), Err(DecodeError::FinderLimit { actual: 3, max: 2 }));
    }

    fn symbol_raster(version: Version, ec_level: EcLevel, module: u32) -> BitMatrix {
        let mut bits = Bits::new(version);
        bits.push_numeric_data(b"123").unwrap();
        bits.push_terminator(ec_level).unwrap();
        let (data, correction) = qrcode_core::ec::construct_codewords(&bits.into_bytes(), version, ec_level).unwrap();
        let mut canvas = Canvas::new(version, ec_level);
        canvas.draw_all_functional_patterns();
        canvas.draw_data(&data, &correction);
        let colors = canvas.apply_best_mask().into_colors();
        let width = version.width() as u32;
        let quiet = if version.is_micro() { 2 } else { 4 };
        let side = (width + 2 * quiet) * module;
        let mut image = BitMatrix::new(side, side).unwrap();
        for y in 0..width {
            for x in 0..width {
                if colors[(y * width + x) as usize] == Color::Dark {
                    image.setRegion((x + quiet) * module, (y + quiet) * module, module, module).unwrap();
                }
            }
        }
        image
    }

    #[test]
    fn bounded_discovery_matches_upstream_for_normal_micro_and_blank_images() {
        let mut images = vec![BitMatrix::new(80, 80).unwrap()];
        for (version, ec_level, module) in [
            (Version::Normal(1), EcLevel::L, 4),
            (Version::Normal(7), EcLevel::H, 3),
            (Version::Normal(27), EcLevel::M, 4),
            (Version::Micro(1), EcLevel::L, 4),
            (Version::Micro(3), EcLevel::M, 4),
            (Version::Micro(4), EcLevel::Q, 3),
        ] {
            images.push(symbol_raster(version, ec_level, module));
        }
        // The tall raster also exercises the non-try-harder skip above three.
        images.push(finder_raster(2, 20));
        for image in images {
            for try_harder in [false, true] {
                let reference = FindFinderPatterns(&image, try_harder, 0);
                assert_eq!(find_bounded(&image, try_harder, 512).unwrap(), reference);
            }
        }
    }
}
