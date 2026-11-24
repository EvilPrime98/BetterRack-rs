//! Storage Access Framework bridge: the folder picker, persisted tree grants and a `LibraryFs`.
//!
//! Folder listing, metadata and file reads call the framework (`ContentResolver`,
//! `DocumentsContract`) straight over JNI. Only the picker needs app code, because a
//! NativeActivity cannot receive `onActivityResult`: `java/picker.dex` holds a headless Fragment
//! that does, loaded at runtime (rebuild it with `java/build-dex.sh`).

use br_core::fs::{LibraryFs, ReadSeek};
use br_core::tree_fs::{Document, DocumentProvider, TreeFs};
use jni::errors::Error as JniError;
use jni::objects::{GlobalRef, JClass, JObject, JString, JValue};
use jni::sys::jclass;
use jni::{JNIEnv, JavaVM, NativeMethod};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::c_void;
use std::io;
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

static PICKER_DEX: &[u8] = include_bytes!("../java/picker.dex");
const PICKER_CLASS: &str = "dev.betterrack.android.SafPicker";
const DIR_MIME: &str = "vnd.android.document/directory";
const COLUMNS: [&str; 5] = ["document_id", "_display_name", "mime_type", "_size", "last_modified"];

fn vm() -> JavaVM {
    // SAFETY: android-activity initialises `ndk_context` with the process JavaVM before `android_main`.
    unsafe { JavaVM::from_raw(ndk_context::android_context().vm().cast()) }.expect("JavaVM")
}

static ACTIVITY: OnceLock<usize> = OnceLock::new();

/// Remembers the Activity (`ndk_context` only carries the Application, which cannot host a Fragment).
pub fn init(app: &slint::android::AndroidApp) {
    let _ = ACTIVITY.set(app.activity_as_ptr() as usize);
}

fn activity<'a>() -> JObject<'a> {
    // SAFETY: android-activity keeps a global reference to the Activity for the process lifetime.
    unsafe { JObject::from_raw(*ACTIVITY.get().expect("saf::init not called") as jni::sys::jobject) }
}

fn io_err(env: &mut JNIEnv, e: JniError) -> io::Error {
    if matches!(e, JniError::JavaException) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    io::Error::other(format!("android: {e}"))
}

/// Runs `f` on an attached thread, inside a local reference frame.
fn with_env<R>(f: impl FnOnce(&mut JNIEnv) -> jni::errors::Result<R>) -> io::Result<R> {
    let vm = vm();
    let mut env = vm.attach_current_thread().map_err(|e| io::Error::other(e.to_string()))?;
    let result = env.with_local_frame(32, |env| f(env));
    result.map_err(|e| io_err(&mut env, e))
}

fn parse_uri<'l>(env: &mut JNIEnv<'l>, s: &str) -> jni::errors::Result<JObject<'l>> {
    let s = env.new_string(s)?;
    env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[(&s).into()])?
        .l()
}

fn resolver<'l>(env: &mut JNIEnv<'l>) -> jni::errors::Result<JObject<'l>> {
    env.call_method(&activity(), "getContentResolver", "()Landroid/content/ContentResolver;", &[])?.l()
}

fn string_of(env: &mut JNIEnv, obj: JObject) -> jni::errors::Result<Option<String>> {
    if obj.is_null() {
        return Ok(None);
    }
    Ok(Some(env.get_string(&JString::from(obj))?.into()))
}

fn document_uri<'l>(env: &mut JNIEnv<'l>, tree: &str, doc_id: &str) -> jni::errors::Result<JObject<'l>> {
    let tree = parse_uri(env, tree)?;
    let id = env.new_string(doc_id)?;
    env.call_static_method(
        "android/provider/DocumentsContract",
        "buildDocumentUriUsingTree",
        "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
        &[(&tree).into(), (&id).into()],
    )?
    .l()
}

fn query<'l>(env: &mut JNIEnv<'l>, uri: &JObject, columns: &[&str]) -> jni::errors::Result<JObject<'l>> {
    let projection = env.new_object_array(columns.len() as i32, "java/lang/String", JObject::null())?;
    for (i, c) in columns.iter().enumerate() {
        let c = env.new_string(c)?;
        env.set_object_array_element(&projection, i as i32, c)?;
    }
    let resolver = resolver(env)?;
    env.call_method(
        &resolver,
        "query",
        "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
        &[uri.into(), (&projection).into(), (&JObject::null()).into(), (&JObject::null()).into(), (&JObject::null()).into()],
    )?
    .l()
}

