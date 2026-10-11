//! Upload a test artifact (a log or a screenshot) to cloud storage.
//!
//! The example writes a small log file to `latest_logs/` and uploads it to each
//! backend that was compiled in and has its environment variables set. Each
//! backend is its own Cargo feature, reads its credentials from the environment,
//! and is tried independently, so one failure does not hide the others. Nothing
//! is hard-coded: bucket names come from the environment too.
//!
//! | Feature | Environment                                                                  |
//! |---------|------------------------------------------------------------------------------|
//! | `s3`    | `SB_S3_BUCKET`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`; optional `AWS_REGION` (default `us-east-1`) and `AWS_ENDPOINT_URL` for S3-compatible stores such as MinIO |
//! | `azure` | `AZURE_BLOB_URL`, a blob URL with a write-enabled SAS token                  |
//! | `gcp`   | `SB_GCS_BUCKET`, `GCS_ACCESS_TOKEN` (for example from `gcloud auth print-access-token`) |
//!
//! Run it with one or more of the features:
//!
//! ```bash
//! cargo run --example cloud_upload --features s3
//! cargo run --example cloud_upload --features s3,azure,gcp
//! ```
//!
//! A real upload needs the network and your own cloud account. Without any of
//! the three features, or with a backend's variables unset, the example only
//! says what is missing and exits successfully. It exits with an error if an
//! upload that was attempted fails.

use std::path::PathBuf;

use seleniumbase_rs::artifacts::ensure_latest_logs_dir;

/// What happened to one backend.
#[cfg_attr(
    not(any(feature = "s3", feature = "azure", feature = "gcp")),
    allow(
        dead_code,
        reason = "without a backend feature nothing builds a value of this type"
    )
)]
enum Outcome {
    Uploaded,
    /// The backend is compiled in, but its environment is not set.
    Skipped(String),
    Failed(String),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifact = write_demo_artifact()?;
    println!("artifact: {}", artifact.display());

    // One entry per backend that is compiled in.
    let outcomes: Vec<(&str, Outcome)> = Vec::from([
        #[cfg(feature = "s3")]
        ("S3", upload_to_s3(&artifact).await),
        #[cfg(feature = "azure")]
        ("Azure Blob Storage", upload_to_azure(&artifact).await),
        #[cfg(feature = "gcp")]
        ("Google Cloud Storage", upload_to_gcs(&artifact).await),
    ]);

    if outcomes.is_empty() {
        println!("No cloud backend is compiled in, so nothing was uploaded.");
        println!("Re-run with --features s3, azure or gcp (see the table in this file).");
        return Ok(());
    }

    let mut failed = 0;
    for (backend, outcome) in &outcomes {
        match outcome {
            Outcome::Uploaded => println!("ok      {backend}"),
            Outcome::Skipped(reason) => println!("skipped {backend}: {reason}"),
            Outcome::Failed(error) => {
                failed += 1;
                println!("FAILED  {backend}: {error}");
            }
        }
    }
    if failed > 0 {
        return Err(format!("{failed} upload(s) failed").into());
    }
    Ok(())
}

/// The file to upload. A real test would upload what it already saved, such as
/// a screenshot taken on failure; this one makes a file so the example is
/// self-contained.
fn write_demo_artifact() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = ensure_latest_logs_dir()?.join("cloud_upload_demo.log");
    std::fs::write(
        &path,
        "cloud_upload example: this file proves the upload path works\n",
    )?;
    Ok(path)
}

/// A message naming the variables of `names` that are not set, or `None` when
/// they all are.
#[cfg(any(feature = "s3", feature = "azure", feature = "gcp"))]
fn unset_variables(names: &[&str]) -> Option<String> {
    let unset: Vec<&str> = names
        .iter()
        .copied()
        .filter(|name| std::env::var_os(name).is_none())
        .collect();
    (!unset.is_empty()).then(|| format!("set {}", unset.join(", ")))
}

/// The object name to store `artifact` under.
#[cfg(any(feature = "s3", feature = "gcp"))]
fn object_name(artifact: &std::path::Path) -> String {
    let file = artifact
        .file_name()
        .map_or_else(|| "artifact".into(), |name| name.to_string_lossy());
    format!("seleniumbase/logs/{file}")
}

#[cfg(feature = "s3")]
async fn upload_to_s3(artifact: &std::path::Path) -> Outcome {
    use seleniumbase_rs::plugins::s3_logging_plugin::S3LoggingPlugin;

    if let Some(reason) =
        unset_variables(&["SB_S3_BUCKET", "AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"])
    {
        return Outcome::Skipped(reason);
    }
    let bucket = std::env::var("SB_S3_BUCKET").unwrap_or_default();
    let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_owned());
    match S3LoggingPlugin::upload_file(&bucket, &object_name(artifact), artifact, &region).await {
        Ok(()) => Outcome::Uploaded,
        Err(error) => Outcome::Failed(error.to_string()),
    }
}

#[cfg(feature = "azure")]
async fn upload_to_azure(artifact: &std::path::Path) -> Outcome {
    use seleniumbase_rs::plugins::azure_logging_plugin::AzureLoggingPlugin;

    if let Some(reason) = unset_variables(&["AZURE_BLOB_URL"]) {
        return Outcome::Skipped(reason);
    }
    match AzureLoggingPlugin::upload_file(artifact).await {
        Ok(()) => Outcome::Uploaded,
        Err(error) => Outcome::Failed(error.to_string()),
    }
}

#[cfg(feature = "gcp")]
async fn upload_to_gcs(artifact: &std::path::Path) -> Outcome {
    use seleniumbase_rs::plugins::gcp_logging_plugin::GcpLoggingPlugin;

    if let Some(reason) = unset_variables(&["SB_GCS_BUCKET", "GCS_ACCESS_TOKEN"]) {
        return Outcome::Skipped(reason);
    }
    let bucket = std::env::var("SB_GCS_BUCKET").unwrap_or_default();
    match GcpLoggingPlugin::upload_file(&bucket, &object_name(artifact), artifact).await {
        Ok(()) => Outcome::Uploaded,
        Err(error) => Outcome::Failed(error.to_string()),
    }
}
