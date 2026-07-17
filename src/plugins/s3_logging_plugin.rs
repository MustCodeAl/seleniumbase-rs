use crate::core::s3_manager::S3Manager;

pub fn log_to_s3(bucket: &str, filepath: &str) {
    let s3 = S3Manager::new(bucket);
    s3.upload_file(filepath);
}