fn document_from_row(env: &mut JNIEnv, cursor: &JObject) -> jni::errors::Result<Document> {
    let text = |env: &mut JNIEnv, col: i32| -> jni::errors::Result<String> {
        let s = env.call_method(cursor, "getString", "(I)Ljava/lang/String;", &[JValue::Int(col)])?.l()?;
        Ok(string_of(env, s)?.unwrap_or_default())
    };
    let id = text(env, 0)?;
    let name = text(env, 1)?;
    let mime = text(env, 2)?;
    let mut long = |col: i32| -> jni::errors::Result<i64> {
        env.call_method(cursor, "getLong", "(I)J", &[JValue::Int(col)])?.j()
    };
    Ok(Document {
        id,
        name,
        is_dir: mime == DIR_MIME,
        size: long(3)?.max(0) as u64,
        mtime_ms: long(4)? as f64,
    })
}

/// Reads every row of `cursor` and closes it.
fn read_rows(env: &mut JNIEnv, cursor: JObject) -> jni::errors::Result<Vec<Document>> {
    let mut rows = Vec::new();
    let result = (|| {
        while env.call_method(&cursor, "moveToNext", "()Z", &[])?.z()? {
            let row = env.with_local_frame(16, |env| document_from_row(env, &cursor))?;
            rows.push(row);
        }
        Ok(())
    })();
    let _ = env.call_method(&cursor, "close", "()V", &[]);
    result.map(|()| rows)
}

/// The JNI-backed provider: tree keys map to the tree URIs granted by the picker.
#[derive(Default)]
pub struct AndroidDocs {
    trees: RwLock<HashMap<String, String>>,
}

impl AndroidDocs {
    fn uri(&self, key: &str) -> io::Result<String> {
        self.trees
            .read()
            .unwrap()
            .get(key)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("unknown tree {key}")))
    }
}

impl DocumentProvider for AndroidDocs {
    fn children(&self, tree: &str, doc_id: &str) -> io::Result<Vec<Document>> {
        let tree = self.uri(tree)?;
        with_env(|env| {
            let tree_uri = parse_uri(env, &tree)?;
            let id = env.new_string(doc_id)?;
            let children = env
                .call_static_method(
                    "android/provider/DocumentsContract",
                    "buildChildDocumentsUriUsingTree",
                    "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
                    &[(&tree_uri).into(), (&id).into()],
                )?
                .l()?;
            let cursor = query(env, &children, &COLUMNS)?;
            if cursor.is_null() {
                return Ok(Vec::new());
            }
            read_rows(env, cursor)
        })
    }

    fn open(&self, tree: &str, doc_id: &str) -> io::Result<Box<dyn ReadSeek>> {
        let tree = self.uri(tree)?;
        let fd = with_env(|env| {
            let uri = document_uri(env, &tree, doc_id)?;
            let mode = env.new_string("r")?;
            let resolver = resolver(env)?;
            let pfd = env
                .call_method(
                    &resolver,
                    "openFileDescriptor",
                    "(Landroid/net/Uri;Ljava/lang/String;)Landroid/os/ParcelFileDescriptor;",
                    &[(&uri).into(), (&mode).into()],
                )?
                .l()?;
            if pfd.is_null() {
                return Ok(-1);
            }
            env.call_method(&pfd, "detachFd", "()I", &[])?.i()
        })?;
        if fd < 0 {
            return Err(io::Error::new(io::ErrorKind::NotFound, "provider returned no file"));
        }
        // SAFETY: `detachFd` transferred ownership of the descriptor to us.
        Ok(Box::new(unsafe { std::fs::File::from_raw_fd(fd) }))
    }
}

/// Display name and root document id of a freshly granted tree.
fn tree_info(tree: &str) -> io::Result<(String, String)> {
    with_env(|env| {
        let tree_uri = parse_uri(env, tree)?;
        let id = env
            .call_static_method(
                "android/provider/DocumentsContract",
                "getTreeDocumentId",
                "(Landroid/net/Uri;)Ljava/lang/String;",
                &[(&tree_uri).into()],
            )?
            .l()?;
        let root_id = string_of(env, id)?.unwrap_or_default();
        let doc = document_uri(env, tree, &root_id)?;
        let cursor = query(env, &doc, &COLUMNS)?;
        let mut label = String::new();
        if !cursor.is_null() {
            label = read_rows(env, cursor)?.into_iter().next().map(|d| d.name).unwrap_or_default();
        }
        Ok((label, root_id))
    })
}

/// The device's library storage: persisted SAF grants plus the `LibraryFs` that reads them.
pub struct Saf {
    fs: Arc<TreeFs<AndroidDocs>>,
    store: PathBuf,
}

