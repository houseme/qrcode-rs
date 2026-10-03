#![allow(clippy::unicode_not_nfc)]
//! Data mode segmentation optimizer.
//!
//! QR codes support four data modes (Numeric, Alphanumeric, Byte, Kanji),
//! each with different efficiency for different character types. This module
//! finds the optimal sequence of mode switches to minimize the total number
//! of bits required to encode the input data.
//!
//! The optimizer uses dynamic programming to explore all possible mode
//! transitions and selects the segmentation that produces the shortest
//! bit stream for the target QR code version.
#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use crate::types::{Mode, Version};
use core::cmp::Reverse;
use core::marker::PhantomData;
use core::slice::Iter;

//------------------------------------------------------------------------------
//{{{ Segment

/// A segment of data committed to an encoding mode.
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub struct Segment {
    /// The encoding mode of the segment of data.
    pub mode: Mode,

    /// The start index of the segment.
    pub begin: usize,

    /// The end index (exclusive) of the segment.
    pub end: usize,
}

impl Segment {
    /// Compute the number of bits (including the size of the mode indicator and
    /// length bits) when this segment is encoded.
    pub fn encoded_len(&self, version: Version) -> usize {
        let byte_size = self.end - self.begin;
        let chars_count = if self.mode == Mode::Kanji { byte_size / 2 } else { byte_size };

        let mode_bits_count = version.mode_bits_count();
        let length_bits_count = self.mode.length_bits_count(version);
        let data_bits_count = self.mode.data_bits_count(chars_count);

        mode_bits_count + length_bits_count + data_bits_count
    }
}

//}}}
//------------------------------------------------------------------------------
//{{{ Parser

/// This iterator is basically equivalent to
///
/// ```ignore
/// data.map(|c| ExclCharSet::from_u8(*c))
///     .chain(Some(ExclCharSet::End).move_iter())
///     .enumerate()
/// ```
///
/// But the type is too hard to write, thus the new type.
///
struct EcsIter<I> {
    base: I,
    index: usize,
    ended: bool,
}

impl<'a, I: Iterator<Item = &'a u8>> Iterator for EcsIter<I> {
    type Item = (usize, ExclCharSet);

    fn next(&mut self) -> Option<(usize, ExclCharSet)> {
        if self.ended {
            return None;
        }

        match self.base.next() {
            None => {
                self.ended = true;
                Some((self.index, ExclCharSet::End))
            }
            Some(c) => {
                let old_index = self.index;
                self.index += 1;
                Some((old_index, ExclCharSet::from_u8(*c)))
            }
        }
    }
}

/// QR code data parser to classify the input into distinct segments.
pub struct Parser<'a> {
    ecs_iter: EcsIter<Iter<'a, u8>>,
    state: State,
    begin: usize,
    pending_single_byte: bool,
}

impl<'a> Parser<'a> {
    /// Creates a new iterator which parse the data into segments that only
    /// contains their exclusive subsets. No optimization is done at this point.
    ///
    ///     use qrcode_core::optimize::{Parser, Segment};
    ///     use qrcode_core::types::Mode::{Alphanumeric, Numeric, Byte};
    ///
    ///     let parse_res = Parser::new(b"ABC123abcd").collect::<Vec<Segment>>();
    ///     assert_eq!(parse_res, vec![Segment { mode: Alphanumeric, begin: 0, end: 3 },
    ///                                Segment { mode: Numeric, begin: 3, end: 6 },
    ///                                Segment { mode: Byte, begin: 6, end: 10 }]);
    ///
    pub fn new(data: &[u8]) -> Parser<'_> {
        Parser {
            ecs_iter: EcsIter { base: data.iter(), index: 0, ended: false },
            state: State::Init,
            begin: 0,
            pending_single_byte: false,
        }
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Segment;

    fn next(&mut self) -> Option<Segment> {
        if self.pending_single_byte {
            self.pending_single_byte = false;
            self.begin += 1;
            return Some(Segment { mode: Mode::Byte, begin: self.begin - 1, end: self.begin });
        }

        loop {
            let (i, ecs) = self.ecs_iter.next()?;
            let (next_state, action) = STATE_TRANSITION[self.state as usize + ecs as usize];
            self.state = next_state;

            let old_begin = self.begin;
            let push_mode = match action {
                Action::Idle => continue,
                Action::Numeric => Mode::Numeric,
                Action::Alpha => Mode::Alphanumeric,
                Action::Byte => Mode::Byte,
                Action::Kanji => Mode::Kanji,
                Action::KanjiAndSingleByte => {
                    let next_begin = i - 1;
                    if self.begin == next_begin {
                        Mode::Byte
                    } else {
                        self.pending_single_byte = true;
                        self.begin = next_begin;
                        return Some(Segment { mode: Mode::Kanji, begin: old_begin, end: next_begin });
                    }
                }
            };

            self.begin = i;
            return Some(Segment { mode: push_mode, begin: old_begin, end: i });
        }
    }
}

