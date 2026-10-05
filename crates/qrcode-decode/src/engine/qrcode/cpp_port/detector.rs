// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
/*
* Copyright 2016 Nu-book Inc.
* Copyright 2016 ZXing authors
* Copyright 2020 Axel Waggershauser
* Copyright 2023 gitlost
*/
// SPDX-License-Identifier: Apache-2.0

use crate::engine::{
    Exceptions,
    common::{
        DefaultGridSampler, GridSampler, Result, SamplerControl,
        cpp_essentials::{AppendBit, CenterOfRing, DMRegressionLine, FindConcentricPatternCorners, Matrix},
    },
    point, point_i,
    qrcode::{
        common::{FormatInformation, Version, VersionRef},
        detector::QRCodeDetectorResult,
    },
};

use crate::engine::{
    Point,
    common::{
        BitMatrix, PerspectiveTransform, Quadrilateral,
        cpp_essentials::{
            BitMatrixCursorTrait, ConcentricPattern, Direction, EdgeTracer, FitSquareToPoints, FixedPattern, IsPattern,
            PatternView, ReadSymmetricPattern, RegressionLine, RegressionLineTrait,
        },
    },
};

use super::Type;

#[cfg(test)]
use crate::engine::common::cpp_essentials::{
    FindLeftGuardBy, GetPatternRowTP, LocateConcentricPattern, PatternRow, PatternType,
};

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct FinderPatternSet {
    pub bl: ConcentricPattern,
    pub tl: ConcentricPattern,
    pub tr: ConcentricPattern,
}

pub type FinderPatterns = Vec<ConcentricPattern>;
pub type FinderPatternSets = Vec<FinderPatternSet>;

const LEN: usize = 5;
const SUM: usize = 7;
const PATTERN: FixedPattern<LEN, SUM, false> = FixedPattern::new([1, 1, 3, 1, 1]);
const E2E: bool = true;

