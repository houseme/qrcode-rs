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
# Read newline-delimited payloads from stdin and write one file per payload.
printf 'first\nsecond\n' | qrencodes --batch - -f svg -o ./out
```

The `qrcode-rs` facade keeps its feature-gated `qrencodes` binary for
backward compatibility and for crates.io installations.

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
