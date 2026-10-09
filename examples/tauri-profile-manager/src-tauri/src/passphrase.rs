//! Where the passphrase of the profile vault comes from.
//!
//! There are two sources, tried in this order:
//!
//! 1. The `SB_PROFILE_PASSPHRASE` environment variable, when it is set. This is
//!    for headless machines and CI, where there is no keychain to ask.
//! 2. The operating system's keychain. The first time the app runs, a random
//!    256-bit passphrase is generated and stored there; after that it is read
//!    back, and nobody has to remember or type anything.
//!
//! A passphrase is never logged, printed, or included in an error message.
//!
//! The choice of source is a pure function ([`resolve_passphrase`]) over a
//! small [`SecretStore`] trait, so every branch is tested without touching a
//! real keychain.

use std::fmt;

/// The environment variable that supplies the passphrase directly.
pub const PASSPHRASE_ENV: &str = "SB_PROFILE_PASSPHRASE";

/// The keychain item that holds the generated passphrase.
const KEYCHAIN_SERVICE: &str = "seleniumbase-rs profile manager";
const KEYCHAIN_ACCOUNT: &str = "profile vault passphrase";

/// A vault passphrase.
///
/// It prints as `Passphrase(..)` and has no `Display`, so it cannot end up in
/// a log line by accident. Read it with [`expose`](Self::expose).
pub struct Passphrase(String);

impl Passphrase {
    /// Wraps a passphrase.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The passphrase itself. Hand it to the vault and nothing else.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Passphrase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Passphrase(..)")
    }
}

/// Where a passphrase was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassphraseSource {
    /// `SB_PROFILE_PASSPHRASE`.
    Environment,
    /// Read from the keychain, where an earlier run stored it.
    Keychain,
    /// Generated just now and stored in the keychain.
    KeychainNew,
}

impl PassphraseSource {
    /// A short name for the window to show: `environment` or `keychain`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Keychain | Self::KeychainNew => "keychain",
        }
    }
}

/// Why no passphrase could be obtained.
#[derive(Debug, PartialEq, Eq)]
pub enum PassphraseError {
    /// `SB_PROFILE_PASSPHRASE` is set to nothing.
    EnvEmpty,
    /// `SB_PROFILE_PASSPHRASE` is not valid Unicode.
    EnvNotUnicode,
    /// A vault exists but the keychain has no passphrase for it, and the
    /// environment variable is not set. A new passphrase would not open it, so
    /// none is generated.
    KeychainMissing,
    /// The keychain could not be read or written.
    Keychain(String),
    /// The operating system could not supply random bytes.
    Random,
}

impl fmt::Display for PassphraseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnvEmpty => write!(f, "{PASSPHRASE_ENV} is set but empty"),
            Self::EnvNotUnicode => write!(f, "{PASSPHRASE_ENV} is not valid Unicode"),
            Self::KeychainMissing => write!(
                f,
                "the profile vault exists, but its passphrase is not in the system keychain; \
                 set {PASSPHRASE_ENV} to the passphrase it was created with, or restore the \
                 keychain item"
            ),
            Self::Keychain(reason) => write!(
                f,
                "the system keychain failed: {reason}; set {PASSPHRASE_ENV} to use a passphrase \
                 instead"
            ),
            Self::Random => f.write_str("the operating system could not supply random bytes"),
        }
    }
}

impl std::error::Error for PassphraseError {}

/// A keychain failure, described without any secret.
#[derive(Debug, PartialEq, Eq)]
pub struct SecretStoreError(pub String);

/// One secret held by the platform: the vault passphrase.
pub trait SecretStore {
    /// The stored secret, or `None` when nothing has been stored yet.
    ///
    /// # Errors
    ///
    /// Fails when the store cannot be read. That is different from `None`: it
    /// must not be mistaken for "nothing stored".
    fn read(&self) -> Result<Option<String>, SecretStoreError>;

    /// Stores `secret`, replacing any earlier one.
    ///
    /// # Errors
    ///
    /// Fails when the store cannot be written.
    fn write(&self, secret: &str) -> Result<(), SecretStoreError>;
}

/// The operating system's keychain: Keychain Services on macOS, Credential
/// Manager on Windows, the Secret Service elsewhere.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyringSecretStore;

impl KeyringSecretStore {
    fn entry() -> Result<keyring::Entry, SecretStoreError> {
        keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT).map_err(describe)
    }
}

impl SecretStore for KeyringSecretStore {
    fn read(&self) -> Result<Option<String>, SecretStoreError> {
        match Self::entry()?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(describe(error)),
        }
    }

    fn write(&self, secret: &str) -> Result<(), SecretStoreError> {
        Self::entry()?.set_password(secret).map_err(describe)
    }
}

/// Words a keychain error without echoing any data it carries.
fn describe(error: keyring::Error) -> SecretStoreError {
    SecretStoreError(match error {
        keyring::Error::BadEncoding(_) => "the stored passphrase is not valid UTF-8".to_owned(),
        other => other.to_string(),
    })
}

