//! Serve the bundled demo site, so trying the tool is one click rather than a
//! terminal command in a README.
//!
//! This is a static file server for three files on loopback, not a web server.
//! It exists because "run `python3 -m http.server` in another window first" is a
//! setup step nobody should need, and because depending on the machine having
//! Python is a worse dependency than eighty lines of Rust.
//!
//! It serves GET only, binds `127.0.0.1` only, and refuses to hand out anything
//! outside the demo directory — see the traversal tests at the bottom.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// The port the README and the demo site's own docs use.
pub const PREFERRED_PORT: u16 = 5173;

/// Only these come out of the demo directory. A file server that will hand over
/// anything it is asked for is a file server that will hand over your keys.
const SERVABLE: &[(&str, &str)] = &[
    ("html", "text/html; charset=utf-8"),
    ("js", "text/javascript; charset=utf-8"),
    ("css", "text/css; charset=utf-8"),
    ("json", "application/json; charset=utf-8"),
    ("svg", "image/svg+xml"),
    ("png", "image/png"),
    ("ico", "image/x-icon"),
    ("txt", "text/plain; charset=utf-8"),
];

fn content_type(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    SERVABLE
        .iter()
        .find(|(candidate, _)| *candidate == extension)
        .map(|(_, mime)| *mime)
}

/// Turn a request target into a file inside `root`, or nothing.
///
/// Pure so the rules can be attacked in tests rather than trusted.
pub fn resolve(root: &Path, target: &str) -> Option<PathBuf> {
    let target = target.split(['?', '#']).next().unwrap_or("");
    if !target.starts_with('/') {
        return None;
    }
    // Percent escapes are refused rather than decoded: decoding is where
    // traversal bugs live, and the demo site has no need of them.
    if target.contains('%') || target.contains('\\') {
        return None;
    }
    let mut path = root.to_path_buf();
    let mut segments = 0;
    for segment in target.split('/').filter(|part| !part.is_empty()) {
        if segment == "." || segment == ".." || segment.contains(':') {
            return None;
        }
        path.push(segment);
        segments += 1;
    }
    if segments == 0 {
        path.push("index.html");
    }
    if content_type(&path).is_none() {
        return None;
    }
    // Belt and braces: whatever the string said, the file must really be inside.
    let canonical_root = root.canonicalize().ok()?;
    let canonical = path.canonicalize().ok()?;
    if !canonical.starts_with(&canonical_root) {
        return None;
    }
    Some(canonical)
}

/// Where the demo site lives, in a dev checkout or beside a built binary.
pub fn find_root() -> Option<PathBuf> {
    if let Ok(from_env) = std::env::var("WEBMCP_DEMO_SITE") {
        let path = PathBuf::from(from_env);
        if path.join("index.html").is_file() {
            return Some(path);
        }
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        let mut at = exe.parent().map(Path::to_path_buf);
        // target/debug/debugger → repo root is three levels up.
        for _ in 0..4 {
            let Some(dir) = at else { break };
            candidates.push(dir.join("demo-site"));
            at = dir.parent().map(Path::to_path_buf);
        }
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo-site"));
    candidates
        .into_iter()
        .find(|path| path.join("index.html").is_file())
}

pub struct DemoSite {
    pub url: String,
    stop: Arc<AtomicBool>,
}

impl DemoSite {
    /// Start serving `root`. Prefers 5173 so the address matches the docs, and
    /// falls back to any free port rather than failing outright.
    pub fn start(root: PathBuf) -> std::io::Result<Self> {
        Self::start_on(root, PREFERRED_PORT)
    }

    /// Bind a specific port, or any free one when given 0.
    ///
    /// Tests use 0: two tests that both want 5173 contend with each other and
    /// with anything else on the machine, which makes them fail at random.
    pub fn start_on(root: PathBuf, port: u16) -> std::io::Result<Self> {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))
            .or_else(|_| TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;

        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        thread::spawn(move || {
            for incoming in listener.incoming() {
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                match incoming {
                    Ok(stream) => {
                        let root = root.clone();
                        thread::spawn(move || {
                            let _ = serve(stream, &root);
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(40));
                    }
                    Err(_) => return,
                }
            }
        });

        Ok(Self {
            url: format!("http://127.0.0.1:{port}"),
            stop,
        })
    }
}

impl Drop for DemoSite {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn serve(mut stream: TcpStream, root: &Path) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;

    // Drain the headers: closing a socket with unread bytes makes the kernel
    // send RST, and the browser may then throw away the response it already had.
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
    }

    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");

    if method != "GET" {
        return respond(&mut stream, 405, "text/plain; charset=utf-8", b"GET only");
    }
    let Some(path) = resolve(root, target) else {
        return respond(&mut stream, 404, "text/plain; charset=utf-8", b"not found");
    };
    let Some(mime) = content_type(&path) else {
        return respond(&mut stream, 404, "text/plain; charset=utf-8", b"not found");
    };
    let mut body = Vec::new();
    std::fs::File::open(&path)?.read_to_end(&mut body)?;
    respond(&mut stream, 200, mime, &body)
}