#[cfg(test)]
fn FindPattern(view: PatternView<'_>, min_module_size: f32) -> Result<PatternView<'_>> {
    FindLeftGuardBy::<LEN, _>(view, LEN, |view: &PatternView, spaceInPixel: Option<f32>| {
        // perform a fast plausability test for 1:1:3:1:1 pattern
        if (view[2] as i32) < 3
            || view[2] < 2 as PatternType * std::cmp::max(view[0], view[4])
            || view[2] < std::cmp::max(view[1], view[3])
        {
            return false;
        }
        IsPattern::<E2E, 5, 7, false>(view, &PATTERN, spaceInPixel, 0.1, 0.0, min_module_size) != 0.0
    })
}

/// Locate the finder patterns for the symbol.
/// This function can panic
#[cfg(test)]
pub fn FindFinderPatterns(image: &BitMatrix, tryHarder: bool, min_module_size: u32) -> FinderPatterns {
    let min_skip = if min_module_size > 1 { 3 * min_module_size } else { 3 }; // 1 pixel/module times 3 modules/center
    const MAX_MODULES_FAST: u32 = 20 * 4 + 17; // support up to version 20 for mobile clients

    // Let's assume that the maximum version QR Code we support takes up 1/4 the height of the
    // image, and then account for the center being 3 modules in size. This gives the smallest
    // number of pixels the center could be, so skip this often. When trying harder, look for all
    // QR versions regardless of how dense they are.
    let height = image.height();
    let mut skip = (3 * height) / (4 * MAX_MODULES_FAST);
    if tryHarder {
        skip = 3;
    } else if skip < min_skip {
        skip = min_skip;
    }

    let mut res: Vec<ConcentricPattern> = Vec::new();
    let mut y = skip - 1;

    let mut row = PatternRow::default();
    while y < height {
        // for (int y = skip - 1; y < height; y += skip) {
        GetPatternRowTP(image, y, &mut row, false);
        let mut next: PatternView = PatternView::new(&row);

        while {
            if let Ok(up_next) = FindPattern(next, min_module_size as f32) {
                next = up_next;
                next.isValid()
            } else {
                false
            }
        } {
            let p = point(
                next.pixelsInFront() as f32 + next[0] as f32 + next[1] as f32 + next[2] as f32 / 2.0,
                y as f32 + 0.5,
            );

            // make sure p is not 'inside' an already found pattern area
            if !res.iter().any(|old| Point::distance(p, old.p) < (old.size as f32) / 2.0) {
                // if (FindIf(res, [p](const auto& old) { return distance(p, old) < old.size / 2; }) == res.end()) {
                let pattern = LocateConcentricPattern::<E2E, 5, 7>(
                    image,
                    &PATTERN.into(),
                    p,
                    next.iter().sum::<u16>() as i32 * 3,
                ); // 3 for very skewed samples
                //    Reduce(next) * 3); // 3 for very skewed samples
                if let Some(p) = pattern {
                    // log(*pattern, 3);
                    // assert!(image.get_point(pattern.as_ref().unwrap().p));
                    res.push(p);
                }
            }

            next.skipPair();
            next.skipPair();
            next.extend();
        }

        y += skip;
    }

    res
}

// Yields (dx, dy) offsets forming the square ring at exactly the given radius.
// Calling for r in 0..=max_r covers every cell in the square exactly once with no duplicates.
fn spiral(radius: i32) -> impl Iterator<Item = (i32, i32)> {
    let r = radius;
    let center = (r == 0).then_some((0, 0));
    let top = (-r..r).map(move |x| (x, -r));
    let right = (-r..r).map(move |y| (r, y));
    let bottom = (-r + 1..=r).rev().map(move |x| (x, r));
    let left = (-r + 1..=r).rev().map(move |y| (-r, y));
    center.into_iter().chain(top).chain(right).chain(bottom).chain(left)
}

/**
 * @brief GenerateFinderPatternSets
 * @param patterns list of ConcentricPattern objects, i.e. found finder pattern squares
 * @return list of plausible finder pattern sets, sorted by decreasing plausibility
 */
pub fn GenerateFinderPatternSets(patterns: &mut FinderPatterns) -> FinderPatternSets {
    generate_finder_pattern_sets::<true>(patterns).0
}

// The unbounded specialization is instantiated only by the test oracle.
fn generate_finder_pattern_sets<const CLAMP_TO_BINS: bool>(
    patterns: &mut FinderPatterns,
) -> (FinderPatternSets, usize) {
    #[cfg(test)]
    let mut visited_bins = 0;
    #[cfg(not(test))]
    let visited_bins = 0;
    patterns.sort_by_key(|b| std::cmp::Reverse(b.size)); // descending: larger patterns first (less likely to be noise)

    let mut sets: Vec<(f64, FinderPatternSet)> = Vec::new();
    let squaredDistance = |a: ConcentricPattern, b: ConcentricPattern| {
        // The scaling of the distance by the b/a size ratio is a very coarse compensation for the shortening effect of
        // the camera projection on slanted symbols. The fact that the size of the finder pattern is proportional to the
        // distance from the camera is used here. This approximation only works if a < b < 2*a (see below).
        // Test image: fix-finderpattern-order.jpg
        ConcentricPattern::dot(a - b, a - b) as f64 * (((b).size as f64) / ((a).size as f64)) // linear ratio (not squared) to avoid skewing cosine
    };

    let cosUpper: f64 = (60.0_f64 / 180.0 * std::f64::consts::PI).cos();
    let cosLower: f64 = (120.0_f64 / 180.0 * std::f64::consts::PI).cos();

    let nb_patterns = patterns.len();

    if nb_patterns < 3 {
        return (FinderPatternSets::default(), visited_bins);
    }

    // Compute bounding box of all pattern centers
    let min_x = patterns.iter().map(|p| p.p.x).fold(f32::INFINITY, f32::min);
    let max_x = patterns.iter().map(|p| p.p.x).fold(f32::NEG_INFINITY, f32::max);
    let min_y = patterns.iter().map(|p| p.p.y).fold(f32::INFINITY, f32::min);
    let max_y = patterns.iter().map(|p| p.p.y).fold(f32::NEG_INFINITY, f32::max);

    // Bin size based on median pattern size; patterns are in descending size order
    let median_size = patterns[nb_patterns / 2].size;
    let bin_size = std::cmp::max(32, median_size * 3) as f32;

    let bins_w = (((max_x - min_x + 1.0) / bin_size).ceil() as usize).max(1);
    let bins_h = (((max_y - min_y + 1.0) / bin_size).ceil() as usize).max(1);

    let mut bins: Vec<Vec<usize>> = vec![Vec::new(); bins_w * bins_h];
    let bin_idx = |p: Point| -> (usize, usize) {
        let bx = ((p.x - min_x) / bin_size) as usize;
        let by = ((p.y - min_y) / bin_size) as usize;
        (bx.min(bins_w - 1), by.min(bins_h - 1))
    };
    let bin_flat = |bx: usize, by: usize| by * bins_w + bx;

    for (idx, p) in patterns.iter().enumerate() {
        let (bx, by) = bin_idx(p.p);
        bins[bin_flat(bx, by)].push(idx);
    }

    const MAX_MODULE_COUNT: f64 = 177.0 * 1.5;
    const MAX_CANDIDATES: usize = 15;

    let mut candidates: Vec<usize> = Vec::with_capacity(MAX_CANDIDATES * 2);
    for i in 0..nb_patterns.saturating_sub(2) {
        let c0 = &patterns[i];
        let max_dist = c0.size as f64 / 7.0 * MAX_MODULE_COUNT;
        let (cx, cy) = bin_idx(c0.p);
        let radius = (max_dist / bin_size as f64).ceil() as i32;
        // Rings beyond the farthest bin cannot contain candidates. Removing
        // them leaves the order of every in-bounds bin and candidate unchanged.
        let bin_radius = if CLAMP_TO_BINS {
            let farthest_bin = cx.max(bins_w - 1 - cx).max(cy).max(bins_h - 1 - cy);
            radius.min(farthest_bin as i32)
        } else {
            radius
        };

        candidates.clear();

        'outer: for r in 0..=bin_radius {
            for (dx, dy) in spiral(r) {
                #[cfg(test)]
                {
                    visited_bins += 1;
                }
                let bx = cx as i32 + dx;
                let by = cy as i32 + dy;
                if bx < 0 || bx >= bins_w as i32 || by < 0 || by >= bins_h as i32 {
                    continue;
                }
                for &idx in &bins[bin_flat(bx as usize, by as usize)] {
                    if idx <= i {
                        continue;
                    }
                    if c0.size > patterns[idx].size * 2 {
                        continue;
                    }
                    candidates.push(idx);
                    if candidates.len() >= MAX_CANDIDATES {
                        break 'outer;
                    }
                }
            }
        }

        for u in 0..candidates.len().saturating_sub(1) {
            for v in (u + 1)..candidates.len() {
                let j = candidates[u];
                let k = candidates[v];

                // patterns sorted descending; geometry assumes a<=b<=c in size, so remap
                let mut a = &patterns[k]; // smallest (higher index = smaller due to desc sort)
                let mut b = &patterns[j];
                let mut c = &patterns[i]; // largest

                let mut distAB2 = squaredDistance(*a, *b);
                let mut distBC2 = squaredDistance(*b, *c);
                let mut distAC2 = squaredDistance(*a, *c);

                if distBC2 >= distAB2 && distBC2 >= distAC2 {
                    std::mem::swap(&mut a, &mut b);
                    std::mem::swap(&mut distBC2, &mut distAC2);
                } else if distAB2 >= distAC2 && distAB2 >= distBC2 {
                    std::mem::swap(&mut b, &mut c);
                    std::mem::swap(&mut distAB2, &mut distAC2);
                }

                if distAB2 > 4.0 * distBC2 || distBC2 > 4.0 * distAB2 {
                    continue;
                }

                let distAB = distAB2.sqrt();
                let distBC = distBC2.sqrt();

                let module_count = (distAB + distBC) / (2.0 * (a.size + b.size + c.size) as f64 / (3.0 * 7.0)) + 7.0;
                if !(21.0 * 0.9..=177.0 * 1.5).contains(&module_count) {
                    continue;
                }

                let cos_ab_bc = (distAB2 + distBC2 - distAC2) / (2.0 * distAB * distBC);
                if cos_ab_bc.is_nan() || cos_ab_bc > cosUpper || cos_ab_bc < cosLower {
                    continue;
                }

                if ConcentricPattern::cross(*c - *b, *a - *b) < 0.0 {
                    std::mem::swap(&mut a, &mut c);
                }

                // first order approximation of plausibility (lower = more plausible)
                let score = distAB + distBC + (distAB - distBC).abs();
                sets.push((score, FinderPatternSet { bl: *a, tl: *b, tr: *c }));
            }
        }
    }

    // ascending score: most plausible sets first
    sets.sort_by(|a, b| a.0.total_cmp(&b.0));

    (sets.into_iter().map(|(_, s)| s).collect(), visited_bins)
}

