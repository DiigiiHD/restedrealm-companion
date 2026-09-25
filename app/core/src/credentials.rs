//! The upload-only device credential lives in Windows Credential Manager, never
//! in the queue or a save. Target name and blob format match the Python
//! companion, so a PC paired during the pilot stays paired.

use crate::{Error, Result};
use std::sync::Mutex;

pub const TARGET: &str = "RestedRealmCollector/restedrealm.com";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    pub device_id: String,
    pub token: String,
}

impl Credential {
    fn to_blob(&self) -> Vec<u8> {
        serde_json::json!({"deviceId": self.device_id, "token": self.token}).to_string().into_bytes()
    }

    fn from_blob(blob: &[u8]) -> Result<Credential> {
        let invalid = || Error::Refused("Stored RestedRealm credential is invalid".into());
        let value: serde_json::Value = serde_json::from_slice(blob).map_err(|_| invalid())?;
        match (value.get("deviceId").and_then(|v| v.as_str()), value.get("token").and_then(|v| v.as_str())) {
            (Some(device_id), Some(token)) => Ok(Credential { device_id: device_id.into(), token: token.into() }),
            _ => Err(invalid()),
        }
    }
}

pub trait CredentialStore {
    fn load(&self) -> Result<Option<Credential>>;
    fn save(&self, credential: &Credential) -> Result<()>;
    fn forget(&self) -> Result<()>;
}

/// For tests and non-Windows development builds.
#[derive(Default)]
pub struct MemoryStore(Mutex<Option<Vec<u8>>>);

impl CredentialStore for MemoryStore {
    fn load(&self) -> Result<Option<Credential>> {
        self.0.lock().unwrap().as_deref().map(Credential::from_blob).transpose()
    }
    fn save(&self, credential: &Credential) -> Result<()> {
        *self.0.lock().unwrap() = Some(credential.to_blob());
        Ok(())
    }
    fn forget(&self) -> Result<()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(windows)]
pub use windows_store::WindowsStore;

#[cfg(windows)]
mod windows_store {
    use super::*;
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn last_error() -> Error {
        Error::Io(std::io::Error::last_os_error())
    }

    /// Windows Credential Manager, generic credential, this PC only.
    pub struct WindowsStore {
        pub target: String,
    }

    impl Default for WindowsStore {
        fn default() -> WindowsStore {
            WindowsStore { target: TARGET.into() }
        }
    }

    impl CredentialStore for WindowsStore {
        fn load(&self) -> Result<Option<Credential>> {
            let target = wide(&self.target);
            let mut pointer: *mut CREDENTIALW = std::ptr::null_mut();
            // SAFETY: the target is NUL-terminated and the pointer is freed below.
            let ok = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut pointer) };
            if ok == 0 {
                return if unsafe { GetLastError() } == ERROR_NOT_FOUND { Ok(None) } else { Err(last_error()) };
            }
            // SAFETY: CredReadW succeeded, so the pointer and its blob are valid until CredFree.
            let blob = unsafe {
                let credential = &*pointer;
                std::slice::from_raw_parts(credential.CredentialBlob, credential.CredentialBlobSize as usize).to_vec()
            };
            unsafe { CredFree(pointer.cast()) };
            Credential::from_blob(&blob).map(Some)
        }

        fn save(&self, credential: &Credential) -> Result<()> {
            let mut target = wide(&self.target);
            let mut comment = wide("RestedRealm Companion upload-only credential");
            let mut user = wide("RestedRealm Companion");
            let mut blob = credential.to_blob();
            // SAFETY: an all-zero CREDENTIALW is a valid starting value.
            let mut record: CREDENTIALW = unsafe { std::mem::zeroed() };
            record.Type = CRED_TYPE_GENERIC;
            record.TargetName = target.as_mut_ptr();
            record.Comment = comment.as_mut_ptr();
            record.CredentialBlobSize = blob.len() as u32;
            record.CredentialBlob = blob.as_mut_ptr();
            record.Persist = CRED_PERSIST_LOCAL_MACHINE;
            record.UserName = user.as_mut_ptr();
            // SAFETY: every pointer refers to a buffer that outlives the call.
            if unsafe { CredWriteW(&record, 0) } == 0 {
                return Err(last_error());
            }
            Ok(())
        }

        fn forget(&self) -> Result<()> {
            let target = wide(&self.target);
            // SAFETY: the target is NUL-terminated.
            if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0
                && unsafe { GetLastError() } != ERROR_NOT_FOUND
            {
                return Err(last_error());
            }
            Ok(())
        }
    }
}

/// The store the app should use on this platform.
pub fn platform_store() -> Box<dyn CredentialStore + Send + Sync> {
    #[cfg(windows)]
    {
        Box::new(WindowsStore::default())
    }
    #[cfg(not(windows))]
    {
        Box::new(MemoryStore::default())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn windows_credential_manager_round_trip() {
        // A separate target, so a real pairing on this PC is never touched.
        let store = WindowsStore { target: "RestedRealmCompanion/test".into() };
        let credential = Credential { device_id: "dev".into(), token: format!("rrc_{}", "t".repeat(43)) };
        store.forget().unwrap();
        assert_eq!(store.load().unwrap(), None);
        store.save(&credential).unwrap();
        assert_eq!(store.load().unwrap(), Some(credential));
        store.forget().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }
}