fn respond(stream: &mut TcpStream, status: u16, mime: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Method Not Allowed",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo-site")
    }

    #[test]
    fn the_root_serves_the_index() {
        let root = demo_root();
        let resolved = resolve(&root, "/").expect("root must serve index.html");
        assert!(resolved.ends_with("index.html"));
    }

    #[test]
    fn a_named_file_resolves() {
        let root = demo_root();
        assert!(resolve(&root, "/tools.js").is_some());
        assert!(resolve(&root, "/styles.css").is_some());
    }

    #[test]
    fn traversal_is_refused_however_it_is_spelled() {
        let root = demo_root();
        for attempt in [
            "/../Cargo.toml",
            "/../../Cargo.toml",
            "/./../../Cargo.toml",
            "/subdir/../../Cargo.toml",
            "/%2e%2e/Cargo.toml",
            "/..%2fCargo.toml",
            "\\..\\Cargo.toml",
            "/C:/Windows/system.ini",
        ] {
            assert!(resolve(&root, attempt).is_none(), "escaped with {attempt:?}");
        }
    }

    #[test]
    fn an_absolute_or_malformed_target_is_refused() {
        let root = demo_root();
        assert!(resolve(&root, "tools.js").is_none(), "must start with /");
        assert!(resolve(&root, "").is_none());
        assert!(resolve(&root, "http://evil/").is_none());
    }

    #[test]
    fn only_the_allowed_kinds_of_file_come_out() {
        let root = demo_root();
        // Even inside the directory, nothing outside the allowlist is servable.
        assert!(content_type(Path::new("x.html")).is_some());
        assert!(content_type(Path::new("x.js")).is_some());
        assert!(content_type(Path::new("x.pem")).is_none());
        assert!(content_type(Path::new("x.rs")).is_none());
        assert!(content_type(Path::new("noextension")).is_none());
        assert!(resolve(&root, "/../Cargo.lock").is_none());
    }

    #[test]
    fn a_file_that_does_not_exist_resolves_to_nothing() {
        assert!(resolve(&demo_root(), "/nope.html").is_none());
    }

    fn fetch(url: &str, path: &str) -> std::io::Result<String> {
        let address = url.trim_start_matches("http://");
        let mut stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        write!(stream, "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")?;
        let mut body = String::new();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw)?;
        body.push_str(&String::from_utf8_lossy(&raw));
        Ok(body)
    }

    #[test]
    fn the_site_serves_while_it_lives_and_stops_when_dropped() {
        // The playground owns this server: it starts when you enter and must be
        // gone when you leave, not left holding a port for the whole session.
        let site = DemoSite::start_on(demo_root(), 0).expect("start");
        let url = site.url.clone();
        let served = fetch(&url, "/").expect("should serve while alive");
        assert!(served.contains("200 OK"), "{served}");

        drop(site);
        // The accept loop notices the flag on its next pass.
        thread::sleep(Duration::from_millis(300));
        assert!(
            fetch(&url, "/").is_err(),
            "the port must be released when the playground closes"
        );
    }

    #[test]
    fn a_second_site_can_start_after_the_first_is_dropped() {
        // Entering, leaving and entering the playground again must work.
        // Whether it lands back on 5173 depends on what else holds the port, so
        // assert what actually matters: it starts again, and it serves.
        let first = DemoSite::start_on(demo_root(), 0).expect("first");
        drop(first);
        thread::sleep(Duration::from_millis(300));
        let second = DemoSite::start_on(demo_root(), 0).expect("second start must not be blocked");
        let served = fetch(&second.url, "/").expect("the second site must serve");
        assert!(served.contains("200 OK"), "{served}");
    }

    #[test]
    fn the_demo_directory_is_findable_from_a_dev_checkout() {
        let root = find_root().expect("demo-site should be found next to the crate");
        assert!(root.join("index.html").is_file());
    }
}
