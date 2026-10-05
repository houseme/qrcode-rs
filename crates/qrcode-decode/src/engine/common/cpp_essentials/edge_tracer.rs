// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
* Copyright 2017 Axel Waggershauser
*/
// SPDX-License-Identifier: Apache-2.0

use crate::engine::{Point, common::BitMatrix};

use super::{BitMatrixCursorTrait, Direction, Value};

#[derive(Clone, Debug)]
pub struct EdgeTracer<'a> {
    pub(crate) img: &'a BitMatrix,

    pub(crate) p: Point, // current position
    d: Point,            // current direction
}

impl BitMatrixCursorTrait for EdgeTracer<'_> {
    fn testAt(&self, p: Point) -> Value {
        if self.img.isIn(p, 0) { Value::from(self.img.get_point(p)) } else { Value::Invalid }
    }

    fn isIn(&self, p: Point) -> bool {
        self.img.isIn(p, 0)
    }

    fn isInSelf(&self) -> bool {
        self.isIn(self.p)
    }

    fn isBlack(&self) -> bool {
        self.blackAt(self.p)
    }

    fn front(&self) -> &Point {
        &self.d
    }

    fn back(&self) -> Point {
        Point { x: -self.d.x, y: -self.d.y }
    }

    fn left(&self) -> Point {
        Point { x: self.d.y, y: -self.d.x }
    }

    fn right(&self) -> Point {
        Point { x: -self.d.y, y: self.d.x }
    }

    fn turnBack(&mut self) {
        self.d = self.back()
    }

    fn turnLeft(&mut self) {
        self.d = self.left()
    }

    fn turnRight(&mut self) {
        self.d = self.right()
    }

    fn turn(&mut self, dir: Direction) {
        self.d = self.direction(dir)
    }

    fn edgeAt_point(&self, d: Point) -> Value {
        let v = self.testAt(self.p);
        if self.testAt(self.p + d) != v { v } else { Value::Invalid }
    }

    fn setDirection(&mut self, dir: Point) {
        self.d = dir.bresenhamDirection();
    }

    fn step(&mut self, s: Option<f32>) -> bool {
        let s = s.unwrap_or(1.0);
        self.p += self.d * s;
        self.isIn(self.p)
    }

    fn turnedBack(&self) -> Self {
        let mut res = self.clone();
        res.d = res.back();

        res
    }

    /**
     * @brief stepToEdge advances cursor to one step behind the next (or n-th) edge.
     * @param nth number of edges to pass
     * @param range max number of steps to take
     * @param backup whether or not to backup one step so we land in front of the edge
     * @return number of steps taken or 0 if moved outside of range/image
     */
    fn stepToEdge(&mut self, nth: Option<i32>, range: Option<i32>, backup: Option<bool>) -> i32 {
        let mut nth = nth.unwrap_or(1); //if let Some(nth) = nth { nth } else { 1 };
        let range = range.unwrap_or(0); //if let Some(r) = range { r } else { 0 };
        let backup = backup.unwrap_or(false); //if let Some(b) = backup { b } else { false };
        // TODO: provide an alternative and faster out-of-bounds check than isIn() inside testAt()
        let mut steps = 0;
        let mut lv = self.testAt(self.p);

        while nth > 0 && (range <= 0 || steps < range) && lv.isValid() {
            steps += 1;
            let v = self.testAt(self.p + steps * self.d);
            if lv != v {
                lv = v;
                nth -= 1;
            }
        }
        if backup {
            steps -= 1;
        }
        self.p += self.d * steps;
        steps * i32::from(nth == 0)
    }

    fn p(&self) -> Point {
        self.p
    }

    fn d(&self) -> Point {
        self.d
    }

    fn img(&self) -> &BitMatrix {
        self.img
    }
}

impl<'a> EdgeTracer<'_> {
    pub fn new(image: &'a BitMatrix, p: Point, d: Point) -> EdgeTracer<'a> {
        // : img(&image), p(p) { setDirection(d); }
        EdgeTracer {
            img: image,
            p,
            d: Point::bresenhamDirection(d), //d,
        }
    }
}
