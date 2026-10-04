# Decoder dependency review for 2.2.0

The optional `decode-rxing` feature and CLI select `rxing = 0.9.3` with
`qrcode`, `decoders`, and `no_character_set_support`. Default features,
client result parsers, image helpers, encoders and other barcode formats are
disabled. Feature selection is distinct from a whole-crate deployment audit.

## Source identity and criteria

All 24 locked versions were extracted from published crate archives whose
SHA-256 checksums match `Cargo.lock`. Source identity is separate from security
evidence. Review traced unsafe implementations, safe entry contracts, native
and compiler capabilities, parser resource controls and platform boundaries.

The records use Cargo-vet's built-in `safe-to-deploy` criterion: reason about
unsafe code and powerful imports under reasonable deployment usage. They are
AI-assisted source reviews, not independent human certification, exhaustive
logic proofs, fuzz campaigns or platform runtime tests. Transitive dependencies
continue to require their own policy evidence.

## Recorded source audits

| Package | Version | Main reviewed boundary |
| --- | --- | --- |
| aho-corasick | 1.1.5 | SIMD dispatch, pointer spans, buckets and state IDs |
| android_system_properties | 0.1.6 | Native callback, property buffer and libc handle ownership |
| chrono | 0.4.45 | Unsafe date representations, timezone configuration and native clocks |
| futures-core | 0.3.34 | Atomic waker ownership and pinned stream projection |
| futures-task | 0.3.34 | Arc wakers, erased future lifetimes, pin and Send contracts |
| futures-util | 0.3.34 | Task queues, polling, locks, pinning, IO initialization and panic cleanup |
| iana-time-zone-haiku | 0.1.2 | Bounded native copy, exception boundary and compiler inputs |
| num | 0.4.3 | No-std re-export facade and package capabilities |
| num-iter | 0.1.46 | Safe iterator arithmetic and caller termination obligations |
| regex | 1.13.1 | Own-crate unsafe Searcher, UTF-8 endpoints and builder limits |
| regex-syntax | 0.8.11 | Iterative parsers/visitors/destructors, limits and safe table consumers |
| slab | 0.4.12 | Disjoint keys, storage bounds, initialization, cleanup and unchecked contracts |
| windows-link | 0.2.1 | Explicit target-specific native declaration macros |
| windows-result | 0.4.1 | Native error, COM/string ownership and initialized outputs |

Detailed evidence and assumptions are in `supply-chain/audits.toml`. Clock
providers and timezone configuration are trusted host inputs; sparse Slab
keys and numeric iterator termination need caller bounds; regex limits are
not whole-request memory limits. Arbitrary system files, native providers
and caller-defined unsafe implementations are outside those assumptions.

Four superseded development exemptions (aho-corasick, regex, regex-syntax,
windows-link) were removed. Existing cc/Tokio delta audits and the libc pin
remain intact. No new exemption, publisher trust, peer import or weaker
criterion was added.

## Remaining release gate

| Package | Version | Required follow-up |
| --- | --- | --- |
| core-foundation-sys | 0.8.7 | Broader native ABI and callback contract reconciliation |
| iana-time-zone | 0.1.65 | Emscripten result storage and concurrency contract |
| js-sys | 0.3.106 | Generic representation, shared-buffer copying and generated bindings |
| regex-automata | 0.4.18 | Thread identifier exhaustion and pool ownership invariant |
| rxing | 0.9.3 | Discovery bounds and remaining public/optional parser contracts |
| unicode-segmentation | 1.13.3 | Sentence/reverse iterator contracts beyond selected use |
| windows-core | 0.62.2 | Broad public interface ownership and lifetime contracts |
| windows-implement | 0.60.2 | Generated owned interface lifetime contracts |
| windows-interface | 0.59.3 | Scoped versus owned generated interface contracts |
| windows-strings | 0.5.1 | Allocation/representation bounds on all supported targets |

These versions have no new deployment audit or exemption. Investigating a
contract, passing integration tests or excluding one feature cannot justify
an unrestricted certificate for the original published version. Candidate
security details are retained privately for validation and coordination.

The proposed 24-exemption draft was not adopted. After the 14 source audits,
`cargo vet --locked` still fails for the ten entries above. Publication remains
blocked until adequate evidence or a reviewed dependency-graph change closes
those obligations. This document does not claim 2.2.0 publication.