pub fn EstimateModuleSize(image: &BitMatrix, a: ConcentricPattern, b: ConcentricPattern) -> f64 {
    let mut cur = EdgeTracer::new(image, a.p, b.p - a.p);
    if !cur.isBlack() {
        return -1.0;
    }
    assert!(cur.isBlack());

    let pattern = ReadSymmetricPattern::<5, _>(&mut cur, a.size * 2);

    if pattern.is_none() {
        return -1.0;
    }

    let pattern = pattern.unwrap();

    if !(IsPattern::<E2E, 5, 7, false>(&PatternView::from_slice(&pattern), &PATTERN, None, 0.0, 0.0, 0.0) != 0.0) {
        return -1.0;
    }

    // Widen before summing and doubling run lengths; accepted image axes can
    // reach 32768, which is already half of the u16 pattern representation.
    let total = pattern.iter().copied().map(u32::from).sum::<u32>();
    (2 * total - u32::from(pattern[0]) - u32::from(pattern[4])) as f64 / 12.0 * cur.d().length() as f64
    //  (2 * Reduce(*pattern) - (*pattern)[0] - (*pattern)[4]) / 12.0 * length(cur.d)
}

pub struct DimensionEstimate {
    dim: i32,
    ms: f64,
    err: i32,
}

impl Default for DimensionEstimate {
    fn default() -> Self {
        Self { dim: 0, ms: 0.0, err: 4 }
    }
}

pub fn EstimateDimension(image: &BitMatrix, a: ConcentricPattern, b: ConcentricPattern) -> Result<DimensionEstimate> {
    if !valid_finder(image, a) || !valid_finder(image, b) || a.p == b.p {
        return Err(Exceptions::NOT_FOUND);
    }
    let ms_a = EstimateModuleSize(image, a, b);
    let ms_b = EstimateModuleSize(image, b, a);
    if !ms_a.is_finite() || !ms_b.is_finite() || ms_a <= 0.0 || ms_b <= 0.0 {
        return Err(Exceptions::NOT_FOUND);
    }
    dimension_estimate(ConcentricPattern::distance(a, b) as f64, (ms_a + ms_b) / 2.0)
}

fn valid_finder(image: &BitMatrix, finder: ConcentricPattern) -> bool {
    finder.size > 0 && finder.size <= i32::MAX / 2 && image.is_in(finder.p)
}

fn dimension_estimate(distance: f64, module_size: f64) -> Result<DimensionEstimate> {
    if !distance.is_finite()
        || distance <= 0.0
        || !module_size.is_finite()
        || module_size <= 0.0
        || module_size + 1.0 > i32::MAX as f64
    {
        return Err(Exceptions::NOT_FOUND);
    }
    let quotient = distance / module_size;
    // Keep headroom for the finder width and the 1-mod-4 adjustment.
    if !quotient.is_finite() || quotient.round() > (i32::MAX - 8) as f64 {
        return Err(Exceptions::NOT_FOUND);
    }
    let dimension = quotient.round() as i32 + 7;
    let error = 1 - dimension % 4;
    let dimension = dimension + error;
    if dimension < 21 {
        return Err(Exceptions::NOT_FOUND);
    }
    // Coarse estimates above 177 remain admissible here: a high-version
    // symbol may recover its exact dimension from BCH version information.
    Ok(DimensionEstimate { dim: dimension, ms: module_size, err: error.abs() })
}

