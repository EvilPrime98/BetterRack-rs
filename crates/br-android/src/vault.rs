//! The server API key, sealed by an Android Keystore key (`java/.../KeyVault.java`) and stored as
//! `<data>/server_key.bin`. Only ciphertext touches the disk.

use crate::saf::{dex_class, with_env};
use jni::objects::{JByteArray, JClass, JValue};
use jni::sys::jclass;
use std::io;
use std::path::{Path, PathBuf};

const VAULT_CLASS: &str = "dev.betterrack.android.KeyVault";

fn file(data_dir: &Path) -> PathBuf {
    data_dir.join("server_key.bin")
}

fn call(method: &str, input: &[u8]) -> io::Result<Vec<u8>> {
    with_env(|env| {
        let class = dex_class(env, VAULT_CLASS)?;
        // SAFETY: `loadClass` returned a `java.lang.Class`.
        let class = unsafe { JClass::from_raw(class.as_raw() as jclass) };
        let arg = env.byte_array_from_slice(input)?;
        let out = env
            .call_static_method(&class, method, "([B)[B", &[JValue::Object(&arg)])?
            .l()?;
        env.convert_byte_array(JByteArray::from(out))
    })
}

pub fn save(data_dir: &Path, key: &str) -> io::Result<()> {
    if key.is_empty() {
        return clear(data_dir);
    }
    std::fs::write(file(data_dir), call("encrypt", key.as_bytes())?)
}

pub fn load(data_dir: &Path) -> Option<String> {
    let sealed = std::fs::read(file(data_dir)).ok()?;
    let plain = call("decrypt", &sealed).ok()?;
    String::from_utf8(plain).ok().filter(|k| !k.is_empty())
}

pub fn clear(data_dir: &Path) -> io::Result<()> {
    match std::fs::remove_file(file(data_dir)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}
