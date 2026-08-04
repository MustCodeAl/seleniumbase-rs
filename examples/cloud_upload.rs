#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // S3 upload example (requires --features s3)
    #[cfg(feature = "s3")]
    {
        use seleniumbase_rs::plugins::s3_logging_plugin::S3LoggingPlugin;
        use std::path::Path;
        S3LoggingPlugin::upload_file(
            "my-bucket",
            "seleniumbase/logs/session.log",
            Path::new("latest_logs/last_run.log"),
            "us-east-1",
        )
        .await?;
        println!("Uploaded to S3");
    }

    // Azure Blob upload example (requires --features azure)
    #[cfg(feature = "azure")]
    {
        use seleniumbase_rs::plugins::azure_logging_plugin::AzureLoggingPlugin;
        use std::path::Path;
        AzureLoggingPlugin::upload_file(Path::new("latest_logs/last_run.log")).await?;
        println!("Uploaded to Azure Blob Storage");
    }

    // Google Cloud Storage upload example (requires --features gcp)
    #[cfg(feature = "gcp")]
    {
        use seleniumbase_rs::plugins::gcp_logging_plugin::GcpLoggingPlugin;
        use std::path::Path;
        GcpLoggingPlugin::upload_file(
            "my-bucket",
            "seleniumbase/logs/session.log",
            Path::new("latest_logs/last_run.log"),
        )
        .await?;
        println!("Uploaded to GCS");
    }

    Ok(())
}