/// Reads `SB_PROFILE_PASSPHRASE` from the process environment.
///
/// # Errors
///
/// Returns an error when it is set but not valid Unicode.
pub fn passphrase_from_env() -> Result<Option<String>, PassphraseError> {
    match std::env::var_os(PASSPHRASE_ENV) {
        None => Ok(None),
        Some(value) => value
            .into_string()
            .map(Some)
            .map_err(|_| PassphraseError::EnvNotUnicode),
    }
}

/// Decides where the vault passphrase comes from.
///
/// `from_env` is the value of `SB_PROFILE_PASSPHRASE`, if it is set. A
/// passphrase is generated only when `vault_exists` is false: for an existing
/// vault a fresh passphrase can only lock the user out, so a missing keychain
/// item is reported instead.
///
/// # Errors
///
/// See [`PassphraseError`].
pub fn resolve_passphrase(
    from_env: Option<&str>,
    keychain: &dyn SecretStore,
    vault_exists: bool,
) -> Result<(Passphrase, PassphraseSource), PassphraseError> {
    if let Some(value) = from_env {
        if value.is_empty() {
            return Err(PassphraseError::EnvEmpty);
        }
        return Ok((Passphrase::new(value), PassphraseSource::Environment));
    }

    let stored = keychain
        .read()
        .map_err(|error| PassphraseError::Keychain(error.0))?
        .filter(|secret| !secret.is_empty());
    if let Some(secret) = stored {
        return Ok((Passphrase::new(secret), PassphraseSource::Keychain));
    }
    if vault_exists {
        return Err(PassphraseError::KeychainMissing);
    }

    let secret = random_passphrase()?;
    keychain
        .write(&secret)
        .map_err(|error| PassphraseError::Keychain(error.0))?;
    // Some stores accept a write and lose it. Reading it back now costs
    // nothing, and finding out after the vault exists would cost the vault.
    let kept = keychain
        .read()
        .map_err(|error| PassphraseError::Keychain(error.0))?;
    if kept.as_deref() != Some(secret.as_str()) {
        return Err(PassphraseError::Keychain(
            "the passphrase was not kept after it was stored".to_owned(),
        ));
    }
    Ok((Passphrase::new(secret), PassphraseSource::KeychainNew))
}

/// 256 random bits from the operating system, as 64 hexadecimal characters.
fn random_passphrase() -> Result<String, PassphraseError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| PassphraseError::Random)?;
    Ok(hex(&bytes))
}

/// Lowercase hexadecimal.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            // Writing to a String cannot fail.
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// An in-memory [`SecretStore`] for tests.
#[cfg(test)]
pub(crate) mod memory {
    use std::sync::Mutex;

    use super::{SecretStore, SecretStoreError};

