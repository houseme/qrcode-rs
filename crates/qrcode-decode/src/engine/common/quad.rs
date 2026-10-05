// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
use crate::engine::{Exceptions, Point, point};

#[derive(Clone, Copy, Debug)]
pub struct Quadrilateral(pub [Point; 4]);

impl Quadrilateral {
    pub fn rectangle(width: i32, height: i32, margin: Option<f32>) -> Quadrilateral {
        let margin = margin.unwrap_or(0.0);

        Quadrilateral([
            Point { x: margin, y: margin },
            Point { x: width as f32 - margin, y: margin },
            Point { x: width as f32 - margin, y: height as f32 - margin },
            Point { x: margin, y: height as f32 - margin },
        ])
    }

    pub fn rectangle_from_xy(x0: f32, x1: f32, y0: f32, y1: f32, o: Option<f32>) -> Self {
        let o = o.unwrap_or(0.5);
        Quadrilateral::from([
            point(x0 + o, y0 + o),
            point(x1 + o, y0 + o),
            point(x1 + o, y1 + o),
            point(x0 + o, y1 + o),
        ])
    }

    pub fn is_convex(&self) -> bool {
        if self.0.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return false;
        }
        let N = self.0.len();
        let mut sign = false;

        let mut m = f32::INFINITY;
        let mut M = 0.0_f32;

        for i in 0..N
        // for(int i = 0; i < N; i++)
        {
            let d1 = self.0[(i + 2) % N] - self.0[(i + 1) % N];
            let d2 = self.0[i] - self.0[(i + 1) % N];
            let cp = d1.cross(d2);

            // m = if m.abs() > cp { cp } else { m.abs() };

            // M = if M.abs() > cp { M.abs() } else { cp };
            if !cp.is_finite() || cp == 0.0 {
                return false;
            }
            m = m.min(cp.abs());
            M = M.max(cp.abs());

            if i == 0 {
                sign = cp > 0.0;
            } else if sign != (cp > 0.0) {
                return false;
            }
        }

        // It turns out being convex is not enough to prevent a "numerical instability"
        // that can cause the corners being projected inside the image boundaries but
        // some points near the corners being projected outside. This has been observed
        // where one corner is almost in line with two others. The M/m ratio is below 2
        // for the complete existing sample set. For very "skewed" QRCodes a value of
        // around 3 is realistic. A value of 14 has been observed to trigger the
        // instability.
        M / m < 4.0
    }

    pub fn rotated_corners(&self, n: Option<i32>, mirror: Option<bool>) -> Quadrilateral {
        let n = n.unwrap_or(1);

        let mirror = mirror.unwrap_or_default();

        let mut res = *self;
        res.0.rotate_left(n.rem_euclid(4) as usize);
        // std::rotate_copy(q.begin(), q.begin() + ((n + 4) % 4), q.end(), res.begin());
        if mirror {
            res.0.swap(1, 3);
        }
        // {std::swap(res[1], res[3]);}
        res
    }

    pub fn blend(a: &Quadrilateral, b: &Quadrilateral) -> Self {
        let c = a[0];
        let dist2First = |a, b| Point::distance(a, c) < Point::distance(b, c);
        // rotate points such that the the two topLeft points are closest to each other
        let min_element =
            b.0.iter()
                .copied()
                .min_by(|a, b| match dist2First(*a, *b) {
                    true => std::cmp::Ordering::Less,
                    false => std::cmp::Ordering::Greater,
                })
                .unwrap_or_default();
        let offset = b.0.iter().position(|v| *v == min_element).unwrap_or_default();
        // let offset = std::min_element(b.begin(), b.end(), dist2First) - b.begin();

        let mut res = Quadrilateral::default();
        for i in 0..4 {
            // for (int i = 0; i < 4; ++i){
            res[i] = (a[i] + b[(i + offset) % 4]) / 2.0;
        }

        res
    }
}

impl Default for Quadrilateral {
    fn default() -> Self {
        Self([Point { x: 0.0, y: 0.0 }; 4])
    }
}

impl std::ops::Index<usize> for Quadrilateral {
    type Output = Point;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl std::ops::IndexMut<usize> for Quadrilateral {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl From<[Point; 4]> for Quadrilateral {
    fn from(value: [Point; 4]) -> Self {
        Self(value)
    }
}

impl TryFrom<&Vec<Point>> for Quadrilateral {
    type Error = Exceptions;

    fn try_from(value: &Vec<Point>) -> Result<Self, Self::Error> {
        if value.len() == 4 {
            Ok(Self([value[0], value[1], value[2], value[3]]))
        } else {
            Err(Exceptions::INDEX_OUT_OF_BOUNDS)
        }
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    #[test]
    fn convexity_is_winding_independent_and_rejects_invalid_geometry() {
        let rectangle = Quadrilateral::rectangle(4, 4, None);
        assert!(rectangle.is_convex());
        assert!(Quadrilateral::from([rectangle[0], rectangle[3], rectangle[2], rectangle[1]]).is_convex());
        assert!(!Quadrilateral::default().is_convex());
        let skewed = Quadrilateral::from([point(0.0, 0.0), point(4.0, 0.0), point(4.0, 0.1), point(0.0, 4.0)]);
        assert!(!skewed.is_convex());
        assert!(!Quadrilateral::from([skewed[0], skewed[3], skewed[2], skewed[1]]).is_convex());
        for bad in [f32::NAN, f32::INFINITY] {
            let mut invalid = rectangle;
            invalid[0].x = bad;
            assert!(!invalid.is_convex());
        }
    }
    #[test]
    fn rotations_normalize_all_signed_counts() {
        let quad = Quadrilateral::rectangle(4, 4, None);
        for n in [0, 1, 2, 3, -1, -5, i32::MAX, i32::MIN] {
            let rotated = quad.rotated_corners(Some(n), None);
            for i in 0..4 {
                assert_eq!(rotated[i], quad[(i + n.rem_euclid(4) as usize) % 4]);
            }
        }
    }
}
