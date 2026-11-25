---
title: "behavior-dataset Plugin"
sidebarTitle: "behavior-dataset"
description: "Synthetic human-like web behavior generator and batch ML training dataset exporter (JSON, JSONL, CSV events, 33-column feature matrix) built on seleniumbase-rs kinematics."
---

<Badge color="purple">Community plugin</Badge> <Badge color="green">no unsafe</Badge>

`seleniumbase-rs-plugin-behavior-dataset` (`plugins/behavior-dataset` in [seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)) generates reproducible, synthetic human interaction timelines built on `seleniumbase_rs::evasion::behavior`, extracts kinematic and Fitts's-law ML features, optionally pairs them with synthetic linear baseline negatives, and exports training-ready datasets.

## Install

```sh Terminal
cargo add seleniumbase-rs-plugin-behavior-dataset --git https://github.com/MustCodeAl/seleniumbase-rs-plugins
```

## Personas, scenarios, and export formats

- **Personas (`PersonaKind::ALL`)**: `FastTypist`, `AverageDesktop`, `CarefulReader`, `TrackpadUser`, `MobileTouch`, `DistractedBrowser`.
- **Export formats (`ExportFormat::ALL`)**:
  - `Json` (`"json"`): Pretty-printed `DatasetBatch`.
  - `JsonlSamples` (`"jsonl_samples"`): One `DatasetSample` per line.
  - `JsonlEvents` (`"jsonl_events"`): Flattened `JsonlEventRow` stream per event.
  - `CsvEvents` (`"csv_events"`): 20-column flat table with one row per `BehaviorEvent`.
  - `CsvFeatures` (`"csv_features"`): 33-column ML feature matrix (`straightness_index`, `tremor_high_freq_energy`, `mean_velocity_px_s`, Fitts timing, dwell, overshoot, typos).

## Usage

### Generating and exporting datasets in Rust

```rust
use seleniumbase_rs_plugin_behavior_dataset::{
    DatasetBatchGenerator, DatasetConfig, ExportFormat,
};

let config = DatasetConfig {
    sample_count: 4,
    base_seed: 7,
    include_baseline_negatives: true,
    ..DatasetConfig::default()
};
let batch = DatasetBatchGenerator::new(config)?.generate()?;
assert_eq!(batch.human_count(), 4);
assert_eq!(batch.baseline_count(), 4);

let csv = batch.export(ExportFormat::CsvFeatures)?;
assert_eq!(csv.lines().count(), 9);
Ok::<(), seleniumbase_rs_plugin_behavior_dataset::DatasetError>(())
```

### Plugin & task integration

`BehaviorDatasetPlugin` (plugin name `"behavior-dataset"`) requires no Chromium browser (`Requirements::empty()`) and declares `Capability::Task`. It implements `Configurable` under `[plugins.behavior-dataset]` and accepts task overrides (`sample_count`, `base_seed`, `personas`, `scenarios`, `include_baseline_negatives`, `export_format`, `include_export`).
