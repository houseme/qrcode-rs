// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
* Copyright 2017 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

use crate::engine::common::Result;
use crate::engine::{Exceptions, Point};

use super::RegressionLineTrait;

#[derive(Clone)]
pub struct DMRegressionLine {
    points: Vec<Point>,
    direction_inward: Point,
    pub(super) a: f32,
    pub(super) b: f32,
    pub(super) c: f32,
    // std::vector<PointF> _points;
    // PointF _directionInward;
    // PointF::value_t a = NAN, b = NAN, c = NAN;
}

impl Default for DMRegressionLine {
    fn default() -> Self {
        Self { points: Default::default(), direction_inward: Default::default(), a: f32::NAN, b: f32::NAN, c: f32::NAN }
    }
}

impl RegressionLineTrait for DMRegressionLine {
    fn isValid(&self) -> bool {
        !self.a.is_nan()
    }

    fn normal(&self) -> Point {
        if self.isValid() { Point { x: self.a, y: self.b } } else { self.direction_inward }
    }

    fn signedDistance(&self, p: Point) -> f32 {
        Point::dot(self.normal(), p) - self.c
    }

    fn distance_single(&self, p: Point) -> f32 {
        (self.signedDistance(p)).abs()
    }

    fn add(&mut self, p: Point) -> Result<()> {
        if self.direction_inward == Point::default() {
            return Err(Exceptions::ILLEGAL_STATE);
        }
        self.points.push(p);
        if self.points.len() == 1 {
            self.c = Point::dot(self.normal(), p);
        }
        Ok(())
    }

    fn setDirectionInward(&mut self, d: Point) {
        self.direction_inward = Point::normalized(d);
    }

    fn evaluate_max_distance(&mut self, maxSignedDist: Option<f64>, updatePoints: Option<bool>) -> bool {
        // let maxSignedDist = if let Some(m) = maxSignedDist { m } else { -1.0 };
        let maxSignedDist = maxSignedDist.unwrap_or(-1.0);
        let updatePoints = updatePoints.unwrap_or_default();

        let mut ret = self.evaluateSelf();
        if maxSignedDist > 0.0 {
            let mut points = self.points.clone();
            loop {
                let old_points_size = points.len();
                // remove points that are further 'inside' than maxSignedDist or further 'outside' than 2 x maxSignedDist
                // auto end = std::remove_if(points.begin(), points.end(), [this, maxSignedDist](auto p) {
                // 	auto sd = this->signedDistance(p);
                //     return sd > maxSignedDist || sd < -2 * maxSignedDist;
                // });
                // points.erase(end, points.end());
                points.retain(|&p| {
                    let sd = self.signedDistance(p) as f64;
                    !(sd > maxSignedDist || sd < -2.0 * maxSignedDist)
                });
                if old_points_size == points.len() {
                    break;
                }
                // #ifdef PRINT_DEBUG
                // 				printf("removed %zu points\n", old_points_size - points.size());
                // #endif
                ret = self.evaluate(&points);
            }

            if updatePoints {
                self.points = points;
            }
        }
        ret
    }

    fn isHighRes(&self) -> bool {
        let Some(mut min) = self.points.first().copied() else {
            return false;
        };
        let Some(mut max) = self.points.first().copied() else {
            return false;
        };
        for p in &self.points {
            min.x = f32::min(min.x, p.x);
            min.y = f32::min(min.y, p.y);
            max.x = f32::max(max.x, p.x);
            max.y = f32::max(max.y, p.y);
        }
        let diff = max - min;
        let len = diff.maxAbsComponent();
        let steps = f32::min(diff.x.abs(), diff.y.abs());
        // due to aliasing we get bad extrapolations if the line is short and too close to vertical/horizontal
        steps > 2.0 || len > 50.0
    }

    fn evaluate(&mut self, points: &[Point]) -> bool {
        let mean = points.iter().sum::<Point>() / points.len() as f32;

        let mut sumXX = 0.0;
        let mut sumYY = 0.0;
        let mut sumXY = 0.0;
        for p in points {
            // for (auto p = begin; p != end; ++p) {
            let d = *p - mean;
            sumXX += d.x * d.x;
            sumYY += d.y * d.y;
            sumXY += d.x * d.y;
        }
        if sumYY >= sumXX {
            let l = (sumYY * sumYY + sumXY * sumXY).sqrt();
            self.a = sumYY / l;
            self.b = -sumXY / l;
        } else {
            let l = (sumXX * sumXX + sumXY * sumXY).sqrt();
            self.a = sumXY / l;
            self.b = -sumXX / l;
        }
        if Point::dot(self.direction_inward, self.normal()) < 0.0 {
            // if (dot(_directionInward, normal()) < 0) {
            self.a = -self.a;
            self.b = -self.b;
        }
        self.c = Point::dot(self.normal(), mean); // (a*mean.x + b*mean.y);
        Point::dot(self.direction_inward, self.normal()) > 0.5
        // angle between original and new direction is at most 60 degree
    }

    fn evaluateSelf(&mut self) -> bool {
        let mean = self.points.iter().sum::<Point>() / self.points.len() as f32;

        let mut sumXX = 0.0;
        let mut sumYY = 0.0;
        let mut sumXY = 0.0;
        for p in &self.points {
            // for (auto p = begin; p != end; ++p) {
            let d = *p - mean;
            sumXX += d.x * d.x;
            sumYY += d.y * d.y;
            sumXY += d.x * d.y;
        }
        if sumYY >= sumXX {
            let l = (sumYY * sumYY + sumXY * sumXY).sqrt();
            self.a = sumYY / l;
            self.b = -sumXY / l;
        } else {
            let l = (sumXX * sumXX + sumXY * sumXY).sqrt();
            self.a = sumXY / l;
            self.b = -sumXX / l;
        }
        if Point::dot(self.direction_inward, self.normal()) < 0.0 {
            // if (dot(_directionInward, normal()) < 0) {
            self.a = -self.a;
            self.b = -self.b;
        }
        self.c = Point::dot(self.normal(), mean); // (a*mean.x + b*mean.y);
        Point::dot(self.direction_inward, self.normal()) > 0.5
        // angle between original and new direction is at most 60 degree
    }

    fn a(&self) -> f32 {
        self.a
    }

    fn b(&self) -> f32 {
        self.b
    }

    fn c(&self) -> f32 {
        self.c
    }
}

impl DMRegressionLine {
    pub fn new(point_1: Point, point_2: Point) -> Self {
        let mut new = Self::default();
        RegressionLineTrait::evaluate(&mut new, &[point_1, point_2]);
        new
    }

    // template <typename Container, typename Filter>
}
