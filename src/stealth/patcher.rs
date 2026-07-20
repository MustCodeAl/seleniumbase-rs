use crate::error::SeleniumBaseError;
use rand::RngExt;
use regex::bytes::Regex;
use std::fs;
use std::path::Path;

pub fn patch_chromedriver<P: AsRef<Path>>(path: P) -> Result<(), SeleniumBaseError> {
    let mut content = fs::read(path.as_ref()).map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!("Failed to read chromedriver: {}", e))
    })?;

    // Replace window.cdc_... assignments with spaces
    let re1 = Regex::new(r"window\.cdc_[a-zA-Z0-9]{22}_(Array|Promise|Symbol|Object|Proxy|JSON|Window)\s*=\s*window\.(Array|Promise|Symbol|Object|Proxy|JSON|Window);").unwrap();
    content = re1
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            vec![b' '; caps[0].len()]
        })
        .into_owned();

    let re2 = Regex::new(
        r"window\.cdc_[a-zA-Z0-9]{22}_(Array|Promise|Symbol|Object|Proxy|JSON|Window)\s*\|\|",
    )
    .unwrap();
    content = re2
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            vec![b' '; caps[0].len()]
        })
        .into_owned();

    let re3 = Regex::new(r"'\$cdc_[a-zA-Z0-9]{22}_';").unwrap();
    content = re3
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            let rep_len = caps[0].len() - 3;
            let mut rng = rand::rng();
            let ran_len = rng.random_range(6..=rep_len);

            let chars: Vec<u8> = (0..ran_len)
                .map(|_| rng.random_range(b'a'..=b'z'))
                .collect();

            let mut out = Vec::with_capacity(caps[0].len());
            out.push(b'\'');
            out.extend(chars);
            out.extend_from_slice(b"';");
            out.extend(vec![b'\n'; rep_len - ran_len]);
            out
        })
        .into_owned();

    let re4 = Regex::new(r"\{window\.cdc.*?;\}").unwrap();
    content = re4
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            let mut out = b"{console.log(\"chromedriver is undetectable!\")}".to_vec();
            if out.len() < caps[0].len() {
                out.extend(vec![b' '; caps[0].len() - out.len()]);
            } else {
                out.truncate(caps[0].len());
            }
            out
        })
        .into_owned();

    fs::write(path.as_ref(), content).map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!("Failed to write patched chromedriver: {}", e))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_cdc_marker() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chromedriver");
        let marker = b"window.cdc_abcdef1234567890abcdef_Array = window.Array;";
        fs::write(&path, marker).unwrap();

        patch_chromedriver(&path).unwrap();

        let patched = fs::read(&path).unwrap();
        // The original CDC assignment should have been replaced with spaces.
        let original = String::from_utf8_lossy(marker);
        let patched_str = String::from_utf8_lossy(&patched);
        assert_ne!(patched_str, original);
        assert!(!patched_str.contains("cdc_abcdef1234567890abcdef_Array"));
    }
}
