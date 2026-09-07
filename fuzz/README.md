# qrcode-rs fuzz targets

This directory contains the `cargo-fuzz` targets used to exercise the public
encoding, rendering, parsing, and Structured Append boundaries.

List targets and run a short local smoke with the nightly toolchain:

```sh
cargo +nightly fuzz list
cargo +nightly fuzz run encode -- -runs=64 -seed=20260907 -timeout=5
cargo +nightly fuzz run render_image -- -runs=64 -seed=20260907 -timeout=5
```

CI runs the same bounded smoke on pushes, pull requests, and a scheduled
workflow. The fixed seed makes failures reproducible; the run count and
timeout are deliberately short so the job is suitable as a gate.

The smoke job is not evidence of a 72-hour campaign, sanitizer coverage, or
OSS-Fuzz acceptance. Those longer ASAN/UBSAN and OSS-Fuzz campaigns remain a
follow-up task; any discovered reproducer should be checked in as a corpus
input or regression test before changing that status.
