# Security Policy

## Supported versions

Security fixes are provided according to the following policy:

| Version line | Support |
| --- | --- |
| 2.x | Security updates and other maintenance fixes |
| 1.x | Critical security fixes when practical; migrate to 2.x for ongoing maintenance |
| 0.x | Not supported |

The repository currently declares version 2.1.0. Support status follows the
version line in the published release, not a branch name or a local checkout.

## Reporting a vulnerability

Please report suspected vulnerabilities privately through
[GitHub Security Advisories](https://github.com/houseme/qrcode-rs/security/advisories/new).
Do not open a public issue or include a proof of concept in a public pull
request before the issue has been coordinated with the maintainers.

Include enough information to reproduce and assess the report safely:

- the affected crate, feature flags, and release or commit;
- the input, API call, or rendering path that triggers the behavior;
- impact, prerequisites, and an indication of whether the issue is
  reproducible;
- a minimal proof of concept, if one can be shared privately; and
- any suggested mitigation or disclosure deadline.

Please do not include credentials, private user data, production data, or
network/service details that are not needed to reproduce the issue.

### Response targets

The maintainers aim to acknowledge a private report within 48 hours. For a
confirmed critical issue, the target is to provide a fix or a documented
mitigation within 7 calendar days when the affected code and reproduction are
available. These are response targets rather than a guarantee; complexity,
upstream coordination, and release validation can change the timeline. The
maintainers will keep the reporter informed when the assessment or disclosure
date changes.

Reports are triaged for validity, affected versions, exploitability, and
severity. Once a fix is available, the maintainers coordinate a public advisory,
release notes, and credit for the reporter if requested. Do not publish a
coordinated disclosure until the maintainers confirm that the advisory and
patched release are ready.

## Scope and security expectations

The in-scope components are the published `qrcode-rs` facade and workspace
crates, including optional renderers, parsers, decoders, and the `qrencodes`
command-line tool. A vulnerability must demonstrate a security impact such as
memory unsafety, denial of service from unbounded resource use, unintended
disclosure, or incorrect handling of security-sensitive input.

The encoder does not provide encryption, authentication, or confidentiality.
Applications must protect their input, output files, logs, and deployment
environment. Treat QR payloads as untrusted input when decoding or parsing
them, and apply application-specific limits before accepting user-controlled
payloads or producing very large images.

For implementation details and the current verification boundary, see the
[security audit summary](docs/security-audit.md).
