# qrcode-cli

`qrcode-cli` packages the `qrencodes` command-line QR generator from
[`qrcode-rs`](https://crates.io/crates/qrcode-rs) as a split workspace
package used by release assets and CLI-focused builds. It supports string,
Unicode, ANSI, SVG, PNG, EPS, PIC, HTML, and PDF output, including stdin and
batch input.

Install it with:

```bash
cargo install qrcode-rs --features cli --bin qrencodes
```

Examples:

```bash
qrencodes "https://example.com"
qrencodes -f png -o out.png --size 12 "Hello"
printf 'piped input' | qrencodes -f unicode
qrencodes --batch ./payloads.txt -f svg -o ./out
qrencodes --batch ./payloads.csv --batch-format csv --batch-column 2 --parallel -f png -o ./out
qrencodes --batch ./payloads.json --batch-format json --batch-key payload -f svg -o ./out
qrencodes --batch ./payloads.jsonl --batch-format jsonl --batch-key payload -f svg -o ./out
qrencodes --batch ./payloads.txt --batch-pack zip -f svg -o payloads.zip
qrencodes --batch ./payloads.txt --batch-pack grid --grid-columns 3 -f png -o payloads.png
qrencodes validate out.png --expect "Hello"
qrencodes validate out.png --print-payload
qrencodes decode out.png --format json
qrencodes decode out.png --format raw --output payload.bin
qrencodes decode first.png second.png --assemble
qrencodes decode inverted.png --invert
# Read newline-delimited payloads from stdin and write one file per payload.
printf 'first\nsecond\n' | qrencodes --batch - -f svg -o ./out
```

The standalone binary and the facade's feature-gated binary call the same
implementation through the facade's `cli` feature. Both entry points retain
the same arguments, output and exit codes.

`validate` and `decode` support Normal, Micro and Structured Append QR images.
`validate` keeps its summary and lossy UTF-8 display. `decode` accepts multiple
image files and supports strict UTF-8 text, compact JSON and original raw bytes.
JSON contains `symbols`, `errors` and `assemblies`; each symbol retains its
bytes, version, EC level, source and optional Structured Append header. Raw
output requires exactly one logical payload.

Use `--assemble` to recover complete Structured Append groups. Missing or
duplicate fragments fail before publishing output; fragments are never
silently removed. Parity can collide between messages, so select the intended
files explicitly. `--allow-partial` reports failed candidates and outputs valid
symbols, but does not skip unreadable files or invalid assemblies. Use
`--invert` for opposite black/white polarity.

Encoded files are limited to 64 MiB, sides to 32,768 pixels, and default input
pixels to 16,777,216 (`decode --max-pixels` can adjust the pixel budget).
Decoded plus grayscale buffers have a nominal 256 MiB limit. Across multiple
images, at most 4,096 symbols/failures and 256 MiB of payload are retained;
serialized output also has a 256 MiB limit. These are resource checks, not a
total RSS or runtime guarantee. Inputs, assemblies and output formats are
validated before stdout or atomic file publication.

Batch input ignores blank or whitespace-only payloads while preserving the exact
contents of each non-empty payload. Use `--batch -` for a pipe. CSV input uses a 1-based
`--batch-column`, JSON/JSONL input reads strings or a string field named by
`--batch-key`. Directory output preserves file order; its parallel path collects
the batch before writing. ZIP output consumes records incrementally, including
JSON arrays, and its parallel path retains at most 64 input records and their
rendered results at a time. ZIP central-directory metadata and the largest
individual input record still contribute to memory use.

Use `--batch-pack zip` to write one uncompressed archive, or `--batch-pack grid`
with PNG output to draw compact QR symbols directly into one contact sheet.
The direct grid canvas uses the 256 MiB rendering budget.

Regular file outputs are replaced only after a temporary output has been
successfully written and flushed. Existing permissions are preserved. Read-only
files, output symlinks, and special file destinations are rejected; stdout and
`--output -` retain their normal behavior.
