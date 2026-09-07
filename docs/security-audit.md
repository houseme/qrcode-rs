# Security audit summary

This document is the public summary of the repository's security review. It is
deliberately limited to verified behavior and resolved hardening work; it is
not a replacement for an independent security audit or a statement that every
future dependency and configuration is risk-free.

## Review status

The current repository is version 2.0.0 and contains the v2.1 security-plan
hardening work that is available in the source tree. The v2.1 release gates
that require long-running fuzzing, sanitizers, Miri, supply-chain attestations,
or an external audit are not evidenced by this summary and remain follow-up
work until their results are published.

No unresolved vulnerability is intentionally described here. Suspected new
issues should be reported privately according to [SECURITY.md](../SECURITY.md).

## v2.0 review and hardening

The v2.0 workspace split was reviewed along the public encoding, parsing,
rendering, decoding, plugin, image, and CLI boundaries. The following controls
are present in the current source tree:

- Core module views, references, and plugin grids validate dimensions before
  indexing or allocation. Multiplication and addition paths use checked
  arithmetic where malformed dimensions could otherwise wrap.
- Renderers validate module-source lengths and return a render error for an
  invalid source. Plain-text output checks quiet-zone, border, and output-size
  arithmetic.
- Image rendering validates module dimensions and rejects a zero module size
  through the high-level API instead of silently producing an invalid image.
- Input and fixed-version encoding return typed errors for unsupported version,
  error-correction, mode, ECI, and capacity combinations. The encoder does not
  rely on caller-provided dimensions for the QR matrix.
- The accelerated mask-scoring paths in `qrcode-core` are guarded by the
  corresponding CPU feature checks where runtime detection is available. Their
  vector loads are bounded by loop guards and have scalar fallbacks. This is a
  source-level safety review, not a formal proof of the platform intrinsics.
- The default library remains usable without optional rendering and decoding
  dependencies; optional capabilities are feature-gated to keep the dependency
  surface explicit.

## Verification evidence

The repository contains and CI exercises the following finite checks:

- unit, integration, property, and differential tests for core encoding,
  fixed versions, error-correction levels, forced modes, renderers, parsers,
  plugins, and boundary conditions;
- seven `cargo-fuzz` targets covering core encoding, fixed-version encoding,
  SVG and image rendering, structured payload parsing, Structured Append, and
  decoder-side Structured Append parsing;
- a fuzz-target compile check and short smoke runs in the Build workflow; and
- scheduled RustSec, cargo-deny, cargo-vet, and bounded fuzz workflows for
  dependency and input-boundary checks.

These checks reduce regression risk but do not establish a clean result for
all future runs. In particular, the repository does not claim that a short fuzz
smoke run is equivalent to a 72-hour campaign.

## v2.1 review snapshot

The v2.1 hardening review added coverage for arbitrary byte inputs, automatic
version selection, fixed versions, error-correction levels, and supported
forced encoding modes. The current property and differential tests complete
without a panic for their bounded test cases, and all seven fuzz targets remain
available for longer campaigns.

The review also records two classes of hardening observation, both addressed in
the current source tree:

1. Geometry supplied across the core, plugin, and renderer boundaries could
   otherwise make multiplication, addition, indexing, or allocation unsafe.
   Checked validation and typed errors now reject invalid or overflowing
   dimensions before those operations.
2. A zero image module size was previously normalized by the low-level image
   path. The high-level `qrcode-image` API now validates this configuration and
   returns an explicit invalid-dimensions error.

These are engineering observations and remediation records, not CVE
assignments. The review did not run a long-lived fuzz campaign, sanitizer
campaign, or formal verification pass; see the follow-up table below.

## v2.1 follow-up boundary

The following items are planned or require separate evidence before they can
be called complete:

| Control | Current public evidence | Required completion evidence |
| --- | --- | --- |
| Long-running fuzzing / OSS-Fuzz | Local fuzz targets and short CI smoke runs | Published campaign or OSS-Fuzz integration results |
| Miri and ASAN/UBSAN | Not part of the current repository CI evidence | Reproducible sanitizer and Miri runs for supported targets |
| cargo-deny / cargo-vet | Local deny licenses/bans/sources and `cargo vet check` pass with the locked baseline; fresh advisory data remains CI-scoped | Green CI results with current advisory data and audit imports |
| SBOM and release signing | Release workflow and verification instructions are present but no release artifact was produced in this review | Release-attached SBOM and verifiable signing/attestation evidence |
| External security audit | Not performed as part of this work | Public scope, report, and remediation record |

Until those gates are completed, users should treat the existing tests and
scheduled advisory scan as defense-in-depth rather than a security
certification.

## Audit maintenance

This summary should be updated when a release changes a security boundary, a
reported vulnerability is fixed and publicly disclosed, or one of the deferred
verification gates produces reproducible evidence. Do not add private
environment details, credentials, exploit payloads, or unresolved vulnerability
details to this public document.