/// This function can panic
pub fn TraceLine(image: &BitMatrix, p: Point, d: Point, edge: i32) -> impl RegressionLineTrait {
    let mut cur = EdgeTracer::new(image, p, d - p);
    let mut line = RegressionLine::default();
    line.setDirectionInward(cur.back());

    // collect points inside the black line -> backup on 3rd edge
    cur.stepToEdge(Some(edge), Some(0), Some(edge == 3));
    if edge == 3 {
        cur.turnBack();
    }

    let mut curI = EdgeTracer::new(image, cur.p, Point::mainDirection(cur.d()));
    // make sure curI positioned such that the white->black edge is directly behind
    // Test image: fix-traceline.jpg
    while curI.isInSelf() && !bool::from(curI.edgeAtBack()) {
        if curI.edgeAtLeft().into() {
            curI.turnRight();
        } else if curI.edgeAtRight().into() {
            curI.turnLeft();
        } else {
            curI.step(Some(-1.0));
        }
    }

    for dir in [Direction::Left, Direction::Right] {
        // for (auto dir : {Direction::LEFT, Direction::RIGHT}) {
        let mut c = EdgeTracer::new(image, curI.p, curI.direction(dir));
        let mut stepCount = (Point::maxAbsComponent(cur.p - p)) as i32;
        loop {
            line.add(Point::centered(c.p)).expect("could not add point on line");

            stepCount -= 1;
            if !(stepCount > 0 && c.stepAlongEdge(dir, Some(true))) {
                break;
            }
        } //while (--stepCount > 0 && c.stepAlongEdge(dir, true));
    }

    line.evaluate_max_distance(Some(1.0), Some(true));

    line
}

// estimate how tilted the symbol is (return value between 1 and 2, see also above)
pub fn EstimateTilt(fp: &FinderPatternSet) -> f64 {
    let min = [fp.bl.size, fp.tl.size, fp.tr.size].iter().min().copied().unwrap_or(i32::MAX);
    let max = [fp.bl.size, fp.tl.size, fp.tr.size].iter().max().copied().unwrap_or(i32::MIN);

    (max as f64) / (min as f64)
}

pub fn Mod2Pix(dimension: i32, brOffset: Point, pix: Quadrilateral) -> Result<PerspectiveTransform> {
    let mut quad = Quadrilateral::rectangle(dimension, dimension, Some(3.5));
    // let quad = Rectangle(dimension, dimension, 3.5);
    quad[2] -= brOffset;

    PerspectiveTransform::quadrilateralToQuadrilateral(quad, pix)
    // return {quad, pix};
}

pub fn LocateAlignmentPattern(image: &BitMatrix, moduleSize: i32, estimate: Point) -> Option<Point> {
    // log(estimate, 2);

    for d in [
        point(0.0, 0.0),
        point(0.0, -1.0),
        point(0.0, 1.0),
        point(-1.0, 0.0),
        point(1.0, 0.0),
        point(-1.0, -1.0),
        point(1.0, -1.0),
        point(1.0, 1.0),
        point(-1.0, 1.0),
    ] {
        // 	for (auto d : {PointF{0, 0}, {0, -1}, {0, 1}, {-1, 0}, {1, 0}, {-1, -1}, {1, -1}, {1, 1}, {-1, 1},
        // #if 1
        // 				   }) {
        // #else
        // 				   {0, -2}, {0, 2}, {-2, 0}, {2, 0}, {-1, -2}, {1, -2}, {-1, 2}, {1, 2}, {-2, -1}, {-2, 1}, {2, -1}, {2, 1}}) {
        // #endif
        let p = (estimate + moduleSize as f32 * 2.25 * d).floor();
        if !image.is_in(p) {
            continue;
        }
        let cor = CenterOfRing(image, p, moduleSize * 3, 1, false);

        // if we did not land on a black pixel the concentric pattern finder will fail
        if cor.is_none() || !image.get_point(cor.unwrap()) {
            continue;
        }

        if let Some(cor1) = CenterOfRing(image, cor.unwrap().floor(), moduleSize, 1, true)
            && let Some(cor2) = CenterOfRing(image, cor.unwrap().floor(), moduleSize * 3, -2, true)
            && Point::distance(cor1, cor2) < moduleSize as f32 / 2.0
        {
            let res = (cor1 + cor2) / 2.0;
            // log(res, 3);
            return Some(res);
        }
    }

    None
}

pub fn ReadVersion(image: &BitMatrix, dimension: u32, mod2Pix: PerspectiveTransform) -> Result<VersionRef> {
    let mut bits = [0; 2]; //

    for mirror in [false, true] {
        // Read top-right/bottom-left version info: 3 wide by 6 tall (depending on mirrored)
        let mut versionBits = 0;
        for y in (0..=5).rev() {
            // for (int y = 5; y >= 0; --y)
            for x in ((dimension - 11)..=(dimension - 9)).rev() {
                // for (int x = dimension - 9; x >= dimension - 11; --x) {
                let mod_ = if mirror { point_i(y, x) } else { point_i(x, y) };
                let pix = mod2Pix.transform_point((mod_).centered());
                if !image.is_in(pix) {
                    versionBits = -1;
                } else {
                    AppendBit(&mut versionBits, image.get_point(pix));
                }
                // log(pix, 3);
            }
            bits[usize::from(mirror)] = versionBits;
        }
    }

    Version::DecodeVersionInformation(bits[0], bits[1])
}

