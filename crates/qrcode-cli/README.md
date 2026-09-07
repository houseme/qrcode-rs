# qrcode-cli

`qrcode-cli` packages the `qrencodes` command-line QR generator from
[`qrcode-rs`](https://crates.io/crates/qrcode-rs) as an independently
installable binary. It supports string, Unicode, ANSI, SVG, PNG, EPS, PIC,
HTML, and PDF output, including stdin and batch input.

Install it with:

```bash
cargo install qrcode-cli
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
backward compatibility. New installations can use this package directly.

Batch input ignores blank or whitespace-only payloads while preserving the exact
contents of each non-empty payload. Use `--batch -` for a pipe; batch output is
always written to the directory given by `--output`. CSV input uses a 1-based
`--batch-column`, JSON/JSONL input reads strings or a string field named by
`--batch-key`, and `--parallel` renders a collected batch concurrently while
preserving file order. Use `--batch-pack zip` to write the generated batch files
into a single uncompressed ZIP archive without adding packaging dependencies, or
`--batch-pack grid` with PNG output to build a single contact sheet.