impl Saf {
    /// Registers every previously granted tree. Does no JNI work, so it is cheap at startup.
    pub fn load(data_dir: &std::path::Path) -> Arc<Self> {
        let store = data_dir.join("saf_trees.json");
        let saved: Vec<Value> = std::fs::read(&store)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let saf = Arc::new(Self { fs: Arc::new(TreeFs::new(AndroidDocs::default())), store });
        for t in saved {
            let (Some(uri), Some(label)) = (t["uri"].as_str(), t["label"].as_str()) else { continue };
            let Some(root_id) = t["rootId"].as_str() else { continue };
            saf.register(uri, label, root_id);
        }
        saf
    }

    pub fn fs(&self) -> Arc<dyn LibraryFs> {
        self.fs.clone()
    }

    fn register(&self, uri: &str, label: &str, root_id: &str) -> String {
        let key = tree_key(uri);
        self.fs.provider().trees.write().unwrap().insert(key.clone(), uri.to_string());
        self.fs.register(&key, label, root_id)
    }

    /// Registers a tree the picker just granted and persists it. Returns its root path, which is
    /// what `POST /api/settings/library-folder` takes. Blocking.
    pub fn add(&self, uri: &str) -> io::Result<String> {
        let (label, root_id) = tree_info(uri)?;
        let label = if label.is_empty() { "Library".to_string() } else { label.replace('/', "-") };
        let path = self.register(uri, &label, &root_id);
        self.save(uri, &label, &root_id);
        Ok(path)
    }

    fn save(&self, uri: &str, label: &str, root_id: &str) {
        let mut stored: Vec<Value> = std::fs::read(&self.store)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        stored.retain(|t| t["uri"].as_str() != Some(uri));
        stored.push(json!({ "uri": uri, "label": label, "rootId": root_id }));
        let _ = std::fs::write(&self.store, serde_json::to_vec(&stored).unwrap_or_default());
    }
}

/// Stable short id of a tree URI, so the same folder keeps its uids across restarts.
fn tree_key(uri: &str) -> String {
    Sha256::digest(uri.as_bytes()).iter().take(4).map(|b| format!("{b:02x}")).collect()
}

type PickCallback = Box<dyn FnOnce(Option<String>) + Send>;
static PENDING: Mutex<Option<PickCallback>> = Mutex::new(None);
static PICKER: OnceLock<GlobalRef> = OnceLock::new();

extern "system" fn on_picked(mut env: JNIEnv, _class: JClass, uri: JString) {
    let picked = if uri.is_null() { None } else { env.get_string(&uri).ok().map(String::from) };
    if let Some(callback) = PENDING.lock().unwrap().take() {
        callback(picked);
    }
}

fn load_picker(env: &mut JNIEnv) -> jni::errors::Result<GlobalRef> {
    let parent = env.call_method(&activity(), "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?.l()?;
    // SAFETY: the dex bytes are `'static`; the loader only reads them.
    let buffer = unsafe { env.new_direct_byte_buffer(PICKER_DEX.as_ptr() as *mut u8, PICKER_DEX.len())? };
    let loader = env.new_object(
        "dalvik/system/InMemoryDexClassLoader",
        "(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V",
        &[(&buffer).into(), (&parent).into()],
    )?;
    let name = env.new_string(PICKER_CLASS)?;
    let class = env
        .call_method(&loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[(&name).into()])?
        .l()?;
    // SAFETY: `loadClass` returned a `java.lang.Class`.
    let class_ref = unsafe { JClass::from_raw(class.as_raw() as jclass) };
    env.register_native_methods(
        &class_ref,
        &[NativeMethod {
            name: "onPicked".into(),
            sig: "(Ljava/lang/String;)V".into(),
            fn_ptr: on_picked as *mut c_void,
        }],
    )?;
    env.new_global_ref(&class)
}

/// Opens the system folder picker. `done` runs on the UI thread with the granted tree URI, or
/// `None` if the user cancelled. Do heavy work (`Saf::add`) on another thread.
pub fn pick_folder(done: impl FnOnce(Option<String>) + Send + 'static) -> io::Result<()> {
    *PENDING.lock().unwrap() = Some(Box::new(done));
    with_env(|env| {
        let class = match PICKER.get() {
            Some(c) => c.clone(),
            None => {
                let c = load_picker(env)?;
                PICKER.get_or_init(|| c).clone()
            }
        };
        // SAFETY: the global reference holds a `java.lang.Class`.
        let class = unsafe { JClass::from_raw(class.as_obj().as_raw() as jclass) };
        env.call_static_method(&class, "launch", "(Landroid/app/Activity;)V", &[(&activity()).into()])?;
        Ok(())
    })
}