pub fn SampleQR(image: &BitMatrix, fp: &FinderPatternSet) -> Result<QRCodeDetectorResult> {
    if [fp.tl, fp.tr, fp.bl].into_iter().any(|finder| !valid_finder(image, finder))
        || fp.tl.p == fp.tr.p
        || fp.tl.p == fp.bl.p
        || fp.tr.p == fp.bl.p
    {
        return Err(Exceptions::NOT_FOUND);
    }
    let top = EstimateDimension(image, fp.tl, fp.tr).unwrap_or_default();
    let left = EstimateDimension(image, fp.tl, fp.bl).unwrap_or_default();

    if top.dim == 0 && left.dim == 0 {
        return Err(Exceptions::NOT_FOUND);
    }

    let top_dim = top.dim;
    let left_dim = left.dim;

    let best = match top.err.cmp(&left.err) {
        std::cmp::Ordering::Less => top,
        std::cmp::Ordering::Equal => {
            if top.dim > left.dim {
                top
            } else {
                left
            }
        }
        std::cmp::Ordering::Greater => left,
    };

    // let best = if top.err == left.err {
    //     if top.dim > left.dim {
    //         top
    //     } else {
    //         left
    //     }
    // } else if top.err < left.err {
    //     top
    // } else {
    //     left
    // };
    let mut dimension = best.dim;
    if dimension < 21 || dimension % 4 != 1 {
        return Err(Exceptions::NOT_FOUND);
    }
    let moduleSize = (best.ms + 1.0) as i32;

    let mut br = ConcentricPattern { p: point(-1.0, -1.0), size: 0 };
    let mut brOffset = point_i(3, 3);

    // Everything except version 1 (21 modules) has an alignment pattern. Estimate the center of that by intersecting
    // line extensions of the 1 module wide square around the finder patterns. This could also help with detecting
    // slanted symbols of version 1.

    // generate 4 lines: outer and inner edge of the 1 module wide black line between the two outer and the inner
    // (tl) finder pattern
    let bl2 = TraceLine(image, fp.bl.p, fp.tl.p, 2);
    let bl3 = TraceLine(image, fp.bl.p, fp.tl.p, 3);
    let tr2 = TraceLine(image, fp.tr.p, fp.tl.p, 2);
    let tr3 = TraceLine(image, fp.tr.p, fp.tl.p, 3);

    if bl2.isValid() && tr2.isValid() && bl3.isValid() && tr3.isValid() {
        // intersect both outer and inner line pairs and take the center point between the two intersection points
        let brInter = (DMRegressionLine::intersect(&bl2, &tr2).ok_or(Exceptions::NOT_FOUND)?
            + DMRegressionLine::intersect(&bl3, &tr3).ok_or(Exceptions::NOT_FOUND)?)
            / 2.0;
        // log(brInter, 3);

        if dimension > 21
            && let Some(brCP) = LocateAlignmentPattern(image, moduleSize, brInter)
        {
            br = brCP.into();
        }

        // if the symbol is tilted or the resolution of the RegressionLines is sufficient, use their intersection
        // as the best estimate (see discussion in #199 and test image estimate-tilt.jpg )
        if !image.is_in(br.p)
            && (EstimateTilt(fp) > 1.1 || (bl2.isHighRes() && bl3.isHighRes() && tr2.isHighRes() && tr3.isHighRes()))
        {
            br = brInter.into();
        }
    }

    // otherwise the simple estimation used by upstream is used as a best guess fallback
    if !image.is_in(br.p) || FitSquareToPoints(image, fp.bl.p, fp.bl.size, 2, false).is_none() {
        br = fp.tr - fp.tl + fp.bl;
        brOffset = point_i(0, 0);
    }

    // log(br, 3);
    let mut mod2Pix = Mod2Pix(dimension, brOffset, Quadrilateral::from([fp.tl.p, fp.tr.p, br.p, fp.bl.p]))?;

    if dimension >= Version::SymbolSize(7, Type::Model2).x {
        let version = ReadVersion(image, dimension as u32, mod2Pix);

        // if the version bits are garbage -> discard the detection
        if version.is_err()
            || std::cmp::min(
                (version.as_ref().unwrap().getDimensionForVersion() as i32 - top_dim).abs(),
                (version.as_ref().unwrap().getDimensionForVersion() as i32 - left_dim).abs(),
            ) > 8
        {
            /*return DetectorResult();*/
            return Err(Exceptions::NOT_FOUND);
        }
        if version.as_ref().unwrap().getDimensionForVersion() as i32 != dimension {
            // printf("update dimension: %d -> %d\n", dimension, version.dimension());
            dimension = version.as_ref().unwrap().getDimensionForVersion() as i32;
            mod2Pix = Mod2Pix(dimension, brOffset, Quadrilateral::from([fp.tl.p, fp.tr.p, br.p, fp.bl.p]))?;
        }
        // #if 1
        if !Version::IsValidSize(point_i(dimension, dimension).into(), Type::Model2) {
            return Err(Exceptions::NOT_FOUND);
        }
        let apM = version.as_ref().unwrap().getAlignmentPatternCenters(); // alignment pattern positions in modules
        let mut apP = Matrix::new(apM.len(), apM.len())?; // found/guessed alignment pattern positions in pixels
        let N = (apM.len()) - 1;

        // project the alignment pattern at module coordinates x/y to pixel coordinate based on current mod2Pix
        let projectM2P =
            |x, y, mod2Pix: &PerspectiveTransform| mod2Pix.transform_point(Point::centered(point_i(apM[x], apM[y])));

        let mut findInnerCornerOfConcentricPattern = |x, y, fp: ConcentricPattern| {
            let pc = apP.set(x, y, projectM2P(x, y, &mod2Pix));
            if let Some(fpQuad) = FindConcentricPatternCorners(image, fp.p, fp.size, 2) {
                for c in fpQuad.0 {
                    if Point::distance(c, pc) < (fp.size as f32) / 2.0 {
                        apP.set(x, y, c);
                    }
                }
            }
        };

        findInnerCornerOfConcentricPattern(0, 0, fp.tl);
        findInnerCornerOfConcentricPattern(0, N, fp.bl);
        findInnerCornerOfConcentricPattern(N, 0, fp.tr);

        let bestGuessAPP = |x, y, apP: &Matrix<Point>| {
            if let Some(p) = apP.get(x, y)
            // if (auto p = apP(x, y))
            {
                return p;
            }
            projectM2P(x, y, &mod2Pix)
        };

        for y in 0..=N {
            // for (int y = 0; y <= N; ++y)
            for x in 0..=N {
                // for (int x = 0; x <= N; ++x) {
                if apP.get(x, y).is_some() {
                    continue;
                }

                let guessed = if x * y == 0 {
                    bestGuessAPP(x, y, &apP)
                } else {
                    bestGuessAPP(x - 1, y, &apP) + bestGuessAPP(x, y - 1, &apP) - bestGuessAPP(x - 1, y - 1, &apP)
                };
                if let Some(found) = LocateAlignmentPattern(image, moduleSize, guessed)
                // if (auto found = LocateAlignmentPattern(image, moduleSize, guessed))
                {
                    apP.set(x, y, found);
                }
            }
        }

        // go over the whole set of alignment patters again and try to fill any remaining gap by using available neighbors as guides
        let mut hori = Vec::new();
        let mut verti = Vec::new();
        for y in 0..=N {
            // for (int y = 0; y <= N; ++y) {
            for x in 0..=N {
                // for (int x = 0; x <= N; ++x) {
                if apP.get(x, y).is_some() {
                    continue;
                }

                // find the two closest valid alignment pattern pixel positions both horizontally and vertically
                hori.clear();
                verti.clear();
                let mut i = 2;
                while i < 2 * N + 2 && hori.len() < 2 {
                    let xi = x as isize + i as isize / 2 * (if i % 2 != 0 { 1 } else { -1 });
                    if 0 <= xi
                        && xi <= N as isize
                        && let Some(p) = apP.get(xi as usize, y)
                    {
                        hori.push(p);
                    }
                    i += 1;
                }
                // for (int i = 2; i < 2 * N + 2 && Size(hori) < 2; ++i) {
                // 	let xi = x + i / 2 * (i%2 ? 1 : -1);
                // 	if (0 <= xi && xi <= N && apP(xi, y))
                // 		{hori.push_back(*apP(xi, y));}
                // }
                let mut i = 2;
                while i < 2 * N + 2 && verti.len() < 2 {
                    let yi = y as isize + i as isize / 2 * (if i % 2 != 0 { 1 } else { -1 });
                    if 0 <= yi
                        && yi <= N as isize
                        && let Some(p) = apP.get(x, yi as usize)
                    {
                        verti.push(p);
                    }
                    i += 1;
                }
                // for (int i = 2; i < 2 * N + 2 && Size(verti) < 2; ++i) {
                // 	let yi = y + i / 2 * (i%2 ? 1 : -1);
                // 	if (0 <= yi && yi <= N && apP(x, yi))
                // 		{verti.push_back(*apP(x, yi));}
                // }

                // if we found 2 each, intersect the two lines that are formed by connecting the point pairs
                if (hori.len()) == 2 && (verti.len()) == 2 {
                    let guessed = RegressionLine::intersect(
                        &DMRegressionLine::new(hori[0], hori[1]),
                        &DMRegressionLine::new(verti[0], verti[1]),
                    )
                    .ok_or(Exceptions::ILLEGAL_STATE)?;
                    let found = LocateAlignmentPattern(image, moduleSize, guessed);
                    // search again near that intersection and if the search fails, use the intersection
                    // if (!found.is_some()) {printf("location guessed at %dx%d\n", x, y)};
                    apP.set(x, y, if let Some(f) = found { f } else { guessed });
                }
            }
        }

        if let Some(c) = apP.get(N, N)
        // if (auto c = apP.get(N, N))
        {
            mod2Pix = Mod2Pix(dimension, point_i(3, 3), Quadrilateral::from([fp.tl.p, fp.tr.p, c, fp.bl.p]))?;
        }

        // go over the whole set of alignment patters again and fill any remaining gaps by a projection based on an updated mod2Pix
        // projection. This works if the symbol is flat, wich is a reasonable fall-back assumption.
        for y in 0..=N {
            // for (int y = 0; y <= N; ++y) {
            for x in 0..=N {
                // for (int x = 0; x <= N; ++x) {
                if apP.get(x, y).is_some() {
                    continue;
                }

                // printf("locate failed at %dx%d\n", x, y);
                apP.set(x, y, projectM2P(x, y, &mod2Pix));
            }
        }

        // assemble a list of region-of-interests based on the found alignment pattern pixel positions

        let mut rois = Vec::new();
        for y in 0..N {
            // for (int y = 0; y < N; ++y){
            for x in 0..N {
                // for (int x = 0; x < N; ++x) {
                let x0 = apM[x];
                let x1 = apM[x + 1];
                let y0 = apM[y];
                let y1 = apM[y + 1];
                rois.push(SamplerControl {
                    p0: point_i(x0 - u32::from(x == 0) * 6, y0 - u32::from(y == 0) * 6),
                    p1: point_i(x1 + u32::from(x == N - 1) * 7, y1 + u32::from(y == N - 1) * 7),
                    transform: PerspectiveTransform::quadrilateralToQuadrilateral(
                        Quadrilateral::rectangle_from_xy(x0 as f32, x1 as f32, y0 as f32, y1 as f32, None),
                        Quadrilateral::from([
                            apP.get(x, y).unwrap(),
                            apP.get(x + 1, y).unwrap(),
                            apP.get(x + 1, y + 1).unwrap(),
                            apP.get(x, y + 1).unwrap(),
                        ]),
                    )?,
                });
            }
        }
        let grid_sampler = DefaultGridSampler;
        let (sampled, _) = grid_sampler.sample_grid(image, dimension as u32, dimension as u32, &rois)?;
        let result = QRCodeDetectorResult::new(sampled);
        return Ok(result);
        //  grid_sampler.sample_grid(image, dimension, dimension, &rois);
        // #endif
    }

    let grid_sampler = DefaultGridSampler;
    let (sampled, _) = grid_sampler.sample_grid(
        image,
        dimension as u32,
        dimension as u32,
        &[SamplerControl { p1: point_i(dimension as u32, dimension as u32), p0: point_i(0, 0), transform: mod2Pix }],
    )?;
    let result = QRCodeDetectorResult::new(sampled);
    Ok(result)
    // return SampleGrid(image, dimension, dimension, mod2Pix);
}

