---
title: "cloud-logging Plugin"
sidebarTitle: "cloud-logging"
description: "Standalone plugin and library uploaders for sending test artifacts to Google Cloud Storage (GcsUploader) and Azure Blob Storage (AzureBlobUploader)."
---

<Badge color="purple">Community plugin</Badge> <Badge color="green">no unsafe</Badge>

`seleniumbase-rs-plugin-cloud-logging` (`plugins/cloud-logging` in [seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)) uploads test artifacts (screenshots, logs, traces, HAR files) to **Google Cloud Storage** or **Azure Blob Storage**.

In `v0.10.0`, the GCS and Azure uploaders moved out of `seleniumbase-rs` core into this plugin crate so core builds stay lightweight (`s3` remains in core behind its optional feature; see [Cloud uploads](/tutorials/cloud_integrations)).

<Visibility for="agents">

Summary for AI exports: `seleniumbase-rs-plugin-cloud-logging` (plugin name `"cloud-logging"`) provides `GcsUploader` (`SB_GCS_BUCKET`, `GCS_ACCESS_TOKEN`), `AzureBlobUploader` (`AZURE_BLOB_URL` SAS PUT URL), `CloudLoggingPlugin`, `CloudLoggingConfig` (`provider`: `"gcs"` or `"azure"`, `artifact_path`, `object_name`), `CloudProvider`, and `CloudUploadError`. Uploads run directly via `upload(...)`, automatically on `Plugin::on_close`, or on demand through `Task::run`. Declares `Capability::Task`, `Capability::HostNet(NetScope::Any)`, and `Capability::HostFsRead(PathScope::RunOutput)`.

</Visibility>

## Install

```sh Terminal
cargo add seleniumbase-rs-plugin-cloud-logging --git https://github.com/MustCodeAl/seleniumbase-rs-plugins --tag v0.10.0
```

## Direct uploaders

### Google Cloud Storage (`GcsUploader`)

Set `SB_GCS_BUCKET` and an OAuth bearer token in `GCS_ACCESS_TOKEN`:

```bash Terminal
export SB_GCS_BUCKET=my-artifacts-bucket
export GCS_ACCESS_TOKEN=$(gcloud auth print-access-token)
```

```rust
use seleniumbase_rs_plugin_cloud_logging::GcsUploader;

async fn upload_to_gcs() -> seleniumbase_rs_plugin_cloud_logging::Result<()> {
    let uploader = GcsUploader::from_env()?;
    uploader
        .upload("latest_logs/last_run.log", "seleniumbase/logs/session.log")
        .await
}
```

Use `GcsUploader::from_lookup(|name| ...)` in tests or custom secret loaders without mutating process environment variables.

### Azure Blob Storage (`AzureBlobUploader`)

Set `AZURE_BLOB_URL` to a full blob URL including a SAS query string with write permission (`sp=rw` or `sp=cw`):

```bash Terminal
export AZURE_BLOB_URL="https://myaccount.blob.core.windows.net/mycontainer/session.log?sv=...&sig=...&sp=rw"
```

```rust
use seleniumbase_rs_plugin_cloud_logging::AzureBlobUploader;

async fn upload_to_azure() -> seleniumbase_rs_plugin_cloud_logging::Result<()> {
    let uploader = AzureBlobUploader::from_env()?;
    uploader.upload("latest_logs/last_run.log").await
}
```

`AzureBlobUploader::from_lookup` validates that the URL has a host, account segment, container, blob name, and non-empty SAS query string before sending `PUT` with `x-ms-blob-type: BlockBlob`.

## Using `CloudLoggingPlugin`

`CloudLoggingPlugin` implements `Plugin`, `Configurable`, and `Task`:

- **Configuration (`[plugins.cloud-logging]`)**:
  - `provider`: `"gcs"` (default) or `"azure"`.
  - `artifact_path`: optional local file path uploaded automatically in `on_close` (or used as default by `Task::run`).
  - `object_name`: optional GCS object name (defaults to the local filename when omitted).
- **Capabilities**: `Capability::Task`, `Capability::HostNet(NetScope::Any)`, `Capability::HostFsRead(PathScope::RunOutput)`.
- **Error type**: `CloudUploadError` (`MissingEnv`, `InvalidTarget`, `Io`, `Upload { status, body }`, `Request`).
