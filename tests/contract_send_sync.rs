//! Compile-time pins: the public handles are `Send + Sync`, so they cross
//! `tokio::spawn` and thread boundaries (C-SEND-SYNC, M-TYPES-SEND), and the
//! two main types print through `Debug` (C-DEBUG).

use std::fmt::Debug;

fn send_sync<T: Send + Sync>() {}

fn debug<T: Debug>() {}

#[test]
fn the_public_handles_are_send_and_sync() {
    send_sync::<seleniumbase_rs::BaseCase>();
    send_sync::<seleniumbase_rs::BrowserSession>();
    send_sync::<seleniumbase_rs::sb_cdp::Browser>();
    send_sync::<seleniumbase_rs::sb_cdp::Page>();
    send_sync::<seleniumbase_rs::sb_cdp::Locator>();
    send_sync::<seleniumbase_rs::sb_cdp::BrowserPool>();
    send_sync::<seleniumbase_rs::sb_cdp::Lease>();
    send_sync::<seleniumbase_rs::sb_cdp::Cookies>();
    send_sync::<seleniumbase_rs::api::run_settings::RunSettings>();
}

#[cfg(feature = "testing")]
#[test]
fn the_fixtures_are_send_and_sync() {
    send_sync::<seleniumbase_rs::testing::Sb>();
    send_sync::<seleniumbase_rs::testing::SbCdp>();
}

#[test]
fn the_main_types_have_debug() {
    debug::<seleniumbase_rs::BaseCase>();
    debug::<seleniumbase_rs::BrowserSession>();
}

#[test]
fn the_choice_enums_are_map_keys() {
    let mut seen = std::collections::HashSet::new();
    seen.insert(seleniumbase_rs::Browser::default());
    seen.insert(seleniumbase_rs::Browser::default());
    assert_eq!(seen.len(), 1);
    let mut modes = std::collections::HashSet::new();
    modes.insert(seleniumbase_rs::DriverMode::default());
    assert_eq!(modes.len(), 1);
}