pub fn SampleMQR(image: &BitMatrix, fp: ConcentricPattern) -> Result<QRCodeDetectorResult> {
    if !valid_finder(image, fp) {
        return Err(Exceptions::NOT_FOUND);
    }
    let Some(fpQuad) = FindConcentricPatternCorners(image, fp.p, fp.size, 2) else {
        return Err(Exceptions::NOT_FOUND);
    };

    let srcQuad = Quadrilateral::rectangle(7, 7, Some(0.5));

    // #if defined(_MSVC_LANG) // TODO: see MSVC issue https://developercommunity.visualstudio.com/t/constexpr-object-is-unable-to-be-used-as/10035065
    // 	static
    // #else
    // 	constexpr
    // #endif
    let FORMAT_INFO_COORDS: [Point; 17] = [
        point_i(0, 8),
        point_i(1, 8),
        point_i(2, 8),
        point_i(3, 8),
        point_i(4, 8),
        point_i(5, 8),
        point_i(6, 8),
        point_i(7, 8),
        point_i(8, 8),
        point_i(8, 7),
        point_i(8, 6),
        point_i(8, 5),
        point_i(8, 4),
        point_i(8, 3),
        point_i(8, 2),
        point_i(8, 1),
        point_i(8, 0),
    ];

    let mut bestFI = FormatInformation::default();
    let mut bestPT =
        PerspectiveTransform::quadrilateralToQuadrilateral(srcQuad, fpQuad.rotated_corners(Some(0), None))?;
    let cur = EdgeTracer::new(image, Point::default(), Point::default());

    for i in 0..4 {
        // for (int i = 0; i < 4; ++i) {
        let mod2Pix =
            PerspectiveTransform::quadrilateralToQuadrilateral(srcQuad, fpQuad.rotated_corners(Some(i), None))?;

        let check = |i, checkOne: bool| {
            let p = mod2Pix.transform_point(Point::centered(FORMAT_INFO_COORDS[i]));
            image.is_in(p) && (!checkOne || image.get_point(p))
        };

        // check that we see both innermost timing pattern modules
        if !check(0, true) || !check(8, false) || !check(16, true) {
            continue;
        }

        let mut formatInfoBits = 0;
        for info_coord in FORMAT_INFO_COORDS.iter().take(15 + 1).skip(1)
        // for i in 1..=15
        // for (int i = 1; i <= 15; ++i)
        {
            AppendBit(&mut formatInfoBits, cur.blackAt(mod2Pix.transform_point(Point::centered(*info_coord))));
        }

        let fi = FormatInformation::DecodeMQR(formatInfoBits as u32);
        if fi.hammingDistance < bestFI.hammingDistance {
            bestFI = fi;
            bestPT = mod2Pix;
        }
    }

    if !bestFI.isValid() {
        return Err(Exceptions::NOT_FOUND);
    }

    let dim: u32 = Version::SymbolSize(bestFI.microVersion, Type::Micro).x as u32;

    // check that we are in fact not looking at a corner of a non-micro QRCode symbol
    // we accept at most 1/3rd black pixels in the quite zone (in a QRCode symbol we expect about 1/2).
    let mut blackPixels = 0;
    for i in 0..dim {
        // for (int i = 0; i < dim; ++i) {
        let px = bestPT.transform_point(Point::centered(point_i(i, dim)));
        let py = bestPT.transform_point(Point::centered(point_i(dim, i)));
        blackPixels += u32::from(cur.blackAt(px)) + u32::from(cur.blackAt(py));
    }
    if blackPixels > 2 * dim / 3 {
        return Err(Exceptions::NOT_FOUND);
    }

    let grid_sampler = DefaultGridSampler;
    let (sample, _) = grid_sampler.sample_grid(
        image,
        dim,
        dim,
        &[SamplerControl { p1: point_i(dim, dim), p0: point_i(0, 0), transform: bestPT }],
    )?;
    Ok(QRCodeDetectorResult::new(sample))

    //  SampleGrid(image, dim, dim, bestPT)
}

