---
title: "ocr Plugin"
sidebarTitle: "ocr"
description: "Standalone OCR plugin providing pinned model cache resolution (SB_OCR_MODELS), PNG validation, SHA-256 verification, and Task orchestration."
---

<Badge color="purple">Community plugin</Badge> <Badge color="green">no unsafe</Badge>

`seleniumbase-rs-plugin-ocr` (`plugins/ocr` in [seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)) manages OCR model cache resolution, SHA-256 integrity checks, PNG validation, and `Task` execution for screenshot text extraction workflows.

In `v0.10.0`, OCR support moved out of `seleniumbase-rs` core (`src/ocr.rs`) into this standalone plugin crate so core builds do not carry OCR dependencies.

<Visibility for="agents">

Summary for AI exports: `seleniumbase-rs-plugin-ocr` (plugin name `"ocr"`) exports `OCR_MODELS` (pinned filenames and SHA-256 digests for `text-detection.rten` and `text-recognition.rten`), `resolve_models_dir(explicit)` (honoring `SB_OCR_MODELS` or the platform cache directory), `validate_png_header(bytes)`, `sha256_hex(bytes)`, `OcrConfig`, `OcrPlugin`, and `OcrError`. Declares `Capability::Task` and `Capability::HostFsRead(PathScope::RunOutput)`.

</Visibility>

## Install

```sh Terminal
cargo add seleniumbase-rs-plugin-ocr --git https://github.com/MustCodeAl/seleniumbase-rs-plugins --tag v0.10.0
```

## Public API

| Item | Description |
|---|---|
| `OCR_MODELS` | Pinned `(&str, &str)` pairs of model filename and expected SHA-256 digest (`text-detection.rten` and `text-recognition.rten`) |
| `resolve_models_dir(explicit)` | Resolves the model folder from `explicit`, then `SB_OCR_MODELS`, then `<cache_dir>/seleniumbase-rs/ocr` |
| `validate_png_header(bytes)` | Checks the 8-byte PNG signature (`\x89PNG\r\n\x1a\n`), returning `OcrError::InvalidImage` on non-PNG input |
| `sha256_hex(bytes)` | Computes lowercase hexadecimal SHA-256 digest via `ring` |
| `OcrConfig` | Configuration struct with `models_dir: Option<PathBuf>` and `image_path: Option<PathBuf>` (`deny_unknown_fields`) |
| `OcrPlugin` | Implements `Plugin`, `Configurable` (`"ocr"`), and `Task` with `Requirements::empty()` |
| `OcrError` | Typed error enum (`InvalidConfig`, `Io`, `InvalidImage`) |

## Usage

```rust
use std::path::Path;
use seleniumbase_rs_plugin_ocr::{
    OCR_MODELS, OcrConfig, OcrPlugin, resolve_models_dir, sha256_hex, validate_png_header,
};

fn check_ocr_inputs(png_bytes: &[u8]) -> seleniumbase_rs_plugin_ocr::Result<()> {
    validate_png_header(png_bytes)?;
    let models_dir = resolve_models_dir(None)?;
    println!("Models cache: {} ({} pinned models)", models_dir.display(), OCR_MODELS.len());
    println!("PNG sha256: {}", sha256_hex(png_bytes));

    let _plugin = OcrPlugin::new(OcrConfig {
        models_dir: Some(models_dir),
        image_path: Some(Path::new("home.png").to_path_buf()),
    });
    Ok(())
}
```

### Configuration (`[plugins.ocr]`)

```toml sbase_config.toml
[plugins.ocr]
models_dir = "/opt/ocr-models"
image_path = "latest_logs/screenshot.png"
```

On air-gapped machines, set `SB_OCR_MODELS` to the directory holding `text-detection.rten` and `text-recognition.rten`.
