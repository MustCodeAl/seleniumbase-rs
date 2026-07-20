# Recorder CLI

The recorder captures browser interactions and emits a Rust test or JSON scenario.

## Record a Rust test

```bash
cargo run --bin sbase -- recorder --output my_test.rs
```

## Record a JSON scenario

```bash
cargo run --bin sbase -- recorder --format json --output scenario.json
```

## Options

| Option | Description |
|--------|-------------|
| `--output PATH` | Output file path. |
| `--format rust|json` | Output format. |
| `--url URL` | Starting URL. |

## After recording

Run the generated Rust test:

```bash
cargo run --example my_test
```

Or replay the JSON scenario:

```bash
cargo run --bin sbase -- run-scenario --file scenario.json
```