#[cfg(test)]
mod bounds_tests {
    use super::*;
    use crate::engine::common::DetectorRXingResult;
    use crate::engine::qrcode::cpp_port::decoder::Decode;
    use qrcode_core::bits::Bits;
    use qrcode_core::canvas::Canvas;
    use qrcode_core::{Color, EcLevel, Version as CoreVersion};

    fn pattern(x: f32, y: f32, size: i32) -> ConcentricPattern {
        ConcentricPattern { p: point(x, y), size }
    }

    #[test]
    fn bin_clamp_preserves_exact_candidate_order_against_unbounded_oracle() {
        for size in [7, 14, 28, 56] {
            let patterns = vec![
                pattern(0.0, 0.0, size),
                pattern(80.0, 0.0, size),
                pattern(0.0, 80.0, size),
                pattern(80.0, 80.0, size),
                pattern(32.0, 32.0, 7),
                pattern(96.0, 32.0, 14),
            ];
            let (bounded, bounded_visits) = generate_finder_pattern_sets::<true>(&mut patterns.clone());
            let (oracle, oracle_visits) = generate_finder_pattern_sets::<false>(&mut patterns.clone());
            assert_eq!(bounded, oracle);
            assert!(bounded_visits <= oracle_visits);
        }
    }

    #[test]
    fn mixed_huge_and_small_finders_visit_only_rings_covering_real_bins() {
        let mut patterns =
            vec![pattern(0.0, 0.0, 4000), pattern(4000.0, 0.0, 7), pattern(0.0, 4000.0, 7), pattern(4000.0, 4000.0, 7)];
        let (_, visits) = generate_finder_pattern_sets::<true>(&mut patterns);
        // There are at most 126 bins per axis and two visited anchors.
        // The unclamped huge anchor would have radius 4742 and over 89M
        // visits; that oracle is deliberately not run for this fixture.
        let per_anchor_bound = 1 + 4 * 125 * 126;
        assert!(visits <= 2 * per_anchor_bound, "{visits}");
        assert!(visits < 200_000);
    }

