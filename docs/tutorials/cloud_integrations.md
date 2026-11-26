---
title: "Cloud Integrations Tutorial"
sidebarTitle: "Cloud uploads"
---

SeleniumBase for Rust can upload test artifacts (screenshots, logs, PDFs) to
Amazon S3 directly from core (`s3` feature), and to Azure Blob Storage and
Google Cloud Storage via [`seleniumbase-rs-plugin-cloud-logging`](/plugins/community/cloud-logging)
in [seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)
(moved out of core in `v0.10.0`).

These types are uploaders (and, for `cloud-logging`, an optional `on_close` /
`Task` plugin): call them after a test, for example on the screenshot a failure
plugin saved. See [Test Plugins](/tutorials/test_plugins) for test lifecycle hooks.

<Info>

**What you will learn**

- How to enable the core `s3` feature and the `cloud-logging` plugin crate.
- How to authenticate with AWS, Azure, and GCS.
- How to upload files from a test.
- How to run the included `cloud_upload` example.

</Info>

<Visibility for="agents">

Summary for AI exports: `S3LoggingPlugin::upload_file(bucket, key, path, region)` lives in `seleniumbase-rs` behind feature `s3` (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, optional `AWS_REGION` and `AWS_ENDPOINT_URL`). In v0.10.0, Azure Blob Storage and Google Cloud Storage uploaders moved to `seleniumbase-rs-plugin-cloud-logging` (`/plugins/community/cloud-logging`): `AzureBlobUploader::from_env()?.upload(path)` (`AZURE_BLOB_URL` SAS URL) and `GcsUploader::from_env()?.upload(path, object_name)` (`SB_GCS_BUCKET`, `GCS_ACCESS_TOKEN`).

</Visibility>

## Enabling backends

Enable `s3` on `seleniumbase-rs` for AWS/S3-compatible uploads, and add `seleniumbase-rs-plugin-cloud-logging` for Azure Blob Storage or Google Cloud Storage:

<CodeGroup>

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", tag = "v0.10.0", features = ["s3"] }
seleniumbase-rs-plugin-cloud-logging = { git = "https://github.com/MustCodeAl/seleniumbase-rs-plugins", tag = "v0.10.0" }
```

```bash Command line
cargo run --example cloud_upload --features s3
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

<Badge color="purple">Plugin: seleniumbase-rs-plugin-cloud-logging</Badge>

Azure support uses a blob URL that already contains a <Tooltip tip="Shared access signature: a token in the URL query string that grants limited, time-bound access to a blob.">SAS token</Tooltip> with write
permission:

```bash
export AZURE_BLOB_URL="https://myaccount.blob.core.windows.net/mycontainer/myblob?sv=...&sig=...&sp=rw"
```

Upload from a test using `AzureBlobUploader` (see [`cloud-logging`](/plugins/community/cloud-logging)):

```rust
use seleniumbase_rs_plugin_cloud_logging::AzureBlobUploader;

let uploader = AzureBlobUploader::from_env()?;
uploader.upload("latest_logs/last_run.log").await?;
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

<Badge color="purple">Plugin: seleniumbase-rs-plugin-cloud-logging</Badge>

GCS support uses a bucket name and an OAuth access token:

```bash
export SB_GCS_BUCKET=my-bucket
export GCS_ACCESS_TOKEN=$(gcloud auth print-access-token)
```

Upload from a test using `GcsUploader` (see [`cloud-logging`](/plugins/community/cloud-logging)):

```rust
use seleniumbase_rs_plugin_cloud_logging::GcsUploader;

let uploader = GcsUploader::from_env()?;
uploader
    .upload("latest_logs/last_run.log", "seleniumbase/logs/session.log")
    .await?;
```

</View>

## Running the example

```bash
cargo run --example cloud_upload --features s3
```

The example writes a small log file to `latest_logs/` and uploads to S3 when the
`s3` feature is enabled and its environment variables are set:

| Backend | Crate / feature | Variables |
|---|---|---|
| Amazon S3 | `seleniumbase-rs` (`s3`) | `SB_S3_BUCKET`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`; optional `AWS_REGION` and `AWS_ENDPOINT_URL` |
| Azure Blob Storage | `seleniumbase-rs-plugin-cloud-logging` | `AZURE_BLOB_URL` |
| Google Cloud Storage | `seleniumbase-rs-plugin-cloud-logging` | `SB_GCS_BUCKET`, `GCS_ACCESS_TOKEN` |

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
