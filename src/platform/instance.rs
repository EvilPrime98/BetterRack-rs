//! Single instance for local file tabs. The first process binds a per-user local socket (named
//! pipe on Windows, unix socket elsewhere); later launches send their file path to it and exit.
//! Binding doubles as the "is an instance alive" check.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::runtime;

fn user() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default()
}

/// Send `path` to the running instance. `false` when none is reachable.
pub fn forward(path: &Path) -> bool {
    let Some(path) = path.to_str() else {
        return false;
    };
    connect()
        .and_then(|mut stream| stream.write_all(path.as_bytes()))
        .is_ok()
}

#[cfg(windows)]
mod imp {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    fn name() -> String {
        format!(r"\\.\pipe\betterrack-{}", user())
    }

    pub fn connect() -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new().write(true).open(name())
    }

    pub struct Server(NamedPipeServer);

    pub fn bind() -> Option<Server> {
        runtime::block_on(async {
            ServerOptions::new()
                .first_pipe_instance(true)
                .create(name())
        })
        .ok()
        .map(Server)
    }

    impl Server {
        pub async fn next(&mut self) -> Option<PathBuf> {
            loop {
                self.0.connect().await.ok()?;
                let next = ServerOptions::new().create(name()).ok()?;
                let mut done = std::mem::replace(&mut self.0, next);
                let mut text = String::new();
                if done.read_to_string(&mut text).await.is_ok() && !text.is_empty() {
                    return Some(PathBuf::from(text));
                }
            }
        }
    }
}

#[cfg(unix)]
mod imp {
    use super::*;
    use std::os::unix::net::UnixStream;
    use tokio::io::AsyncReadExt;
    use tokio::net::UnixListener;

    fn socket() -> PathBuf {
        match dirs::runtime_dir() {
            Some(dir) => dir.join("betterrack.sock"),
            None => std::env::temp_dir().join(format!("betterrack-{}.sock", user())),
        }
    }

    pub fn connect() -> std::io::Result<UnixStream> {
        UnixStream::connect(socket())
    }

    pub struct Server(UnixListener);

    pub fn bind() -> Option<Server> {
        let path = socket();
        let listen = || runtime::block_on(async { UnixListener::bind(&path) });
        match listen() {
            Ok(l) => Some(Server(l)),
            // A file with nobody behind it (crashed instance) must not block startup.
            Err(_) if connect().is_err() => {
                let _ = std::fs::remove_file(&path);
                listen().ok().map(Server)
            }
            Err(_) => None,
        }
    }

    impl Server {
        pub async fn next(&mut self) -> Option<PathBuf> {
            loop {
                let (mut stream, _) = self.0.accept().await.ok()?;
                let mut text = String::new();
                if stream.read_to_string(&mut text).await.is_ok() && !text.is_empty() {
                    return Some(PathBuf::from(text));
                }
            }
        }
    }
}

use imp::connect;
pub use imp::{Server, bind};

/// Decide between becoming the primary instance and handing `file` to the running one.
/// `Err(())` means the file was forwarded and this process should exit.
pub fn claim(file: Option<&Path>) -> Result<Option<Server>, ()> {
    // Simultaneous launches race for the bind; the losers retry the connect until the winner listens.
    let attempts = if file.is_some() { 20 } else { 1 };
    for _ in 0..attempts {
        if let Some(server) = bind() {
            return Ok(Some(server));
        }
        if file.is_some_and(forward) {
            return Err(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(None)
}