    #[test]
    fn dimension_estimates_reject_nonfinite_or_unrepresentable_geometry() {
        for (distance, module_size) in [
            (f64::NAN, 1.0),
            (f64::INFINITY, 1.0),
            (100.0, f64::NAN),
            (100.0, f64::INFINITY),
            (100.0, 0.0),
            (100.0, -1.0),
            (f64::MAX, f64::MIN_POSITIVE),
            (i32::MAX as f64, 1.0),
            (100.0, i32::MAX as f64),
            (0.0, 1.0),
            (1.0, 1.0),
        ] {
            assert!(dimension_estimate(distance, module_size).is_err());
        }
        let image = BitMatrix::new(32, 32).unwrap();
        assert!(EstimateDimension(&image, pattern(1.0, 1.0, 0), pattern(2.0, 2.0, 7)).is_err());
        assert!(EstimateDimension(&image, pattern(f32::NAN, 1.0, 7), pattern(2.0, 2.0, 7)).is_err());
    }

    #[test]
    fn coarse_high_version_dimensions_are_left_for_bch_correction() {
        // 174 modules between finder centers yields a coarse 181-module
        // estimate. The exact factory version is checked after BCH recovery.
        assert_eq!(dimension_estimate(174.0, 1.0).unwrap().dim, 181);
        assert_eq!(dimension_estimate(170.0, 1.0).unwrap().dim, 177);
        assert_eq!(dimension_estimate(14.0, 1.0).unwrap().dim, 21);
    }

    fn raster(version: CoreVersion, ec: EcLevel, module: u32) -> BitMatrix {
        let mut payload = Bits::new(version);
        payload.push_numeric_data(b"123").unwrap();
        payload.push_terminator(ec).unwrap();
        let (data, correction) = qrcode_core::ec::construct_codewords(&payload.into_bytes(), version, ec).unwrap();
        let mut canvas = Canvas::new(version, ec);
        canvas.draw_all_functional_patterns();
        canvas.draw_data(&data, &correction);
        let colors = canvas.apply_best_mask().into_colors();
        let width = version.width() as u32;
        let quiet = if version.is_micro() { 2 } else { 4 };
        let mut image = BitMatrix::new((width + 2 * quiet) * module, (width + 2 * quiet) * module).unwrap();
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
    fn normal_and_micro_sampling_retain_rotated_boundary_versions() {
        for (version, ec, module) in [
            (CoreVersion::Normal(1), EcLevel::L, 4),
            (CoreVersion::Normal(7), EcLevel::H, 4),
            (CoreVersion::Normal(40), EcLevel::M, 3),
            (CoreVersion::Micro(1), EcLevel::L, 4),
            (CoreVersion::Micro(4), EcLevel::Q, 4),
        ] {
            let original = raster(version, ec, module);
            for degrees in [0, 90, 180, 270] {
                let mut image = original.clone();
                image.rotate(degrees).unwrap();
                let mut finders = FindFinderPatterns(&image, true, 0);
                let expected_width = version.width() as u32;
                if version.is_micro() {
                    let sampled = finders
                        .iter()
                        .find_map(|finder| {
                            let sampled = SampleMQR(&image, *finder).ok()?;
                            (sampled.getBits().width() == expected_width).then_some(sampled)
                        })
                        .expect("valid rotated Micro QR must sample");
                    let format = super::super::bitmatrix_parser::ReadFormatInformation(sampled.getBits()).unwrap();
                    assert!(format.isValid());
                    assert_eq!(format.microVersion * 2 + 9, expected_width);
                } else {
                    let found = GenerateFinderPatternSets(&mut finders)
                        .iter()
                        .find_map(|set| {
                            let sampled = SampleQR(&image, set).ok()?;
                            let decoded = Decode(sampled.getBits()).ok()?;
                            (sampled.getBits().width() == expected_width && decoded.isValid()).then_some(decoded)
                        })
                        .expect("valid rotated normal QR must sample and decode");
                    assert_eq!(found.content().bytes(), b"123");
                }
            }
        }
    }
}
