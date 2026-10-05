# Decoder dependency review for 2.2.0

## Runtime boundary

The historical `decode-rxing` / `rxing` feature names now select a maintained
private QR engine derived from the checksum-verified `rxing 0.9.3` source.
It supports bounded Normal Model 2/Micro scanning and raw Structured Append
recovery. This is an explicit source and dependency-graph change, not a
certificate for the unrestricted original package.

The scanner continues to expose raw bytes and independent candidate outcomes.
It performs one binarization/discovery/sampling pipeline, with explicit
inversion and no backend retry chain. The new 2.2 backend error type is
`qrcode_decode::rxing::BackendError`; it has project-owned Rust identity.
Existing published rqrr APIs, no_std default behavior and text/JSON/raw output
contracts remain separate and unchanged.

## Source review and provenance

The upstream archive checksum and each retained original/derived source hash
are recorded in `crates/qrcode-decode/src/engine/source-provenance.json`.
Original copyright notices, a modified-file notice, the Apache license,
NOTICE and ORIGIN documentation are bundled in the actual crate archive.
Project-owned source outside that directory keeps its dual-license terms;
the SPDX package expression explicitly includes the Apache port obligation.

Every retained source file and selected entry/helper was reviewed, including
version/EC tables, RS bounds, floating-point geometry, byte/ECI handling,
cache ownership and the public input/error/publication boundaries. Unused
native clocks/platform bindings, other barcode formats, client parsers,
text converters and general unsafe-contract adapters are excluded.
This finite AI-assisted review and the focused checks are not a formal proof
or an independent human security certification.

## Changes at the checked boundary

- Accepted finder count is checked during discovery. Rejected horizontal
  ratio matches advance a monotone pixel prefix instead of rescanning each
  preceding run. Finder grouping visits only bins actually in its grid.
- Coordinates are checked before offset arithmetic/pixel access. Sampling
  rejects nonfinite transforms, degenerate geometry and invalid ROI controls.
  QR estimates retain legitimate BCH correction while actual sampling stays
  within supported version dimensions.
- FNC1 percent escaping uses linear in-place compaction. Raw payload ownership
  transfers only after status, geometry, version, EC and SA validation.
- Unused row/column caches, luma mutations, text conversion, legacy adapters
  and result-point allocations are removed. Private visibility alone was
  never used as an audit substitute.

`max_results` bounds retained sampled-candidate outcomes; it does not promise
that all rejected geometric proposals consume that counter. Grouping/sampling
work is separately finite under pixel/finder limits and capped neighborhoods.
These settings are not a process-RSS limit or wall-clock deadline.

## Cargo-vet policy

The 14 previously recorded published-source audits remain historical evidence
for their exact versions. Their archive identity was checked against baseline
commit `62b14bc` before this graph reduction. No audit is fabricated for the
ten unresolved original deployment contracts.

Nine original gap packages leave the graph with the unused runtime closure.
`regex-automata` remains through the development-only criterion dependency;
its original safe-to-run baseline applies again without a new exemption or
weaker rule. Existing criteria, imports, trust, exceptions and cc/Tokio records
remain unchanged. A passing locked check therefore means the reviewed graph
satisfies that original policy, not that every dependency has a full audit.

Publication requires successful final source review, focused/workspace checks,
finite scanner fuzzing, actual package reconstruction and exact-commit CI.
Neither this document nor a prepared changelog claims publication.
