# Cloud Integrations Tutorial

SeleniumBase for Rust can upload test artifacts (screenshots, logs, PDFs) to
Amazon S3, Azure Blob Storage, and Google Cloud Storage. Each backend is behind
an optional Cargo feature so the default build stays lightweight.

## Enabling backends

```toml
[dependencies]
seleniumbase-rs = { version = "0.1", features = ["s3", "azure", "gcp"] }
```

Or on the command line:

```bash
cargo run --features s3
```

## Amazon S3

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

## Azure Blob Storage

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
  --expiry 2025-12-31T00:00:00Z \
  --https-only
```

## Google Cloud Storage

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

## Running the example

```bash
cargo run --example cloud_upload --features s3,azure,gcp
```

The example only performs an upload when the corresponding feature is enabled
and the required environment variable is set.
