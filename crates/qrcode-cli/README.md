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
# Read newline-delimited payloads from stdin and write one file per payload.
printf 'first\nsecond\n' | qrencodes --batch - -f svg -o ./out
```

The `qrcode-rs` facade keeps its feature-gated `qrencodes` binary for
backward compatibility. New installations can use this package directly.

Batch input ignores blank or whitespace-only lines while preserving the exact
contents of each non-empty line. Use `--batch -` for a pipe; batch output is
always written to the directory given by `--output`.