#[cfg(test)]
mod parse_tests {
    use crate::optimize::{Parser, Segment};
    use crate::types::Mode;

    fn parse(data: &[u8]) -> Vec<Segment> {
        Parser::new(data).collect()
    }

    #[test]
    fn test_parse_1() {
        let segs = parse(b"01049123451234591597033130128%10ABC123");
        assert_eq!(
            segs,
            vec![
                Segment { mode: Mode::Numeric, begin: 0, end: 29 },
                Segment { mode: Mode::Alphanumeric, begin: 29, end: 30 },
                Segment { mode: Mode::Numeric, begin: 30, end: 32 },
                Segment { mode: Mode::Alphanumeric, begin: 32, end: 35 },
                Segment { mode: Mode::Numeric, begin: 35, end: 38 },
            ]
        );
    }

    #[test]
    fn test_parse_shift_jis_example_1() {
        let segs = parse(b"\x82\xa0\x81\x41\x41\xb1\x81\xf0"); // "あ、AｱÅ"
        assert_eq!(
            segs,
            vec![
                Segment { mode: Mode::Kanji, begin: 0, end: 4 },
                Segment { mode: Mode::Alphanumeric, begin: 4, end: 5 },
                Segment { mode: Mode::Byte, begin: 5, end: 6 },
                Segment { mode: Mode::Kanji, begin: 6, end: 8 },
            ]
        );
    }

    #[test]
    fn test_parse_utf_8() {
        // Mojibake?
        let segs = parse(b"\xe3\x81\x82\xe3\x80\x81A\xef\xbd\xb1\xe2\x84\xab");
        assert_eq!(
            segs,
            vec![
                Segment { mode: Mode::Kanji, begin: 0, end: 4 },
                Segment { mode: Mode::Byte, begin: 4, end: 5 },
                Segment { mode: Mode::Kanji, begin: 5, end: 7 },
                Segment { mode: Mode::Byte, begin: 7, end: 10 },
                Segment { mode: Mode::Kanji, begin: 10, end: 12 },
                Segment { mode: Mode::Byte, begin: 12, end: 13 },
            ]
        );
    }

    #[test]
    fn test_not_kanji_1() {
        let segs = parse(b"\x81\x30");
        assert_eq!(
            segs,
            vec![Segment { mode: Mode::Byte, begin: 0, end: 1 }, Segment { mode: Mode::Numeric, begin: 1, end: 2 }]
        );
    }

    #[test]
    fn test_not_kanji_2() {
        // Note that it's implementation detail that the byte seq is split into
        // two. Perhaps adjust the test to check for this.
        let segs = parse(b"\xeb\xc0");
        assert_eq!(
            segs,
            vec![Segment { mode: Mode::Byte, begin: 0, end: 1 }, Segment { mode: Mode::Byte, begin: 1, end: 2 }]
        );
    }

    #[test]
    fn test_not_kanji_3() {
        let segs = parse(b"\x81\x7f");
        assert_eq!(
            segs,
            vec![Segment { mode: Mode::Byte, begin: 0, end: 1 }, Segment { mode: Mode::Byte, begin: 1, end: 2 }]
        );
    }

    #[test]
    fn test_not_kanji_4() {
        let segs = parse(b"\x81\x40\x81");
        assert_eq!(
            segs,
            vec![Segment { mode: Mode::Kanji, begin: 0, end: 2 }, Segment { mode: Mode::Byte, begin: 2, end: 3 }]
        );
    }
}

//}}}
//------------------------------------------------------------------------------
//{{{ Optimizer

/// Iterator that merges consecutive parser segments to minimize the total
/// encoded length for a given [`Version`]. Created via [`Parser::optimize`].
pub struct Optimizer<I> {
    optimized: Vec<Segment>,
    index: usize,
    _source: PhantomData<I>,
}

impl<I: Iterator<Item = Segment>> Optimizer<I> {
    /// Optimize the segments by combining adjacent segments when beneficial.
    ///
    /// This uses dynamic programming over the parser's segment boundaries. It
    /// finds the minimum-size contiguous merge plan for those boundaries, but
    /// does not split a parser segment into smaller pieces.
    ///
    pub fn new(segments: I, version: Version) -> Self {
        let segments = segments.collect::<Vec<_>>();
        Self { optimized: optimize_segments(&segments, version), index: 0, _source: PhantomData }
    }
}

impl<'a> Parser<'a> {
    /// Turns this parser into an [`Optimizer`] for `version`, which yields
    /// optimally merged segments.
    pub fn optimize(self, version: Version) -> Optimizer<Parser<'a>> {
        Optimizer::new(self, version)
    }
}

