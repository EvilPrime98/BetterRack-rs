//! Keeps the process alive while downloads run: starts `KeepAliveService` (a foreground service,
//! `java/app/.../KeepAliveService.java`) when the first job is active and stops it after the last.

use crate::backend::Backend;
use jni::objects::{JObject, JValue};
use serde_json::Value;
use std::time::Duration;

const SERVICE: &str = "dev.betterrack.android.KeepAliveService";

/// Jobs that still need the process: `running` or `queued`.
pub fn active_jobs(payload: &Value) -> usize {
    payload["jobs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|j| matches!(j["state"].as_str(), Some("running" | "queued")))
        .count()
}

/// The notification text for `n` active jobs.
pub fn describe(n: usize) -> String {
    if n == 1 { "Downloading 1 comic".into() } else { format!("Downloading {n} comics") }
}

fn service_intent<'l>(env: &mut jni::JNIEnv<'l>, ctx: &JObject) -> jni::errors::Result<JObject<'l>> {
    let package = env.call_method(ctx, "getPackageName", "()Ljava/lang/String;", &[])?.l()?;
    let class = env.new_string(SERVICE)?;
    let intent = env.new_object("android/content/Intent", "()V", &[])?;
    env.call_method(
        &intent,
        "setClassName",
        "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;",
        &[JValue::Object(&package), JValue::Object(&class)],
    )?;
    Ok(intent)
}

/// Shows the service with `text`, or stops it when `text` is `None`.
fn set_service(text: Option<&str>) -> std::io::Result<()> {
    crate::saf::with_env(|env| {
        // SAFETY: `ndk_context` holds a global reference to the Application for the whole process.
        let ctx = unsafe { JObject::from_raw(ndk_context::android_context().context().cast()) };
        let intent = service_intent(env, &ctx)?;
        match text {
            Some(text) => {
                let (key, value) = (env.new_string("text")?, env.new_string(text)?);
                env.call_method(
                    &intent,
                    "putExtra",
                    "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;",
                    &[JValue::Object(&key), JValue::Object(&value)],
                )?;
                env.call_method(
                    &ctx,
                    "startForegroundService",
                    "(Landroid/content/Intent;)Landroid/content/ComponentName;",
                    &[JValue::Object(&intent)],
                )?;
            }
            None => {
                env.call_method(&ctx, "stopService", "(Landroid/content/Intent;)Z", &[JValue::Object(&intent)])?;
            }
        }
        Ok(())
    })
}

/// Polls the on-device download jobs and mirrors "any active" onto the service. Skipped while a
/// remote server is linked: its downloads run on that server, not on this device.
pub async fn watch(backend: Backend) {
    let mut shown = 0usize;
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let n = if backend.is_remote() {
            0
        } else {
            backend.get_json("/api/downloads/jobs").await.map(|p| active_jobs(&p)).unwrap_or(shown)
        };
        if n == shown {
            continue;
        }
        let result = set_service((n > 0).then(|| describe(n)).as_deref());
        match result {
            Ok(()) => shown = n,
            Err(e) => eprintln!("keep-alive service: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn counts_only_unfinished_jobs() {
        let p = json!({"jobs": [
            {"state": "running"}, {"state": "queued"}, {"state": "done"}, {"state": "error"}
        ]});
        assert_eq!(active_jobs(&p), 2);
        assert_eq!(active_jobs(&json!({})), 0);
    }
}
