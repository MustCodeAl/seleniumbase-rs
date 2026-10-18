---
title: "Cloud Integrations Tutorial"
---

# Cloud Integrations Tutorial

SeleniumBase for Rust can upload test artifacts (screenshots, logs, PDFs) to
Amazon S3, Azure Blob Storage, and Google Cloud Storage. Each backend is behind
an optional Cargo feature so the default build stays lightweight.

These types are uploaders, not test hooks: call `upload_file` after a test, for
example on the screenshot a failure plugin saved. See
[Test Plugins](/tutorials/test_plugins) for the hooks.

## What you will learn

- Which Cargo features enable each cloud backend.
- How to authenticate with AWS, Azure, and GCS.
- How to upload files from a test.
- How to run the included `cloud_upload` example.

## Enabling backends

Add the features you need, in `Cargo.toml` or on the command line:

<CodeGroup>

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { version = "0.1", features = ["s3", "azure", "gcp"] }
```

```bash Command line
cargo run --features s3
```

</CodeGroup>

## Providers

<Tabs>

<Tab title="Amazon S3">

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

</Tab>

<Tab title="Azure Blob Storage">

Azure support uses a blob URL that already contains a SAS token with write
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

</Tab>

<Tab title="Google Cloud Storage">

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

</Tab>

</Tabs>

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

| Symptom | Likely cause | Fix |
|---|---|---|
| Upload function not found | Feature not enabled | Add the matching feature in `Cargo.toml`. |
| S3 credentials error | Missing `AWS_*` variables | Export `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`. |
| Azure SAS rejected | Token lacks write permission | Regenerate with `--permissions rw`. |
| GCS token expired | `GCS_ACCESS_TOKEN` timed out | Re-run `gcloud auth print-access-token`. |
