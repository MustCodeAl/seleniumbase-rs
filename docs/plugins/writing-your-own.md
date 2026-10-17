---
title: "Writing Your Own Plugin"
---

# Writing your own plugin

For turning plugins on and attaching a set to a browser, read the
[Browser Plugins guide](../tutorials/plugins.md). This page is about the plugin
itself.

## A plugin

```rust
use seleniumbase_rs_plugin::{async_trait, PageHandle, Plugin, Result};

pub struct Hello;

#[async_trait]
impl Plugin for Hello {
    fn name(&self) -> &str {
        "hello"
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()> {
        page.add_init_script("console.log('hello from a plugin')").await
    }
}
```

Every hook is optional: `before_launch` (change the browser's start-up
settings), `on_browser`, `on_page_created`, `on_close`. A plugin can also
declare JavaScript and CSS `packages` for every tab, its `dependencies` on other
plugins, and `requirements` (run last, a visible window, a Chromium browser, a
browser it starts itself). Errors in the launch and page hooks stop the launch or
the tab instead of being skipped.

## Declaring metadata and capabilities

A plugin can say what it is and what it needs by overriding `metadata()`. The
default is `Metadata::legacy()`, which keeps older plugins working.

```rust
use seleniumbase_rs_plugin::{
    async_trait, Capability, Category, HostScope, Metadata, PageHandle, Plugin, Result,
};

pub struct Hello;

#[async_trait]
impl Plugin for Hello {
    fn name(&self) -> &str {
        "hello"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Environment)
            .summary("Says hello in every page")
            .capability(Capability::PageScript(HostScope::Everywhere))
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()> {
        page.add_init_script("console.log('hello from a plugin')").await
    }
}
```

`Metadata` also takes `version`, `optional` capabilities, `requires`,
`conflicts_with`, `after`, `before`, `phase`, `seal` and `budget`. Without
`phase`, the category sets it (`Category::default_phase`). For a pool,
`per_launch()` can return a fresh instance for each browser.

A set grants capabilities when you add a plugin:

- `PluginSet::new().with(p)` grants what `p` declares, except Critical
  capabilities.
- `with_trusted(p, &[Capability])` also grants the Critical ones you name.
- `with_grants(p, set)` grants exactly `set`.
- `PluginSet::strict()` refuses plugins that declare no metadata. The command
  line and the MCP server use strict sets.

To let users enable a plugin by name with options, implement `Configurable`
(`NAME`, `describe()`, `configure(&Value)`) and add
`Descriptor::of::<P>()` to a `Registry`. The design is on the
[architecture page](architecture.md).

<Note>

For a native Rust plugin, capabilities are a declared contract, not a sandbox.
Only [WebAssembly](wasm.md) plugins are sandboxed.

</Note>

## Write your own

<Steps>

<Step title="Copy the template">

Copy [`template/`](https://github.com/MustCodeAl/seleniumbase-rs-plugins/tree/main/template) and rename the crate to
`seleniumbase-rs-plugin-<name>`.

</Step>

<Step title="Test it without a browser">

Test it with `seleniumbase-rs-plugin-testing`: `RecordingPage` and
`RecordingBrowser` record every command and script your hooks send.

</Step>

<Step title="Publish it">

Publish it on GitHub with the topic `seleniumbase-rs-plugin` so people find it.

</Step>

</Steps>

Not a Rust programmer? Write the plugin in any language that compiles to
WebAssembly and load it with the [`wasm`](wasm.md) host: the module only
describes the plugin in JSON (a name, browser switches, page scripts, URLs to
block) and runs in a sandbox. The ABI is on the [wasm page](wasm.md).

<Info>

A plugin depends only on `seleniumbase-rs-plugin`, never on seleniumbase-rs, so
it keeps working as seleniumbase-rs changes.

</Info>

## The template

The template is a whole plugin crate with a test.

<Tree>
  <Tree.Folder name="template" defaultOpen>
    <Tree.Folder name="src" defaultOpen>
      <Tree.File name="lib.rs" />
    </Tree.Folder>
    <Tree.File name="Cargo.toml" />
  </Tree.Folder>
</Tree>

<CodeGroup>

```toml Cargo.toml
[package]
name = "seleniumbase-rs-plugin-hello"
description = "A seleniumbase-rs plugin (rename me)"
version = "0.1.0"
edition = "2024"
license = "MIT"

[dependencies]
seleniumbase-rs-plugin = { git = "https://github.com/MustCodeAl/seleniumbase-rs-plugins" }

[dev-dependencies]
seleniumbase-rs-plugin-testing = { git = "https://github.com/MustCodeAl/seleniumbase-rs-plugins" }
```

```rust src/lib.rs
//! A seleniumbase-rs plugin. Rename the crate to `seleniumbase-rs-plugin-<name>`.

use seleniumbase_rs_plugin::{async_trait, PageHandle, Plugin, Result};

/// Says hello in the console of every page.
pub struct Hello;

#[async_trait]
impl Plugin for Hello {
    fn name(&self) -> &str {
        "hello"
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()> {
        page.add_init_script("console.log('hello from a plugin')").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seleniumbase_rs_plugin::PluginSet;
    use seleniumbase_rs_plugin_testing::{block_on, Call, RecordingPage};

    #[test]
    fn every_page_says_hello() {
        let page = RecordingPage::new();
        block_on(PluginSet::new().with(Hello).unwrap().on_page_created(&page)).unwrap();
        assert_eq!(page.calls(), [Call::InitScript("console.log('hello from a plugin')".into())]);
    }
}
```

</CodeGroup>

<Warning>

A plugin is code that runs with your program's rights and sees every page,
cookie and password the browser handles. Review a plugin before you add it,
as you would any dependency.

</Warning>
