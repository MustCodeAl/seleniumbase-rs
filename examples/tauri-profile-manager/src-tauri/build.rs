fn main() {
    // `capabilities/e2e/` grants the permission of the automation plugin, which
    // exists only when the `e2e` feature brings the plugin in: without the
    // feature the build would refuse that capability as naming an unknown
    // permission. So the folder is read only with the feature.
    let capabilities = if std::env::var_os("CARGO_FEATURE_E2E").is_some() {
        "./capabilities/**/*"
    } else {
        "./capabilities/*.json"
    };
    // A custom pattern makes watching the folder our job.
    println!("cargo:rerun-if-changed=capabilities");
    let attributes = tauri_build::Attributes::new().capabilities_path_pattern(capabilities);
    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("{error:#}");
    }
}
