//! Helpers shared by both engines that are not themselves a browser: colour,
//! the debug prompt, encryption and obfuscation, shutdown, and the test id.

pub mod color;
pub mod debug_prompt;
pub mod encryption;
pub mod obfuscate;
pub(crate) mod orphan_guard;
pub(crate) mod poison;
pub mod shutdown;
pub mod test_id;
pub(crate) mod tls;
