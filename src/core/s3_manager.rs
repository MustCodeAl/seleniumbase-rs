pub struct S3Manager {
    pub bucket: String,
}
impl S3Manager {
    pub fn new(bucket: &str) -> Self {
        Self {
            bucket: bucket.to_string(),
        }
    }
    pub fn upload_file(&self, filepath: &str) {
        println!("Uploading {} to S3 bucket {}", filepath, self.bucket);
    }
}