    /// How a [`MemorySecretStore`] misbehaves.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Fault {
        None,
        /// Every read fails.
        Read,
        /// Every write fails.
        Write,
        /// Writes succeed and are then forgotten.
        ForgetWrites,
    }

    #[derive(Debug)]
    pub(crate) struct MemorySecretStore {
        secret: Mutex<Option<String>>,
        fault: Fault,
        writes: Mutex<u32>,
    }

    impl MemorySecretStore {
        pub(crate) fn empty() -> Self {
            Self::with(None, Fault::None)
        }

        pub(crate) fn holding(secret: &str) -> Self {
            Self::with(Some(secret.to_owned()), Fault::None)
        }

        pub(crate) fn failing(fault: Fault) -> Self {
            Self::with(None, fault)
        }

        fn with(secret: Option<String>, fault: Fault) -> Self {
            Self {
                secret: Mutex::new(secret),
                fault,
                writes: Mutex::new(0),
            }
        }

        pub(crate) fn current(&self) -> Option<String> {
            self.secret.lock().expect("test lock").clone()
        }

        pub(crate) fn writes(&self) -> u32 {
            *self.writes.lock().expect("test lock")
        }
    }

    impl SecretStore for MemorySecretStore {
        fn read(&self) -> Result<Option<String>, SecretStoreError> {
            if self.fault == Fault::Read {
                return Err(SecretStoreError("the keychain is locked".to_owned()));
            }
            Ok(self.current())
        }

        fn write(&self, secret: &str) -> Result<(), SecretStoreError> {
            if self.fault == Fault::Write {
                return Err(SecretStoreError("the keychain is read-only".to_owned()));
            }
            *self.writes.lock().expect("test lock") += 1;
            if self.fault != Fault::ForgetWrites {
                *self.secret.lock().expect("test lock") = Some(secret.to_owned());
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::memory::{Fault, MemorySecretStore};
    use super::*;

    const FAKE: &str = "fake-passphrase-for-tests";

    #[test]
    fn the_environment_variable_wins_over_the_keychain() {
        let keychain = MemorySecretStore::holding("from-keychain");

        let (passphrase, source) = resolve_passphrase(Some(FAKE), &keychain, true).unwrap();

        assert_eq!(passphrase.expose(), FAKE);
        assert_eq!(source, PassphraseSource::Environment);
        assert_eq!(keychain.writes(), 0, "the keychain must be left alone");
    }

    #[test]
    fn the_environment_variable_is_used_without_consulting_a_broken_keychain() {
        let keychain = MemorySecretStore::failing(Fault::Read);

        let (_, source) = resolve_passphrase(Some(FAKE), &keychain, false).unwrap();

        assert_eq!(source, PassphraseSource::Environment);
    }

    #[test]
    fn an_empty_environment_variable_is_an_error_not_a_silent_fallback() {
        let keychain = MemorySecretStore::holding("from-keychain");

        let error = resolve_passphrase(Some(""), &keychain, true).unwrap_err();

        assert_eq!(error, PassphraseError::EnvEmpty);
    }

    #[test]
    fn a_stored_passphrase_is_read_back_from_the_keychain() {
        let keychain = MemorySecretStore::holding("from-keychain");

        let (passphrase, source) = resolve_passphrase(None, &keychain, true).unwrap();

        assert_eq!(passphrase.expose(), "from-keychain");
        assert_eq!(source, PassphraseSource::Keychain);
        assert_eq!(keychain.writes(), 0);
    }

    #[test]
    fn the_first_run_generates_a_256_bit_passphrase_and_keeps_it() {
        let keychain = MemorySecretStore::empty();

        let (passphrase, source) = resolve_passphrase(None, &keychain, false).unwrap();

        assert_eq!(source, PassphraseSource::KeychainNew);
        let secret = passphrase.expose();
        assert_eq!(secret.len(), 64, "256 bits as hex");
        assert!(secret.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(keychain.current().as_deref(), Some(secret));
        assert_eq!(keychain.writes(), 1);
    }

    #[test]
    fn generated_passphrases_differ() {
        let a = resolve_passphrase(None, &MemorySecretStore::empty(), false).unwrap();
        let b = resolve_passphrase(None, &MemorySecretStore::empty(), false).unwrap();

        assert_ne!(a.0.expose(), b.0.expose());
    }

    #[test]
    fn an_existing_vault_with_no_keychain_item_is_reported_not_regenerated() {
        let keychain = MemorySecretStore::empty();

        let error = resolve_passphrase(None, &keychain, true).unwrap_err();

        assert_eq!(error, PassphraseError::KeychainMissing);
        assert_eq!(
            keychain.writes(),
            0,
            "a new passphrase would only lock the user out"
        );
    }

    #[test]
    fn an_empty_keychain_item_counts_as_missing() {
        let keychain = MemorySecretStore::holding("");

        assert_eq!(
            resolve_passphrase(None, &keychain, true).unwrap_err(),
            PassphraseError::KeychainMissing
        );
        // With no vault yet, a usable passphrase replaces it.
        let (passphrase, _) = resolve_passphrase(None, &keychain, false).unwrap();
        assert_eq!(passphrase.expose().len(), 64);
    }

    #[test]
    fn a_keychain_that_cannot_be_read_is_not_treated_as_empty() {
        // Treating a failed read as "nothing stored" would generate a new
        // passphrase and overwrite the one that opens the vault.
        let keychain = MemorySecretStore::failing(Fault::Read);

        let error = resolve_passphrase(None, &keychain, false).unwrap_err();

        assert!(matches!(error, PassphraseError::Keychain(_)), "{error}");
        assert_eq!(keychain.writes(), 0);
    }

    #[test]
    fn a_keychain_that_cannot_be_written_stops_the_first_run() {
        let keychain = MemorySecretStore::failing(Fault::Write);

        let error = resolve_passphrase(None, &keychain, false).unwrap_err();

        assert!(matches!(error, PassphraseError::Keychain(_)), "{error}");
    }

    #[test]
    fn a_keychain_that_forgets_the_write_stops_the_first_run() {
        let keychain = MemorySecretStore::failing(Fault::ForgetWrites);

        let error = resolve_passphrase(None, &keychain, false).unwrap_err();

        assert!(matches!(error, PassphraseError::Keychain(_)), "{error}");
    }

    #[test]
    fn a_passphrase_never_prints_itself() {
        let passphrase = Passphrase::new(FAKE);

        assert!(!format!("{passphrase:?}").contains(FAKE));
    }

    #[test]
    fn errors_never_contain_the_passphrase() {
        let keychain = MemorySecretStore::holding(FAKE);
        let errors = [
            resolve_passphrase(Some(""), &keychain, true).unwrap_err(),
            resolve_passphrase(None, &MemorySecretStore::empty(), true).unwrap_err(),
            resolve_passphrase(None, &MemorySecretStore::failing(Fault::Read), true).unwrap_err(),
        ];

        for error in errors {
            assert!(!error.to_string().contains(FAKE), "{error}");
            assert!(!format!("{error:?}").contains(FAKE), "{error:?}");
        }
    }

    #[test]
    fn hex_is_lowercase_and_fixed_width() {
        assert_eq!(hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
        assert_eq!(hex(&[]), "");
    }
}
