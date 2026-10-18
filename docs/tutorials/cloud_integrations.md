---
title: "Cloud Integrations Tutorial"
---

SeleniumBase for Rust can upload test artifacts (screenshots, logs, PDFs) to
Amazon S3, Azure Blob Storage, and Google Cloud Storage. Each backend is behind
an optional Cargo feature so the default build stays lightweight.

These types are uploaders, not test hooks: call `upload_file` after a test, for
example on the screenshot a failure plugin saved. See
[Test Plugins](/tutorials/test_plugins) for the hooks.

<Info>

**What you will learn**

- Which Cargo features enable each cloud backend.
- How to authenticate with AWS, Azure, and GCS.
- How to upload files from a test.
- How to run the included `cloud_upload` example.

</Info>

<Visibility for="agents">

Summary for AI exports: these types are uploaders, not test hooks. `S3LoggingPlugin::upload_file(bucket, key, path, region)` needs feature `s3` and `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` (optional `AWS_REGION` and `AWS_ENDPOINT_URL`). `AzureLoggingPlugin::upload_file(path)` needs feature `azure` and `AZURE_BLOB_URL`, a blob URL with a SAS token that has write permission. `GcpLoggingPlugin::upload_file(bucket, key, path)` needs feature `gcp` and `GCS_ACCESS_TOKEN`. Try them with `cargo run --example cloud_upload --features s3,azure,gcp`.

</Visibility>

## Enabling backends

Add the features you need, in `Cargo.toml` or on the command line:

<CodeGroup>

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["s3", "azure", "gcp"] }
```

```bash Command line
cargo run --features s3
```

</CodeGroup>

## Providers

<View title="Amazon S3" icon="aws" iconType="brands">

<Badge color="blue">Feature: s3</Badge>

Set the standard AWS environment variables:

```bash
export AWS_ACCESS_KEY_ID=AKIA...
export AWS_SECRET_ACCESS_KEY=...
export AWS_REGION=us-east-1   # optional fallback
```

MinIO and other S3-compatible stores are supported via `AWS_ENDPOINT_URL`:

```bash
export AWS_ENDPOINT_URL=https://minio.example.com
```

Upload from a test:

```rust
use seleniumbase_rs::plugins::s3_logging_plugin::S3LoggingPlugin;
use std::path::Path;

S3LoggingPlugin::upload_file(
    "my-bucket",
    "seleniumbase/logs/session.log",
    Path::new("latest_logs/last_run.log"),
    "us-east-1",
).await?;
```

</View>

<View title="Azure Blob Storage" icon="microsoft" iconType="brands">

<Badge color="blue">Feature: azure</Badge>

Azure support uses a blob URL that already contains a <Tooltip tip="Shared access signature: a token in the URL query string that grants limited, time-bound access to a blob.">SAS token</Tooltip> with write
permission:

```bash
export AZURE_BLOB_URL="https://myaccount.blob.core.windows.net/mycontainer/myblob?sv=...&sig=...&sp=rw"
```

Upload from a test:

```rust
use seleniumbase_rs::plugins::azure_logging_plugin::AzureLoggingPlugin;
use std::path::Path;

AzureLoggingPlugin::upload_file(Path::new("latest_logs/last_run.log")).await?;
```

Generate a SAS token with the Azure CLI:

```bash
az storage blob generate-sas \
  --account-name myaccount \
  --container-name mycontainer \
  --name myblob \
  --permissions rw \
  --expiry 2027-12-31T00:00:00Z \
  --https-only
```

</View>

<View title="Google Cloud Storage" icon="google" iconType="brands">

<Badge color="blue">Feature: gcp</Badge>

GCS support uses an OAuth access token. The easiest way to obtain one is:

```bash
export GCS_ACCESS_TOKEN=$(gcloud auth print-access-token)
```

Upload from a test:

```rust
use seleniumbase_rs::plugins::gcp_logging_plugin::GcpLoggingPlugin;
use std::path::Path;

GcpLoggingPlugin::upload_file(
    "my-bucket",
    "seleniumbase/logs/session.log",
    Path::new("latest_logs/last_run.log"),
).await?;
```

</View>

## Running the example

```bash
cargo run --example cloud_upload --features s3,azure,gcp
```

The example writes a small log file to `latest_logs/` and only performs an
upload when the corresponding feature is enabled and its environment variables
are set. A backend that is compiled in but not configured is reported as
skipped, with the variables to set; the example fails only when an upload it
attempted fails. Besides the credentials described above, it reads the bucket
name from the environment, so nothing is hard-coded:

| Feature | Variables the example reads |
|---|---|
| `s3` | `SB_S3_BUCKET`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`; optional `AWS_REGION` and `AWS_ENDPOINT_URL` |
| `azure` | `AZURE_BLOB_URL` |
| `gcp` | `SB_GCS_BUCKET`, `GCS_ACCESS_TOKEN` |

## Troubleshooting

<AccordionGroup>

<Accordion title="Upload function not found" icon="magnifying-glass">

**Likely cause:** Feature not enabled.

**Fix:** Add the matching feature in `Cargo.toml`.

</Accordion>

<Accordion title="S3 credentials error" icon="key">

**Likely cause:** Missing `AWS_*` variables.

**Fix:** Export `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`.

</Accordion>

<Accordion title="Azure SAS rejected" icon="ban">

**Likely cause:** Token lacks write permission.

**Fix:** Regenerate with `--permissions rw`.

</Accordion>

<Accordion title="GCS token expired" icon="clock">

**Likely cause:** `GCS_ACCESS_TOKEN` timed out.

**Fix:** Re-run `gcloud auth print-access-token`.

</Accordion>

</AccordionGroup>
