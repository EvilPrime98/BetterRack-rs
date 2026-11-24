use axum::body::Body;
use http::Request;
use http_body_util::BodyExt;
use slint::{ModelRc, SharedString, VecModel};
use std::sync::Arc;
use tower::ServiceExt;

slint::include_modules!();

const FIXTURES: [(&str, &[u8]); 2] = [
    ("Spike One.cb7", include_bytes!("../fixtures/7z.cb7")),
    ("Spike Two.cbr", include_bytes!("../fixtures/rar5.cbr")),
];

async fn get(router: &axum::Router, uri: &str) -> Result<String, String> {
    let res = router
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let bytes = res.into_body().collect().await.map_err(|e| e.to_string())?.to_bytes();
    Ok(format!("{status} {}", String::from_utf8_lossy(&bytes)))
}

async fn boot(base: std::path::PathBuf) -> Result<(String, Vec<String>), String> {
    let lib = base.join("library");
    std::fs::create_dir_all(&lib).map_err(|e| e.to_string())?;
    for (n, b) in FIXTURES {
        std::fs::write(lib.join(n), b).map_err(|e| e.to_string())?;
    }
    let config = br_core::config::Config {
        port: 0,
        bind_addr: "127.0.0.1".into(),
        api_key: None,
        log_level: "info".into(),
        seven_zip_path: None,
        data_dir: base.join("data"),
    };
    std::fs::create_dir_all(&config.data_dir).map_err(|e| e.to_string())?;
    let state = br_server::state::AppState::open(config).map_err(|e| e.to_string())?;
    let router = br_server::app_with_web(state.clone(), None);
    let health = get(&router, "/healthz").await?;
    let body = serde_json::json!({ "path": lib.to_string_lossy() }).to_string();
    let res = router
        .clone()
        .oneshot(
            Request::post("/api/settings/library-folder")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .map_err(|e| e.to_string())?;
    let _ = res.status();
    let mut list = String::new();
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        list = get(&router, "/api/library").await?;
        if list.contains("Spike") {
            break;
        }
    }
    let json: serde_json::Value = serde_json::from_str(&list[list.find('{').unwrap_or(0)..])
        .unwrap_or_default();
    let mut names = Vec::new();
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            serde_json::Value::Object(o) => {
                if let Some(n) = o.get("name").and_then(|n| n.as_str()) {
                    out.push(n.to_string());
                }
                o.values().for_each(|x| walk(x, out));
            }
            _ => {}
        }
    }
    walk(&json, &mut names);
    Ok((format!("{}", &health[..6]), names))
}

#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    let base = app.internal_data_path().expect("data path");
    slint::android::init(app).unwrap();
    let ui = App::new().unwrap();
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        let t0 = std::time::Instant::now();
        let result = rt.block_on(boot(base));
        let ms = t0.elapsed().as_millis();
        let _ = weak.upgrade_in_event_loop(move |ui| match result {
            Ok((status, names)) => {
                ui.set_status(SharedString::from(format!("{} names | boot {ms} ms | {status}", names.len())));
                let model: Vec<SharedString> = names.into_iter().map(Into::into).collect();
                ui.set_items(ModelRc::new(VecModel::from(model)));
            }
            Err(e) => ui.set_status(SharedString::from(format!("ERROR: {e}"))),
        });
        std::mem::forget(rt);
    });
    ui.run().unwrap();
    let _ = Arc::new(());
}

/// Skia's prebuilt objects reference libc++'s `std::__hash_memory`, which NDK 27's libc++ lacks.
#[unsafe(no_mangle)]
pub extern "C" fn _ZNSt6__ndk113__hash_memoryEPKvm(p: *const u8, n: usize) -> usize {
    let bytes = unsafe { std::slice::from_raw_parts(p, n) };
    let mut h: usize = 0xcbf29ce484222325;
    for b in bytes {
        h = (h ^ *b as usize).wrapping_mul(0x100000001b3);
    }
    h
}
