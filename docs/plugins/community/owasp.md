---
title: "owasp Plugin"
sidebarTitle: "owasp"
description: "Defensive OWASP Top 10:2021 passive security auditor and OWASP PTK integration plugin inspecting HTTP headers, CSP, CORS, cookies, JWTs, DOM sinks, and client-side library versions with SARIF output."
---

<Badge color="purple">Community plugin</Badge> <Badge color="green">no unsafe</Badge>

`seleniumbase-rs-plugin-owasp` (`plugins/owasp` in [seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)) is a strictly **passive, read-only** security auditing plugin and library targeting applications you own or have explicit permission to test. It never sends fuzzing payloads, never mutates requests, and never exploits findings.

## Install

```sh Terminal
cargo add seleniumbase-rs-plugin-owasp --git https://github.com/MustCodeAl/seleniumbase-rs-plugins
```

## Passive auditors

| Auditor | What it evaluates |
|---|---|
| `audit_headers` / `audit_headers_map` | Security headers (HSTS, `X-Frame-Options`, `X-Content-Type-Options`, `Referrer-Policy`, `Permissions-Policy`), full CSP directive/source AST (`CspPolicy`, `CspDirective`, `CspSource`), permissive CORS (`Access-Control-Allow-Origin`), server version banners |
| `audit_cookies` / `audit_set_cookie` | `Secure`, `HttpOnly`, `SameSite`, scope, and `document.cookie` session exposure |
| `audit_jwt` / `decode_jwt` / `find_jwts_in_text` | Base64URL header/claims inspection (`alg: none`, sensitive claim leaks, expiration posture) |
| `audit_dom` | Mixed active/passive content, missing Subresource Integrity (`integrity` attributes), unsafe `target="_blank"` without `rel="noopener"`, `localStorage`/`sessionStorage` token exposure, HTML comment leaks, `postMessage` / inline DOM sinks |
| `audit_sca` | Client-side JavaScript library detection (`DetectedComponent`) and known outdated/EOL version warnings |
| `audit_forms` / `auditors::forms` | Passive form posture auditor inspecting form action URLs for insecure `http://` transmission, missing CSRF token fields, insecure password inputs, and plain-text transmission |
| OWASP PTK bridge (`PtkEngine`, `PtkModuleCatalog`) | Parses `modules.json` rules, awaits async `window.PTK_AGENT` snapshots (`scanStatus`, `getFindings`, `exportFullReport` to `PtkAgentSnapshot::export_plan`) when loaded alongside an authorized PTK extension (`PtkExtensionConfig`), and merges findings cleanly into `OwaspAuditReport` |

## Usage

### Passive header & SARIF report

```rust
use seleniumbase_rs_plugin_owasp::{
    HeaderAuditContext, OwaspAuditReport, audit_headers,
};

let headers = vec![
    ("Server".to_owned(), "nginx/1.18.0".to_owned()),
    ("Access-Control-Allow-Origin".to_owned(), "*".to_owned()),
];
let findings = audit_headers(
    &HeaderAuditContext::new("https://app.example.com/", 200, "text/html", &headers)
        .document(true),
);
let report = OwaspAuditReport::from_parts(
    Some("https://app.example.com/".to_owned()),
    findings,
    Vec::new(),
    1,
    0,
    None,
    true,
);
assert!(report.summary.total > 0);
assert_eq!(report.to_sarif()["version"], "2.1.0");
```

### Configuring `OwaspPlugin`

`OwaspConfig` toggles each auditor independently (`headers`, `cookies`, `jwt`, `dom`, `sca`), bounds in-memory history (`max_observed_responses`, `max_findings`), filters response observation via `observe_urls(...)`, and optionally attaches `PtkExtensionConfig`. Redacted secret handling (`redact_secret`) prevents raw tokens from appearing in reports.
