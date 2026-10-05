// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
/*
 * Copyright 2007 ZXing authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

// package com.google.zxing.common;

// import com.google.zxing.NotFoundException;

use crate::engine::common::Result;
use crate::engine::{Exceptions, Point, point};

use super::{BitMatrix, GridSampler, SamplerControl};

/**
 * @author Sean Owen
 */
#[derive(Default)]
pub struct DefaultGridSampler;

impl GridSampler for DefaultGridSampler {
    fn sample_grid(
        &self,
        image: &BitMatrix,
        dimensionX: u32,
        dimensionY: u32,
        controls: &[SamplerControl],
    ) -> Result<(BitMatrix, [Point; 4])> {
        if dimensionX == 0 || dimensionY == 0 {
            return Err(Exceptions::NOT_FOUND);
        }

        for SamplerControl { p0, p1, transform } in controls {
            if ![p0.x, p0.y, p1.x, p1.y].iter().all(|v| v.is_finite() && v.fract() == 0.0)
                || p0.x < 0.0
                || p0.y < 0.0
                || p0.x >= p1.x
                || p0.y >= p1.y
                || p1.x > dimensionX as f32
                || p1.y > dimensionY as f32
            {
                return Err(Exceptions::NOT_FOUND);
            }
            // Precheck the corners of every roi to bail out early if the grid is "obviously" not completely inside the image
            let isInside = |x: f32, y: f32| image.is_in(transform.transform_point(Point::centered(point(x, y))));
            if !transform.isValid()
                || !isInside(p0.x, p0.y)
                || !isInside(p1.x - 1.0, p0.y)
                || !isInside(p1.x - 1.0, p1.y - 1.0)
                || !isInside(p0.x, p1.y - 1.0)
            {
                return Err(Exceptions::NOT_FOUND);
            }
        }

        let mut bits = BitMatrix::new(dimensionX, dimensionY)?;
        for SamplerControl { p0, p1, transform } in controls {
            // for (auto&& [x0, x1, y0, y1, mod2Pix] : rois) {
            for y in (p0.y as i32)..(p1.y as i32) {
                // for (int y = y0; y < y1; ++y)
                for x in (p0.x as i32)..(p1.x as i32) {
                    // for (int x = x0; x < x1; ++x) {
                    let p = transform.transform_point(Point::from((x, y)).centered()); //mod2Pix(centered(PointI{x, y}));

                    if !image.is_in(p) {
                        return Err(Exceptions::NOT_FOUND);
                    }
                    if image.get_point(p) {
                        bits.set(x as u32, y as u32);
                    }
                }
            }
        }

        // dbg!(image.to_string());
        // dbg!(bits.to_string());

        let projectCorner = |p: Point| -> Point {
            for SamplerControl { p0, p1, transform } in controls {
                if p0.x <= p.x && p.x <= p1.x && p0.y <= p.y && p.y <= p1.y {
                    return transform.transform_point(p) + point(0.5, 0.5);
                }
            }
            Point::default()
        };

        let tl = projectCorner(Point::default());
        let tr = projectCorner(Point::from((dimensionX, 0)));
        let br = projectCorner(Point::from((dimensionX, dimensionY)));
        let bl = projectCorner(Point::from((0, dimensionY)));

        Ok((bits, [tl, tr, bl, br]))
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    use crate::engine::common::{PerspectiveTransform, Quadrilateral};
    fn identity(size: i32) -> PerspectiveTransform {
        let quad = Quadrilateral::rectangle(size, size, None);
        PerspectiveTransform::quadrilateralToQuadrilateral(quad, quad).unwrap()
    }
    #[test]
    fn identity_sampling_preserves_the_grid() {
        let mut image = BitMatrix::new(3, 3).unwrap();
        image.set(1, 2);
        let controls = [SamplerControl { p0: point(0.0, 0.0), p1: point(3.0, 3.0), transform: identity(3) }];
        let (sample, _) = DefaultGridSampler.sample_grid(&image, 3, 3, &controls).unwrap();
        assert_eq!(sample, image);
    }
    #[test]
    fn invalid_regions_and_transforms_return_errors() {
        let image = BitMatrix::new(3, 3).unwrap();
        for (p0, p1) in [
            (point(-1.0, 0.0), point(3.0, 3.0)),
            (point(0.0, 0.0), point(4.0, 3.0)),
            (point(0.5, 0.0), point(3.0, 3.0)),
            (point(0.0, 0.0), point(f32::INFINITY, 3.0)),
            (point(3.0, 0.0), point(3.0, 3.0)),
        ] {
            let controls = [SamplerControl { p0, p1, transform: identity(3) }];
            assert!(DefaultGridSampler.sample_grid(&image, 3, 3, &controls).is_err());
        }
        let controls =
            [SamplerControl { p0: point(0.0, 0.0), p1: point(3.0, 3.0), transform: PerspectiveTransform::default() }];
        assert!(DefaultGridSampler.sample_grid(&image, 3, 3, &controls).is_err());
        assert!(DefaultGridSampler.sample_grid(&image, 0, 3, &[]).is_err());
    }
}
