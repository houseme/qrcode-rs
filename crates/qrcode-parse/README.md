# qrcode-parse

`qrcode-parse` provides zero-dependency parsers for common QR payload formats:
WiFi credentials, vCard contact data, and GS1 application identifiers. It is
the decode-side companion to the structured payload helpers in
[`qrcode-rs`](https://crates.io/crates/qrcode-rs).

```toml
[dependencies]
qrcode-parse = "2.0"
```

Use the facade crate instead when you also need QR encoding or rendering:

```toml
[dependencies]
qrcode-rs = "2.0"
```

## Features

| Feature | Purpose |
| --- | --- |
| `std` | Opts into the standard library. Disabled by default. |

## Modules

- `wifi`: parses `WIFI:T:...;S:...;P:...;;` payloads.
- `vcard`: parses vCard 2.1, 3.0, and 4.0 style payloads.
- `gs1`: parses GS1 AI strings into typed application-identifier data.
