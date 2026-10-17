---
title: "wasm Plugin Host"
---

# wasm: plugins in WebAssembly

Run [seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs) plugins that
are written in **any language that compiles to WebAssembly** (Rust, C, Zig, Go,
AssemblyScript, or plain WAT), sandboxed by the pure-Rust interpreter
[wasmi](https://github.com/wasmi-labs/wasmi).

A module describes a plugin in data: a name, browser switches, scripts for
pages, URL patterns to block. The host reads that data **once**, when it loads
the module, checks every value, and then drops the module. Nothing of the module
runs while the browser does. The module has no imports: no files, network,
clock or randomness, and it cannot call back into the host. Only pure functions
cross the boundary.

```sh
cargo add seleniumbase-rs-plugin-wasm --git https://github.com/MustCodeAl/seleniumbase-rs-plugins --rev 628991af0e6c5711298e3bd3692dc2974887401a
```

<Info>

Install it from the same revision of the plugins repository that your seleniumbase-rs pins; see [Installing a plugin](../tutorials/plugins.md#installing-a-plugin). The plugins overview is [here](overview.md).

</Info>

## Use it

```rust,no_run
use seleniumbase_rs_plugin::PluginSet;
use seleniumbase_rs_plugin_wasm::WasmPlugin;

let plugins = PluginSet::new().with(WasmPlugin::from_file("hello.wasm")?)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`WasmPlugin::from_bytes` takes the module from memory instead, and
`from_file_with` / `from_bytes_with` take a `Limits` to change the defaults
below. Loading fails with a `WasmError` that says what is wrong with the module;
it never panics and never hangs. A `WasmError` converts into a `PluginError`, so
`?` works in code that returns the plugin API's `Result`.

Loading is synchronous and does the interpreter's work on the calling thread. A
well-behaved module takes microseconds to milliseconds; a hostile one can spend
its whole fuel budget on each of its calls and then fails. In async code, load
modules at start-up or inside `spawn_blocking`.

The loaded `WasmPlugin` is an ordinary `Plugin`:

| Hook | What it does with the module's data |
|---|---|
| `name` | The name from `sbrs_name`. |
| `packages` | One `Package::script` per entry of `sbrs_scripts`, loaded `only_on` its hosts or `everywhere`. |
| `before_launch` | Appends the arguments of `sbrs_launch_args` to `LaunchSettings::args`. |
| `on_page_created` | When there are blocked URLs: `Network.enable`, then `Network.setBlockedURLs` with the patterns. |
| `requirements` | `CHROMIUM_ONLY` when the module blocks URLs (it uses the DevTools Protocol); none otherwise. |

## The ABI, version 1

A module exports a linear memory called `memory` and these functions. None takes
a parameter.

| Export | Required | Returns |
|---|---|---|
| `memory` | yes | The module's linear memory (32-bit; at most one). |
| `sbrs_abi_version() -> i32` | yes | `1`. Any other value is refused. |
| `sbrs_name() -> i64` | yes | A JSON string: the plugin's name. |
| `sbrs_launch_args() -> i64` | no | A JSON array of strings: browser switches. |
| `sbrs_scripts() -> i64` | no | A JSON array of script objects. |
| `sbrs_blocked_urls() -> i64` | no | A JSON array of strings: URL patterns to block. |

An optional export that is missing means "none". An export that is present with
the wrong type is an error. The `sbrs_` prefix is reserved for the ABI: an
export that starts with it and is not in the table (a typo such as
`sbrs_script`) is an error, so a mistake is never silently ignored. Exports with other names are ignored, so a module can export
what it likes.

### Returning a string: the packed `i64`

A function that returns text returns one `i64` that packs a pointer and a
length, and leaves the UTF-8 JSON text in its memory at that place:

```text
value = (pointer << 32) | length          pointer, length: u32 (bytes)
```

The host reads `memory[pointer .. pointer + length]` right after the call
returns, so the bytes must stay valid until then: a `static` string, or a
buffer the module leaks. A range outside the memory, or longer than the limit,
is an error. The text must be valid UTF-8 and exactly one JSON value, with
nothing after it.

### The values

**`sbrs_name`**: a JSON string of 1 to 64 characters from `a-z`, `0-9`, `-` and
`_`, starting with a letter or a digit, such as `"my-plugin"`. It must be unique
in the `PluginSet`, like any plugin name.

**`sbrs_launch_args`**: at most 256 strings of at most 4096 bytes. Each is a
switch: it starts with `-`, followed by a name that starts with a letter or a
digit (`"--lang=de"`), and holds no control characters. A bare `--`, or anything
that is not a switch, is refused because the browser would take it for a URL.

**`sbrs_scripts`**: at most 64 objects with exactly these two fields and no
others:

```json
[
  { "code": "window.demo = 1", "hosts": ["example.com", "example.org"] },
  { "code": "window.all = 1",  "hosts": "everywhere" }
]
```

`code` is non-empty JavaScript, run in every matching document before the
page's own scripts. `hosts` is either the string `"everywhere"` or a list of 1
to 64 lower-case host names (`example.com`: the host and its subdomains).
Saying where a script runs is not optional, as for any `Package`.

**`sbrs_blocked_urls`**: at most 1024 strings of 1 to 2048 bytes, Chrome URL
patterns where `*` matches anything (`"*.png"`, `"*://ads.example.com/*"`).

### Limits

The host applies these to every module. `Limits` changes the first four.

| Limit | Default |
|---|---|
| Module size | 4 MiB |
| Memory (at start and after any `memory.grow`) | 16 MiB |
| Fuel for each call, and for the start function | 20,000,000 (roughly as many instructions) |
| Size of one returned JSON value | 1 MiB |
| Call stack | wasmi's defaults; a recursion bomb traps |
| Imports | none: a module that imports anything is refused |
| Memories, tables | 1 memory; tables of at most 100,000 elements; wasmi's strict limits on functions, globals and parameters |
| Memory page size, memory64 | the standard 64 KiB page and 32-bit addresses only |

A function that runs out of fuel stops with `WasmError::OutOfFuel`; a trap
(`unreachable`, stack overflow, out-of-bounds access) is `WasmError::Trapped`.
`memory.grow` past the cap returns `-1`, as the WebAssembly specification says.
The text format is not accepted: give `from_bytes` a binary module (use `wat` or
`wasm-tools` to build one from WAT).

### What a module can still do

<Warning>

The sandbox keeps a module from touching your machine while it is loaded. What
it describes then reaches three places, so review a module you did not write.

</Warning>

- **Scripts** run inside pages, with the page's rights. A script for
  `"everywhere"` sees every page, cookie and password in the browser, and can send
  them anywhere the page can. Prefer `hosts` lists.
- **Browser switches** go to the browser. The host refuses the ones known to
  run programs or switch off the browser's protections, by name, wherever they
  appear in the list: `renderer-cmd-prefix`, `utility-cmd-prefix`,
  `zygote-cmd-prefix`, `gpu-launcher`, `ppapi-plugin-launcher`,
  `browser-subprocess-path`, `nacl-gdb`, `no-sandbox`, `disable-setuid-sandbox`,
  `disable-gpu-sandbox`, `no-zygote`, `disable-web-security`,
  `ignore-certificate-errors`, `proxy-server`, `proxy-pac-url`,
  `remote-debugging-port`, `remote-debugging-address`, `remote-debugging-pipe`,
  `remote-allow-origins`, `load-extension`, `disable-extensions-except` and
  `user-data-dir` (`FORBIDDEN_FLAGS` in the code). That list is a floor, not a
  complete audit of Chrome's switches: read `WasmPlugin::launch_args()` if you
  must be sure.
- **Blocked URLs** change what a page can load.

## Write a module

A module can be written in WAT or in any language that compiles to WebAssembly. Two examples:

<Tabs>

<Tab title="WAT">

[`examples/hello.wat`](https://github.com/MustCodeAl/seleniumbase-rs-plugins/blob/main/plugins/wasm/examples/hello.wat) is a whole plugin: it is named
`hello` and logs a line on `example.com`. The two strings sit in the data
segments; each function returns the pointer in the high half and the length in
the low half.

```wat
(module
  (memory (export "memory") 1)

  ;; The strings the host reads: a JSON string and a JSON array.
  (data (i32.const 16) "\"hello\"")
  (data (i32.const 32) "[{\"code\":\"console.log('hello from wasm')\",\"hosts\":[\"example.com\"]}]")

  (func (export "sbrs_abi_version") (result i32)
    (i32.const 1))

  ;; Pointer 16 in the high half, length 7 in the low half.
  (func (export "sbrs_name") (result i64)
    (i64.const 0x0000_0010_0000_0007))

  ;; Pointer 32, length 67.
  (func (export "sbrs_scripts") (result i64)
    (i64.const 0x0000_0020_0000_0043)))
```

Build the binary with `wasm-tools parse examples/hello.wat -o hello.wasm`, or
in Rust with `wat::parse_str`.

</Tab>

<Tab title="Rust">

Compile a plugin to `wasm32-unknown-unknown` as a `cdylib`, with no dependencies
and no `unsafe`:

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]
```

```rust,ignore
// src/lib.rs
const NAME: &str = "\"hello\"";
const SCRIPTS: &str =
    r#"[{"code":"console.log('hello from wasm')","hosts":["example.com"]}]"#;

/// Packs the address and length of `text` into one `i64`.
fn pack(text: &'static str) -> i64 {
    let pointer = text.as_ptr() as usize as u64; // an address in linear memory
    ((pointer << 32) | text.len() as u64) as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn sbrs_abi_version() -> i32 {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn sbrs_name() -> i64 {
    pack(NAME)
}

#[unsafe(no_mangle)]
pub extern "C" fn sbrs_scripts() -> i64 {
    pack(SCRIPTS)
}
```

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
# the plugin: target/wasm32-unknown-unknown/release/<crate_name>.wasm
```

To build the JSON at run time (with `serde_json`, say), leak the `String` so it
stays valid after the function returns: `pack(Box::leak(json.into_boxed_str()))`.
The host calls each function once and then discards the module, so the leak is
a single allocation.

</Tab>

</Tabs>

## Test a module

Load it, then run the plugin against the recording fakes of
`seleniumbase-rs-plugin-testing`: no browser, no module-specific harness.

```rust,ignore
use seleniumbase_rs_plugin::PluginSet;
use seleniumbase_rs_plugin_testing::{RecordingPage, block_on};
use seleniumbase_rs_plugin_wasm::WasmPlugin;

let set = PluginSet::new().with(WasmPlugin::from_file("hello.wasm")?)?;
let page = RecordingPage::new();
block_on(set.on_page_created(&page))?;
assert_eq!(page.calls().len(), 1); // the one script
```

## Versioning

The ABI version is the number `sbrs_abi_version` returns. A change that an
existing module could not survive (a new required export, another meaning for a
field) bumps it. A host refuses a module of a version it does not speak, with
`WasmError::UnsupportedAbi`, before it looks at anything else. For that reason
`sbrs_abi_version() -> i32` keeps its signature in every version.
