# Private QR engine

This private module derives from the published `rxing 0.9.3` archive, checksum
`842b2a567172af73f7d18ee076f7bbb4ed8545c933b11f43868f0b1bba203157`.
The baseline dependency identity was verified against commit `62b14bc` Cargo.lock.
Original file names/copyright notices are retained. `source-provenance.json`
records original and final file hashes, removed files and generated glue.

The historical `rxing`/`decode-rxing` feature names remain for the new 2.2 API.
The implementation is project maintained; it no longer links the external
rxing package. Its supported image boundary is bounded Normal Model 2/Micro
scanning and raw payload/Structured Append recovery. Unsupported Model 1 and
rMQR parsing is rejected at the production entrypoint; retained shared data
must not be exposed as a claim of support for those formats.

Only `BackendError` is publicly re-exported from this module. The 2.2 scanner
has not previously been released. Its error type has a project-owned Rust
identity instead of `rxing::Exceptions`; existing rqrr APIs are unchanged.
Raw ECI bytes remain uninterpreted and text conversion is outside this engine.

Source review covers all retained files and actual callers. Private visibility,
source relocation and passing Cargo-vet do not independently establish safety.
New entrypoints or future use of retained shared helpers require review again.
The review is finite source/contract validation, with focused regression and
fuzz checks; it is not a formal proof or independent human certification.

The bundled Apache license/NOTICE apply to this directory. Root project source
outside this directory keeps its own dual-license terms. A current packaged
artifact must include source, licensing and this provenance before release.
