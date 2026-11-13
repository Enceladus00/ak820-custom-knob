//! The snippet list on disk: `%APPDATA%\ak820-custom-knob\snippets.dat`,
//! encrypted with Windows DPAPI for the signed-in user. Only that user on
//! this PC can read it; copied anywhere else it is unreadable noise.
//!
//! Layout: the 8-byte magic `KNOBSNIP`, a version byte (1), then the DPAPI
//! blob of the list as UTF-8 TOML.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use knob_core::snippets::SnippetStore;
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};

const MAGIC: &[u8] = b"KNOBSNIP";
const VERSION: u8 = 1;
/// Extra secret mixed into the encryption, so other programs using DPAPI for
/// the same user can't decrypt the file by accident.
const ENTROPY: &[u8] = b"ak820-custom-knob snippets v1";

pub fn path() -> PathBuf {
    crate::paths::config_file().with_file_name("snippets.dat")
}

/// The saved list. A missing file is an empty list; a damaged one, or one
/// from another user or PC, is an error (and is left untouched).
pub fn load() -> Result<SnippetStore, String> {
    load_from(&path())
}

pub fn load_from(path: &Path) -> Result<SnippetStore, String> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(SnippetStore::default()),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let blob = bytes
        .strip_prefix(MAGIC)
        .and_then(|rest| rest.split_first())
        .filter(|(v, _)| **v == VERSION)
        .map(|(_, blob)| blob)
        .ok_or("the snippets file is not in a known format")?;
    let mut plain = unprotect(blob).map_err(|_| {
        "the snippets file can't be decrypted: it belongs to another Windows user or PC".to_string()
    })?;
    let result = std::str::from_utf8(&plain)
        .map_err(|_| "the snippets file is damaged".to_string())
        .and_then(|text| {
            toml::from_str(text).map_err(|e| format!("the snippets file is damaged: {e}"))
        });
    plain.fill(0);
    result
}

/// Encrypts and saves the list, keeping the previous file as `.bak` (also
/// encrypted). Refuses a list with problems.
pub fn save(store: &SnippetStore) -> Result<(), String> {
    save_to(&path(), store)
}

pub fn save_to(path: &Path, store: &SnippetStore) -> Result<(), String> {
    let problems = store.problems();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let mut text = toml::to_string(store)
        .map_err(|e| e.to_string())?
        .into_bytes();
    let blob = protect(&text);
    text.fill(0);
    let blob = blob.map_err(|e| format!("cannot encrypt the snippets: {e}"))?;
    let mut bytes = MAGIC.to_vec();
    bytes.push(VERSION);
    bytes.extend_from_slice(&blob);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("dat.tmp");
    fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    if path.exists() {
        let _ = fs::copy(path, path.with_extension("dat.bak"));
    }
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    }
}

/// Copies DPAPI's output and frees it (wiping the plain text first).
unsafe fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    unsafe {
        let slice = std::slice::from_raw_parts_mut(out.pbData, out.cbData as usize);
        let v = slice.to_vec();
        slice.fill(0);
        let _ = LocalFree(Some(HLOCAL(out.pbData as _)));
        v
    }
}

fn protect(data: &[u8]) -> windows::core::Result<Vec<u8>> {
    let entropy = blob(ENTROPY);
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &blob(data),
            None,
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )?;
        Ok(take(out))
    }
}

fn unprotect(data: &[u8]) -> windows::core::Result<Vec<u8>> {
    let entropy = blob(ENTROPY);
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &blob(data),
            None,
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )?;
        Ok(take(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knob_core::snippets::Snippet;

    #[test]
    fn saves_encrypted_and_loads_back() {
        let dir = std::env::temp_dir().join(format!("knob-snip-{}", std::process::id()));
        let file = dir.join("snippets.dat");
        assert_eq!(load_from(&file).unwrap(), SnippetStore::default());
        let store = SnippetStore {
            snippets: vec![Snippet {
                trigger: ";mail".into(),
                text: "secret@example.com".into(),
                enabled: true,
                secret: true,
                apps: vec![],
            }],
            ..SnippetStore::default()
        };
        save_to(&file, &store).unwrap();
        let raw = fs::read(&file).unwrap();
        assert!(raw.starts_with(MAGIC));
        let needle = b"secret@example.com";
        assert!(
            !raw.windows(needle.len()).any(|w| w == needle),
            "stored in plain text"
        );
        assert_eq!(load_from(&file).unwrap(), store);

        // Tampering is detected, not silently accepted.
        let mut bad = raw.clone();
        let last = bad.len() - 1;
        bad[last] ^= 0xFF;
        fs::write(&file, &bad).unwrap();
        assert!(load_from(&file).is_err());

        let mut invalid = store.clone();
        invalid.snippets[0].trigger = ";".into();
        assert!(save_to(&file, &invalid).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