impl<I: Iterator<Item = Segment>> Iterator for Optimizer<I> {
    type Item = Segment;

    fn next(&mut self) -> Option<Segment> {
        let segment = self.optimized.get(self.index).copied();
        if segment.is_some() {
            self.index += 1;
        }
        segment
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.optimized.len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<I: Iterator<Item = Segment>> ExactSizeIterator for Optimizer<I> {}
impl<I: Iterator<Item = Segment>> core::iter::FusedIterator for Optimizer<I> {}

/// Computes the total encoded length of all segments.
pub fn total_encoded_len(segments: &[Segment], version: Version) -> usize {
    segments.iter().map(|seg| seg.encoded_len(version)).sum()
}

/// Computes the minimum-size merge plan for parser segments.
///
/// Segment boundaries are preserved; adjacent segments may be merged into the
/// smallest common data mode that can encode the merged range.
///
/// Large inputs use an exact linear-time dynamic program with a fixed number of
/// candidate buckets. Small inputs and exceptional coordinate ranges use the
/// quadratic reference algorithm.
#[must_use]
pub fn optimize_segments(segments: &[Segment], version: Version) -> Vec<Segment> {
    if segments.len() <= 32 || !supports_linear_costs(segments, version) {
        optimize_segments_quadratic(segments, version)
    } else {
        optimize_segments_linear(segments, version)
    }
}

#[derive(Clone, Copy)]
struct ModeCost {
    mode: Mode,
    period: usize,
    bits_per_period: i128,
    bucket_offset: usize,
}

const MODE_COSTS: [ModeCost; 4] = [
    ModeCost { mode: Mode::Numeric, period: 3, bits_per_period: 10, bucket_offset: 0 },
    ModeCost { mode: Mode::Alphanumeric, period: 2, bits_per_period: 11, bucket_offset: 3 },
    ModeCost { mode: Mode::Byte, period: 1, bits_per_period: 8, bucket_offset: 5 },
    ModeCost { mode: Mode::Kanji, period: 2, bits_per_period: 13, bucket_offset: 6 },
];

fn mode_index(mode: Mode) -> usize {
    match mode {
        Mode::Numeric => 0,
        Mode::Alphanumeric => 1,
        Mode::Byte => 2,
        Mode::Kanji => 3,
    }
}

fn supports_linear_costs(segments: &[Segment], version: Version) -> bool {
    // Public segments may have arbitrary coordinates. Keep reference behavior
    // when suffix lengths or the original cost arithmetic could overflow.
    let mut min_begin = usize::MAX;
    let mut max_begin = 0;
    let mut max_end = 0;
    for segment in segments {
        min_begin = min_begin.min(segment.begin);
        max_begin = max_begin.max(segment.begin);
        max_end = max_end.max(segment.end);
        if segment.end < max_begin {
            return false;
        }
    }
    let max_header = MODE_COSTS
        .iter()
        .map(|cost| version.mode_bits_count() + cost.mode.length_bits_count(version))
        .max()
        .unwrap_or(0);
    (max_end - min_begin).checked_mul(13).and_then(|bits| bits.checked_add(max_header)).is_some()
}

fn prefer_start(
    candidate: usize,
    current: usize,
    cost: ModeCost,
    segments: &[Segment],
    best_bits: &[usize],
    best_count: &[usize],
) -> bool {
    if current == usize::MAX {
        return true;
    }
    // For equal begin residues, the endpoint and rounding terms are shared.
    // Signed wide keys also handle large absolute offsets without underflow.
    let key = |start: usize| {
        (
            best_bits[start] as i128 - (segments[start].begin / cost.period) as i128 * cost.bits_per_period,
            best_count[start],
            Reverse(start),
        )
    };
    key(candidate) < key(current)
}

fn optimize_segments_linear(segments: &[Segment], version: Version) -> Vec<Segment> {
    let len = segments.len();
    if len <= 1 {
        return segments.to_vec();
    }
    let mut best_bits = vec![usize::MAX; len + 1];
    let mut best_count = vec![usize::MAX; len + 1];
    let mut previous = vec![0_usize; len + 1];
    let mut previous_mode = vec![Mode::Byte; len + 1];
    best_bits[0] = 0;
    best_count[0] = 0;

    // Each suffix-mode group retains its best start under every future mode's
    // cost, since the best Numeric start need not stay best after a Byte join.
    let mut groups = [[usize::MAX; 8]; 4];
    for end in 1..=len {
        let incoming = segments[end - 1].mode;
        // The join is idempotent, so destination groups will not move again
        // when encountered later in this same pass.
        for (source_index, source_cost) in MODE_COSTS.iter().enumerate() {
            let destination = source_cost.mode.max(incoming);
            if destination == source_cost.mode {
                continue;
            }
            let migrating = core::mem::replace(&mut groups[source_index], [usize::MAX; 8]);
            let destination_group = &mut groups[mode_index(destination)];
            for cost in MODE_COSTS {
                for bucket in cost.bucket_offset..cost.bucket_offset + cost.period {
                    let start = migrating[bucket];
                    if start != usize::MAX
                        && prefer_start(start, destination_group[bucket], cost, segments, &best_bits, &best_count)
                    {
                        destination_group[bucket] = start;
                    }
                }
            }
        }

        let start = end - 1;
        if best_bits[start] != usize::MAX {
            for cost in MODE_COSTS {
                let bucket = cost.bucket_offset + segments[start].begin % cost.period;
                let group = &mut groups[mode_index(incoming)];
                if prefer_start(start, group[bucket], cost, segments, &best_bits, &best_count) {
                    group[bucket] = start;
                }
            }
        }

        for (group_index, cost) in MODE_COSTS.iter().enumerate() {
            for &start in &groups[group_index][cost.bucket_offset..cost.bucket_offset + cost.period] {
                if start == usize::MAX {
                    continue;
                }
                let merged = Segment { mode: cost.mode, begin: segments[start].begin, end: segments[end - 1].end };
                let Some(candidate_bits) = best_bits[start].checked_add(merged.encoded_len(version)) else {
                    continue;
                };
                let candidate_count = best_count[start] + 1;
                // An exact tie keeps the rightmost start, matching the
                // reference algorithm's backwards candidate scan.
                if (candidate_bits, candidate_count, Reverse(start))
                    < (best_bits[end], best_count[end], Reverse(previous[end]))
                {
                    best_bits[end] = candidate_bits;
                    best_count[end] = candidate_count;
                    previous[end] = start;
                    previous_mode[end] = cost.mode;
                }
            }
        }
    }

    let mut cursor = len;
    let mut optimized = Vec::with_capacity(best_count[len]);
    while cursor > 0 {
        let start = previous[cursor];
        optimized.push(Segment {
            mode: previous_mode[cursor],
            begin: segments[start].begin,
            end: segments[cursor - 1].end,
        });
        cursor = start;
    }
    optimized.reverse();
    optimized
}

fn optimize_segments_quadratic(segments: &[Segment], version: Version) -> Vec<Segment> {
    let len = segments.len();
    if len <= 1 {
        return segments.to_vec();
    }

    let mut best_bits = vec![usize::MAX; len + 1];
    let mut best_count = vec![usize::MAX; len + 1];
    let mut previous = vec![0_usize; len + 1];
    let mut previous_mode = vec![Mode::Byte; len + 1];
    best_bits[0] = 0;
    best_count[0] = 0;

    for end in 1..=len {
        let mut mode = segments[end - 1].mode;
        for start in (0..end).rev() {
            if start + 1 < end {
                mode = segments[start].mode.max(mode);
            }
            let merged = Segment { mode, begin: segments[start].begin, end: segments[end - 1].end };
            let Some(candidate_bits) = best_bits[start].checked_add(merged.encoded_len(version)) else {
                continue;
            };
            let candidate_count = best_count[start] + 1;
            if candidate_bits < best_bits[end] || candidate_bits == best_bits[end] && candidate_count < best_count[end]
            {
                best_bits[end] = candidate_bits;
                best_count[end] = candidate_count;
                previous[end] = start;
                previous_mode[end] = mode;
            }
        }
    }

    let mut cursor = len;
    let mut optimized = Vec::with_capacity(best_count[len]);
    while cursor > 0 {
        let start = previous[cursor];
        optimized.push(Segment {
            mode: previous_mode[cursor],
            begin: segments[start].begin,
            end: segments[cursor - 1].end,
        });
        cursor = start;
    }
    optimized.reverse();
    optimized
}

#[cfg(test)]
mod optimize_tests {
    use crate::optimize::{
        Optimizer, Parser, Segment, optimize_segments, optimize_segments_linear, optimize_segments_quadratic,
        supports_linear_costs, total_encoded_len,
    };
    use crate::types::{Mode, Version};

    fn test_optimization_result(given: &[Segment], expected: &[Segment], version: Version) {
        let prev_len = total_encoded_len(given, version);
        let opt_segs = Optimizer::new(given.iter().copied(), version).collect::<Vec<_>>();
        let new_len = total_encoded_len(&opt_segs, version);
        if given != opt_segs {
            assert!(prev_len > new_len, "{prev_len} > {new_len}");
        }
        assert_eq!(
            opt_segs,
            expected,
            "Optimization gave something better: {} < {} ({:?})",
            new_len,
            total_encoded_len(expected, version),
            opt_segs
        );
    }

    #[test]
    fn zero_or_one_segment_preserves_the_input_for_every_version_group() {
        let versions =
            [Version::Normal(1), Version::Normal(10), Version::Normal(27), Version::Micro(1), Version::Micro(4)];
        for version in versions {
            assert!(optimize_segments(&[], version).is_empty());
            for mode in [Mode::Numeric, Mode::Alphanumeric, Mode::Byte, Mode::Kanji] {
                let segment = Segment { mode, begin: 12, end: 30 };
                assert_eq!(optimize_segments(&[segment], version), vec![segment]);
            }
        }
    }

    #[test]
    fn test_example_1() {
        test_optimization_result(
            &[
                Segment { mode: Mode::Alphanumeric, begin: 0, end: 3 },
                Segment { mode: Mode::Numeric, begin: 3, end: 6 },
                Segment { mode: Mode::Byte, begin: 6, end: 10 },
            ],
            &[Segment { mode: Mode::Alphanumeric, begin: 0, end: 6 }, Segment { mode: Mode::Byte, begin: 6, end: 10 }],
            Version::Normal(1),
        );
    }

    #[test]
    fn test_example_2() {
        test_optimization_result(
            &[
                Segment { mode: Mode::Numeric, begin: 0, end: 29 },
                Segment { mode: Mode::Alphanumeric, begin: 29, end: 30 },
                Segment { mode: Mode::Numeric, begin: 30, end: 32 },
                Segment { mode: Mode::Alphanumeric, begin: 32, end: 35 },
                Segment { mode: Mode::Numeric, begin: 35, end: 38 },
            ],
            &[
                Segment { mode: Mode::Numeric, begin: 0, end: 29 },
                Segment { mode: Mode::Alphanumeric, begin: 29, end: 38 },
            ],
            Version::Normal(9),
        );
    }

    #[test]
    fn test_example_3() {
        test_optimization_result(
            &[
                Segment { mode: Mode::Kanji, begin: 0, end: 4 },
                Segment { mode: Mode::Alphanumeric, begin: 4, end: 5 },
                Segment { mode: Mode::Byte, begin: 5, end: 6 },
                Segment { mode: Mode::Kanji, begin: 6, end: 8 },
            ],
            &[Segment { mode: Mode::Byte, begin: 0, end: 8 }],
            Version::Normal(1),
        );
    }

    #[test]
    fn test_example_4() {
        test_optimization_result(
            &[Segment { mode: Mode::Kanji, begin: 0, end: 10 }, Segment { mode: Mode::Byte, begin: 10, end: 11 }],
            &[Segment { mode: Mode::Kanji, begin: 0, end: 10 }, Segment { mode: Mode::Byte, begin: 10, end: 11 }],
            Version::Normal(1),
        );
    }

    #[test]
    fn test_annex_j_guideline_1a() {
        test_optimization_result(
            &[
                Segment { mode: Mode::Numeric, begin: 0, end: 3 },
                Segment { mode: Mode::Alphanumeric, begin: 3, end: 4 },
            ],
            &[
                Segment { mode: Mode::Numeric, begin: 0, end: 3 },
                Segment { mode: Mode::Alphanumeric, begin: 3, end: 4 },
            ],
            Version::Micro(2),
        );
    }

    #[test]
    fn test_annex_j_guideline_1b() {
        test_optimization_result(
            &[
                Segment { mode: Mode::Numeric, begin: 0, end: 2 },
                Segment { mode: Mode::Alphanumeric, begin: 2, end: 4 },
            ],
            &[Segment { mode: Mode::Alphanumeric, begin: 0, end: 4 }],
            Version::Micro(2),
        );
    }

    #[test]
    fn test_annex_j_guideline_1c() {
        test_optimization_result(
            &[
                Segment { mode: Mode::Numeric, begin: 0, end: 3 },
                Segment { mode: Mode::Alphanumeric, begin: 3, end: 4 },
            ],
            &[Segment { mode: Mode::Alphanumeric, begin: 0, end: 4 }],
            Version::Micro(3),
        );
    }

    #[test]
    fn dynamic_programming_can_skip_a_local_merge_for_a_better_total() {
        let given = [
            Segment { mode: Mode::Numeric, begin: 0, end: 7 },
            Segment { mode: Mode::Alphanumeric, begin: 7, end: 8 },
            Segment { mode: Mode::Numeric, begin: 8, end: 9 },
        ];

        let optimized = optimize_segments(&given, Version::Normal(1));

        assert_eq!(
            optimized,
            vec![
                Segment { mode: Mode::Numeric, begin: 0, end: 7 },
                Segment { mode: Mode::Alphanumeric, begin: 7, end: 9 },
            ]
        );
        assert!(
            total_encoded_len(&optimized, Version::Normal(1))
                < total_encoded_len(&[Segment { mode: Mode::Alphanumeric, begin: 0, end: 9 }], Version::Normal(1))
        );
    }

    const COST_VERSIONS: [Version; 7] = [
        Version::Normal(1),
        Version::Normal(10),
        Version::Normal(27),
        Version::Micro(1),
        Version::Micro(2),
        Version::Micro(3),
        Version::Micro(4),
    ];
    const MODES: [Mode; 4] = [Mode::Numeric, Mode::Alphanumeric, Mode::Byte, Mode::Kanji];

    fn assert_matches_quadratic(segments: &[Segment], version: Version) {
        let expected = optimize_segments_quadratic(segments, version);
        assert_eq!(
            optimize_segments_linear(segments, version),
            expected,
            "linear plan: version {version:?}, segments {segments:?}"
        );
        assert_eq!(
            optimize_segments(segments, version),
            expected,
            "hybrid plan: version {version:?}, segments {segments:?}"
        );
    }

    #[test]
    fn linear_plan_matches_quadratic_for_exhaustive_modes_and_length_residues() {
        for len in 0..=6 {
            for mut encoded_modes in 0..4_usize.pow(len) {
                let modes = (0..len)
                    .map(|_| {
                        let mode = MODES[encoded_modes % 4];
                        encoded_modes /= 4;
                        mode
                    })
                    .collect::<Vec<_>>();
                for length_pattern in 0..6 {
                    let mut begin = 17;
                    let segments = modes
                        .iter()
                        .enumerate()
                        .map(|(index, &mode)| {
                            let length = match length_pattern {
                                0..=3 => length_pattern,
                                4 => 7,
                                _ => index % 6 + 1,
                            };
                            let segment = Segment { mode, begin, end: begin + length };
                            begin = segment.end;
                            segment
                        })
                        .collect::<Vec<_>>();
                    for version in COST_VERSIONS {
                        assert_matches_quadratic(&segments, version);
                    }
                }
            }
        }
    }

    fn next_random(seed: &mut u64) -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    }

    #[test]
    fn linear_plan_matches_quadratic_for_long_random_sequences_and_shifted_offsets() {
        let versions = [
            Version::Normal(1),
            Version::Normal(9),
            Version::Normal(10),
            Version::Normal(26),
            Version::Normal(27),
            Version::Normal(40),
            Version::Micro(1),
            Version::Micro(2),
            Version::Micro(3),
            Version::Micro(4),
        ];
        let mut seed = 2_712_u64;
        for case in 0..512 {
            let len = next_random(&mut seed) as usize % 224 + 33;
            let mut begin = if case % 8 == 0 { usize::MAX - 65_536 } else { next_random(&mut seed) as usize % 1024 };
            let segments = (0..len)
                .map(|_| {
                    let mode = MODES[next_random(&mut seed) as usize % 4];
                    let length = next_random(&mut seed) as usize % 71;
                    let gap = next_random(&mut seed) as usize % 5;
                    let segment = Segment { mode, begin, end: begin + length };
                    begin = segment.end + gap;
                    segment
                })
                .collect::<Vec<_>>();
            assert!(supports_linear_costs(&segments, versions[case % versions.len()]));
            assert_matches_quadratic(&segments, versions[case % versions.len()]);
        }
    }

    #[test]
    fn linear_plan_matches_quadratic_at_the_hybrid_boundary() {
        for len in [31, 32, 33, 64, 256, 1024] {
            let data = (0..len).map(|index| if index % 2 == 0 { b'A' } else { b'1' }).collect::<Vec<_>>();
            let segments = Parser::new(&data).collect::<Vec<_>>();
            for version in COST_VERSIONS {
                assert_matches_quadratic(&segments, version);
            }
        }
    }

    #[test]
    fn linear_plan_keeps_the_rightmost_start_when_bits_and_segment_count_tie() {
        let given = [
            Segment { mode: Mode::Numeric, begin: 0, end: 7 },
            Segment { mode: Mode::Alphanumeric, begin: 7, end: 8 },
            Segment { mode: Mode::Numeric, begin: 8, end: 15 },
        ];
        let expected = vec![
            Segment { mode: Mode::Alphanumeric, begin: 0, end: 8 },
            Segment { mode: Mode::Numeric, begin: 8, end: 15 },
        ];
        let optimized = optimize_segments_linear(&given, Version::Normal(1));
        assert_eq!(total_encoded_len(&optimized, Version::Normal(1)), 95);
        assert_eq!(optimized, expected);
        assert_matches_quadratic(&given, Version::Normal(1));
    }

    #[test]
    fn exceptional_coordinates_preserve_the_quadratic_fallback() {
        for segments in [
            vec![Segment { mode: Mode::Byte, begin: 2, end: 1 }; 33],
            (0..33)
                .map(|index| {
                    let begin = if index == 0 { 0 } else { usize::MAX / 4 };
                    Segment { mode: Mode::Byte, begin, end: begin }
                })
                .collect::<Vec<_>>(),
        ] {
            assert!(!supports_linear_costs(&segments, Version::Normal(1)));
            let actual = std::panic::catch_unwind(|| optimize_segments(&segments, Version::Normal(1)));
            let expected = std::panic::catch_unwind(|| optimize_segments_quadratic(&segments, Version::Normal(1)));
            match (actual, expected) {
                (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
                (Err(_), Err(_)) => {}
                _ => panic!("the hybrid path changed the exceptional-coordinate behavior"),
            }
        }
    }
}

//}}}
//------------------------------------------------------------------------------
//{{{ Internal types and data for parsing

/// All values of `u8` can be split into 9 different character sets when
/// determining which encoding to use. This enum represents these groupings for
/// parsing purpose.
#[derive(Copy, Clone)]
enum ExclCharSet {
    /// The end of string.
    End = 0,

    /// All symbols supported by the Alphanumeric encoding, i.e. space, `$`, `%`,
    /// `*`, `+`, `-`, `.`, `/` and `:`.
    Symbol = 1,

    /// All numbers (0–9).
    Numeric = 2,

    /// All uppercase letters (A–Z). These characters may also appear in the
    /// second byte of a Shift JIS 2-byte encoding.
    Alpha = 3,

    /// The first byte of a Shift JIS 2-byte encoding, in the range 0x81–0x9f.
    KanjiHi1 = 4,

    /// The first byte of a Shift JIS 2-byte encoding, in the range 0xe0–0xea.
    KanjiHi2 = 5,

    /// The first byte of a Shift JIS 2-byte encoding, of value 0xeb. This is
    /// different from the other two range that the second byte has a smaller
    /// range.
    KanjiHi3 = 6,

    /// The second byte of a Shift JIS 2-byte encoding, in the range 0x40–0xbf,
    /// excluding letters (covered by `Alpha`), 0x81–0x9f (covered by `KanjiHi1`),
    /// and the invalid byte 0x7f.
    KanjiLo1 = 7,

    /// The second byte of a Shift JIS 2-byte encoding, in the range 0xc0–0xfc,
    /// excluding the range 0xe0–0xeb (covered by `KanjiHi2` and `KanjiHi3`).
    /// This half of byte-pair cannot appear as the second byte leaded by
    /// `KanjiHi3`.
    KanjiLo2 = 8,

    /// Any other values not covered by the above character sets.
    Byte = 9,
}

impl ExclCharSet {
    /// Determines which character set a byte is in.
    fn from_u8(c: u8) -> Self {
        match c {
            0x20 | 0x24 | 0x25 | 0x2a | 0x2b | 0x2d..=0x2f | 0x3a => ExclCharSet::Symbol,
            0x30..=0x39 => ExclCharSet::Numeric,
            0x41..=0x5a => ExclCharSet::Alpha,
            0x81..=0x9f => ExclCharSet::KanjiHi1,
            0xe0..=0xea => ExclCharSet::KanjiHi2,
            0xeb => ExclCharSet::KanjiHi3,
            0x40 | 0x5b..=0x7e | 0x80 | 0xa0..=0xbf => ExclCharSet::KanjiLo1,
            0xc0..=0xdf | 0xec..=0xfc => ExclCharSet::KanjiLo2,
            _ => ExclCharSet::Byte,
        }
    }
}

/// The current parsing state.
#[derive(Copy, Clone)]
enum State {
    /// Just initialized.
    Init = 0,

    /// Inside a string that can be exclusively encoded as Numeric.
    Numeric = 10,

    /// Inside a string that can be exclusively encoded as Alphanumeric.
    Alpha = 20,

    /// Inside a string that can be exclusively encoded as 8-Bit Byte.
    Byte = 30,

    /// Just encountered the first byte of a Shift JIS 2-byte sequence of the
    /// set `KanjiHi1` or `KanjiHi2`.
    KanjiHi12 = 40,

    /// Just encountered the first byte of a Shift JIS 2-byte sequence of the
    /// set `KanjiHi3`.
    KanjiHi3 = 50,

    /// Inside a string that can be exclusively encoded as Kanji.
    Kanji = 60,
}

/// What should the parser do after a state transition.
#[derive(Copy, Clone)]
enum Action {
    /// The parser should do nothing.
    Idle,

    /// Push the current segment as a Numeric string, and reset the marks.
    Numeric,

    /// Push the current segment as an Alphanumeric string, and reset the marks.
    Alpha,

    /// Push the current segment as a 8-Bit Byte string, and reset the marks.
    Byte,

    /// Push the current segment as a Kanji string, and reset the marks.
    Kanji,

    /// Push the current segment excluding the last byte as a Kanji string, then
    /// push the remaining single byte as a Byte string, and reset the marks.
    KanjiAndSingleByte,
}

static STATE_TRANSITION: [(State, Action); 70] = [
    // STATE_TRANSITION[current_state + next_character] == (next_state, what_to_do)

    // Init state:
    (State::Init, Action::Idle),      // End
    (State::Alpha, Action::Idle),     // Symbol
    (State::Numeric, Action::Idle),   // Numeric
    (State::Alpha, Action::Idle),     // Alpha
    (State::KanjiHi12, Action::Idle), // KanjiHi1
    (State::KanjiHi12, Action::Idle), // KanjiHi2
    (State::KanjiHi3, Action::Idle),  // KanjiHi3
    (State::Byte, Action::Idle),      // KanjiLo1
    (State::Byte, Action::Idle),      // KanjiLo2
    (State::Byte, Action::Idle),      // Byte
    // Numeric state:
    (State::Init, Action::Numeric),      // End
    (State::Alpha, Action::Numeric),     // Symbol
    (State::Numeric, Action::Idle),      // Numeric
    (State::Alpha, Action::Numeric),     // Alpha
    (State::KanjiHi12, Action::Numeric), // KanjiHi1
    (State::KanjiHi12, Action::Numeric), // KanjiHi2
    (State::KanjiHi3, Action::Numeric),  // KanjiHi3
    (State::Byte, Action::Numeric),      // KanjiLo1
    (State::Byte, Action::Numeric),      // KanjiLo2
    (State::Byte, Action::Numeric),      // Byte
    // Alpha state:
    (State::Init, Action::Alpha),      // End
    (State::Alpha, Action::Idle),      // Symbol
    (State::Numeric, Action::Alpha),   // Numeric
    (State::Alpha, Action::Idle),      // Alpha
    (State::KanjiHi12, Action::Alpha), // KanjiHi1
    (State::KanjiHi12, Action::Alpha), // KanjiHi2
    (State::KanjiHi3, Action::Alpha),  // KanjiHi3
    (State::Byte, Action::Alpha),      // KanjiLo1
    (State::Byte, Action::Alpha),      // KanjiLo2
    (State::Byte, Action::Alpha),      // Byte
    // Byte state:
    (State::Init, Action::Byte),      // End
    (State::Alpha, Action::Byte),     // Symbol
    (State::Numeric, Action::Byte),   // Numeric
    (State::Alpha, Action::Byte),     // Alpha
    (State::KanjiHi12, Action::Byte), // KanjiHi1
    (State::KanjiHi12, Action::Byte), // KanjiHi2
    (State::KanjiHi3, Action::Byte),  // KanjiHi3
    (State::Byte, Action::Idle),      // KanjiLo1
    (State::Byte, Action::Idle),      // KanjiLo2
    (State::Byte, Action::Idle),      // Byte
    // KanjiHi12 state:
    (State::Init, Action::KanjiAndSingleByte),    // End
    (State::Alpha, Action::KanjiAndSingleByte),   // Symbol
    (State::Numeric, Action::KanjiAndSingleByte), // Numeric
    (State::Kanji, Action::Idle),                 // Alpha
    (State::Kanji, Action::Idle),                 // KanjiHi1
    (State::Kanji, Action::Idle),                 // KanjiHi2
    (State::Kanji, Action::Idle),                 // KanjiHi3
    (State::Kanji, Action::Idle),                 // KanjiLo1
    (State::Kanji, Action::Idle),                 // KanjiLo2
    (State::Byte, Action::KanjiAndSingleByte),    // Byte
    // KanjiHi3 state:
    (State::Init, Action::KanjiAndSingleByte),      // End
    (State::Alpha, Action::KanjiAndSingleByte),     // Symbol
    (State::Numeric, Action::KanjiAndSingleByte),   // Numeric
    (State::Kanji, Action::Idle),                   // Alpha
    (State::Kanji, Action::Idle),                   // KanjiHi1
    (State::KanjiHi12, Action::KanjiAndSingleByte), // KanjiHi2
    (State::KanjiHi3, Action::KanjiAndSingleByte),  // KanjiHi3
    (State::Kanji, Action::Idle),                   // KanjiLo1
    (State::Byte, Action::KanjiAndSingleByte),      // KanjiLo2
    (State::Byte, Action::KanjiAndSingleByte),      // Byte
    // Kanji state:
    (State::Init, Action::Kanji),     // End
    (State::Alpha, Action::Kanji),    // Symbol
    (State::Numeric, Action::Kanji),  // Numeric
    (State::Alpha, Action::Kanji),    // Alpha
    (State::KanjiHi12, Action::Idle), // KanjiHi1
    (State::KanjiHi12, Action::Idle), // KanjiHi2
    (State::KanjiHi3, Action::Idle),  // KanjiHi3
    (State::Byte, Action::Kanji),     // KanjiLo1
    (State::Byte, Action::Kanji),     // KanjiLo2
    (State::Byte, Action::Kanji),     // Byte
];

//}}}
