//! Localhost WebSocket bridge used by the MV3 extension.
//!
//! Bind `127.0.0.1` only. Accept `Origin: chrome-extension://<id>` only so
//! ordinary HTTPS pages cannot drive `execute_tool`.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::Value;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::{accept_hdr, Message};
use webmcp_protocol::{BrowserEvent, DebuggerCommand, WS_HOST, WS_PORT};

/// Comfortably inside Chrome's ~30s service-worker idle timeout.
pub const HEARTBEAT: Duration = Duration::from_secs(20);

/// How long a read waits before the pump checks for outgoing commands.
const READ_POLL: Duration = Duration::from_millis(15);

/// Loopback only, and Chrome drains sockets even while the worker sleeps, so a
/// send buffer that stays full this long means nobody is reading.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

/// Stable unpacked extension id (from `extension/manifest.json` `key`).
pub const EXTENSION_ID: &str = "ffaihbpimepkgggjclheahfddigmmfeg";

pub fn allowed_extension_origin() -> String {
    format!("chrome-extension://{EXTENSION_ID}")
}

pub fn listen_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], WS_PORT))
}

pub fn origin_is_allowed(origin: Option<&str>) -> bool {
    match origin {
        Some(value) if value == allowed_extension_origin() => true,
        Some(_) | None => false,
    }
}

pub fn serve() -> std::io::Result<()> {
    let addr = listen_addr();
    let listener = TcpListener::bind(addr)?;
    eprintln!("ws-server listening on ws://{WS_HOST}:{WS_PORT}");
    eprintln!("allowed Origin: {}", allowed_extension_origin());

    let hub = Hub::new(None, true);
    accept_loop(listener, hub)
}

/// In-process bridge: the GPUI app owns the listener and talks to the
/// extension over channels, so it never needs a chrome-extension Origin.
pub struct ChromeBridge {
    hub: Arc<Hub>,
    incoming: Receiver<BridgeEvent>,
}

#[derive(Debug)]
pub enum BridgeEvent {
    ClientsChanged { connected: usize },
    Browser(BrowserEvent),
    /// A frame we could not turn into a `BrowserEvent`. Carried to the UI instead
    /// of dropped, so a mismatched extension does not look like silence.
    Unparsable { reason: String, raw: String },
}

impl ChromeBridge {
    pub fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind(listen_addr())?;
        eprintln!("debugger listening on ws://{WS_HOST}:{WS_PORT}");
        eprintln!("allowed Origin: {}", allowed_extension_origin());
        let (tx, rx) = mpsc::channel();
        let hub = Hub::new(Some(tx), false);
        let accept_hub = Arc::clone(&hub);

        // Keep the extension's service worker alive. Chrome evicts it after
        // roughly 30 seconds idle, taking the socket with it, so we speak well
        // inside that window.
        let heartbeat_hub = Arc::clone(&hub);
        thread::spawn(move || {
            let ping = serde_json::to_string(&DebuggerCommand::Ping).unwrap_or_default();
            loop {
                thread::sleep(HEARTBEAT);
                heartbeat_hub.heartbeat_all(&ping);
            }
        });
        thread::spawn(move || {
            if let Err(error) = accept_loop(listener, accept_hub) {
                eprintln!("ws accept loop ended: {error}");
            }
        });
        Ok(Self { hub, incoming: rx })
    }

    pub fn poll(&self) -> Vec<BridgeEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.incoming.try_recv() {
            events.push(event);
        }
        events
    }

    pub fn send(&self, command: &DebuggerCommand) -> bool {
        let Ok(text) = serde_json::to_string(command) else {
            return false;
        };
        self.hub.send_one(&text)
    }

    pub fn client_count(&self) -> usize {
        self.hub.client_count()
    }
}

fn accept_loop(listener: TcpListener, hub: Arc<Hub>) -> std::io::Result<()> {
    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                let hub = Arc::clone(&hub);
                thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, hub) {
                        eprintln!("connection closed: {error}");
                    }
                });
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

struct Hub {
    next_id: AtomicUsize,
    clients: Mutex<HashMap<usize, Sender<String>>>,
    ui_tx: Option<Sender<BridgeEvent>>,
    log_stdout: bool,
}

impl Hub {
    fn new(ui_tx: Option<Sender<BridgeEvent>>, log_stdout: bool) -> Arc<Self> {
        Arc::new(Self {
            next_id: AtomicUsize::new(1),
            clients: Mutex::new(HashMap::new()),
            ui_tx,
            log_stdout,
        })
    }

    fn emit(&self, event: BridgeEvent) {
        if let Some(tx) = &self.ui_tx {
            let _ = tx.send(event);
        }
    }

    fn register(&self, tx: Sender<String>) -> usize {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let connected = {
            let mut clients = self.clients.lock().expect("hub lock");
            clients.insert(id, tx);
            clients.len()
        };
        self.emit(BridgeEvent::ClientsChanged { connected });
        id
    }

    fn unregister(&self, id: usize) {
        let connected = {
            let mut clients = self.clients.lock().expect("hub lock");
            clients.remove(&id);
            clients.len()
        };
        self.emit(BridgeEvent::ClientsChanged { connected });
    }

    fn client_count(&self) -> usize {
        self.clients.lock().expect("hub lock").len()
    }

    /// Reach every client. Only for the keepalive: a command must go to one
    /// browser, but a heartbeat has to keep all of them awake.
    fn heartbeat_all(&self, text: &str) {
        let clients = self.clients.lock().expect("hub lock");
        for tx in clients.values() {
            let _ = tx.send(text.to_string());
        }
    }

    /// Send to exactly one client — the lowest-numbered connected one.
    ///
    /// Broadcasting meant two connected browsers each ran the tool, so one click
    /// mutated two sessions. Ids are handed out in order, so "lowest" is stable
    /// for as long as that browser stays connected.
    fn send_one(&self, text: &str) -> bool {
        let clients = self.clients.lock().expect("hub lock");
        match clients.keys().min() {
            Some(id) => clients
                .get(id)
                .map(|tx| tx.send(text.to_string()).is_ok())
                .unwrap_or(false),
            None => false,
        }
    }


    fn broadcast(&self, from: usize, text: &str) {
        let clients = self.clients.lock().expect("hub lock");
        for (id, tx) in clients.iter() {
            if *id != from {
                let _ = tx.send(text.to_string());
            }
        }
    }
}

fn origin_callback(request: &Request, response: Response) -> Result<Response, ErrorResponse> {
    let origin = request
        .headers()
        .get("Origin")
        .and_then(|value| value.to_str().ok());
    if origin_is_allowed(origin) {
        return Ok(response);
    }
    let mut forbidden = http::Response::new(Some("forbidden origin".to_string()));
    *forbidden.status_mut() = http::StatusCode::FORBIDDEN;
    Err(forbidden)
}

fn handle_connection(stream: TcpStream, hub: Arc<Hub>) -> Result<(), Box<dyn std::error::Error>> {
    let peer = stream.peer_addr()?;
    let mut socket = accept_hdr(stream, origin_callback)
        .map_err(|error| format!("handshake from {peer} rejected: {error}"))?;
    // Blocking with timeouts: a large frame on a non-blocking socket hit
    // WouldBlock mid-write, and that dropped the whole connection.
    socket.get_mut().set_read_timeout(Some(READ_POLL))?;
    socket.get_mut().set_write_timeout(Some(WRITE_TIMEOUT))?;

    let (tx, rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    let id = hub.register(tx);
    eprintln!("client {id} connected from {peer}");

    // Every exit from the pump — clean close, read error, or a failed write —
    // funnels through the single unregister below. Returning early from inside
    // the loop is what leaked the client entry: the hub kept counting a peer
    // nobody was reading for, so the UI stayed "connected" and outbound
    // commands queued into a channel with no receiver.
    let outcome = pump_connection(&mut socket, &rx, &hub, id);
    hub.unregister(id);
    eprintln!("client {id} disconnected");
    outcome
}

fn pump_connection(
    socket: &mut tungstenite::WebSocket<TcpStream>,
    rx: &Receiver<String>,
    hub: &Arc<Hub>,
    id: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        while let Ok(outgoing) = rx.try_recv() {
            socket.send(Message::Text(outgoing.into()))?;
        }

        match socket.read() {
            Ok(Message::Text(text)) => {
                let text = text.to_string();
                if is_protocol_message(&text) {
                    if hub.log_stdout {
                        println!("{text}");
                    }
                    hub.broadcast(id, &text);
                    match serde_json::from_str::<BrowserEvent>(&text) {
                        Ok(event) => hub.emit(BridgeEvent::Browser(event)),
                        // Commands are ours, relayed between clients — not
                        // events that failed to parse. Reporting them would put
                        // a protocol error in the log every heartbeat.
                        Err(_) if is_command(&text) => {}
                        Err(error) => hub.emit(BridgeEvent::Unparsable {
                            reason: error.to_string(),
                            raw: text,
                        }),
                    }
                } else {
                    hub.emit(BridgeEvent::Unparsable {
                        reason: "not a protocol message".to_string(),
                        raw: text,
                    });
                }
            }
            Ok(Message::Ping(payload)) => {
                socket.send(Message::Pong(payload))?;
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) | Ok(Message::Frame(_)) => break,
            Ok(Message::Binary(payload)) => {
                hub.emit(BridgeEvent::Unparsable {
                    reason: format!("binary frame, {} bytes", payload.len()),
                    raw: String::new(),
                });
            }
            // The read timeout fired: nothing arrived, go send what is queued.
            Err(tungstenite::Error::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(tungstenite::Error::ConnectionClosed) => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn is_command(text: &str) -> bool {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(Value::as_str)
                .map(|kind| matches!(kind, "ping" | "subscribe_page" | "execute_tool" | "cancel_execution" | "open_page"))
        })
        .unwrap_or(false)
}

fn is_protocol_message(text: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    match value.get("type").and_then(Value::as_str) {
        Some("hello")
        | Some("page_changed")
        | Some("tools_changed")
        | Some("tool_execution_started")
        | Some("tool_execution_finished")
        | Some("tool_execution_failed")
        | Some("page_closed")
        | Some("disconnected")
        | Some("subscribe_page")
        | Some("execute_tool")
        | Some("ping")
        | Some("cancel_execution")
        | Some("open_page") => true,
        Some(_) | None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_allowlist_is_extension_only() {
        assert!(origin_is_allowed(Some(&allowed_extension_origin())));
        assert!(!origin_is_allowed(None));
        assert!(!origin_is_allowed(Some("http://localhost:5173")));
        assert!(!origin_is_allowed(Some("https://evil.example")));
        assert!(!origin_is_allowed(Some(
            "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        )));
    }

    #[test]
    fn listen_addr_is_loopback_only() {
        let addr = listen_addr();
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.port(), 17321);
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
    }

    #[test]
    fn hub_client_count_returns_to_zero_after_unregister() {
        let hub = Hub::new(None, false);
        let (tx, _rx) = mpsc::channel();
        let id = hub.register(tx);
        assert_eq!(hub.client_count(), 1);
        hub.unregister(id);
        assert_eq!(
            hub.client_count(),
            0,
            "a client left registered keeps the UI reporting a peer nobody reads for"
        );
    }

    #[test]
    fn hub_reports_every_client_count_change() {
        let (ui_tx, ui_rx) = mpsc::channel();
        let hub = Hub::new(Some(ui_tx), false);
        let (tx, _rx) = mpsc::channel();
        let id = hub.register(tx);
        hub.unregister(id);

        let counts: Vec<usize> = ui_rx
            .try_iter()
            .filter_map(|event| match event {
                BridgeEvent::ClientsChanged { connected } => Some(connected),
                _ => None,
            })
            .collect();
        assert_eq!(counts, vec![1, 0]);
    }

    #[test]
    fn a_command_reaches_exactly_one_client() {
        // Two browsers connected used to mean one click ran the tool twice.
        let hub = Hub::new(None, false);
        let (first_tx, first_rx) = mpsc::channel();
        let (second_tx, second_rx) = mpsc::channel();
        hub.register(first_tx);
        hub.register(second_tx);

        assert!(hub.send_one("{\"type\":\"execute_tool\"}"));
        assert_eq!(first_rx.try_iter().count(), 1, "the first client gets it");
        assert_eq!(second_rx.try_iter().count(), 0, "the second must not");
    }

    #[test]
    fn routing_falls_to_the_next_client_when_the_first_leaves() {
        let hub = Hub::new(None, false);
        let (first_tx, first_rx) = mpsc::channel();
        let (second_tx, second_rx) = mpsc::channel();
        let first = hub.register(first_tx);
        hub.register(second_tx);
        hub.unregister(first);
        drop(first_rx);

        assert!(hub.send_one("{}"));
        assert_eq!(second_rx.try_iter().count(), 1);
    }

    #[test]
    fn sending_with_nobody_connected_reports_failure_rather_than_success() {
        let hub = Hub::new(None, false);
        assert!(!hub.send_one("{}"));
    }

    #[test]
    fn a_dead_peer_still_counts_until_it_is_unregistered() {
        // Documents why the leak mattered: `send_all` reports success purely from
        // the registry, so a client that is never unregistered keeps the app
        // believing the extension is there. Every exit path in
        // `handle_connection` must therefore reach `unregister`.
        let hub = Hub::new(None, false);
        let (tx, rx) = mpsc::channel();
        let id = hub.register(tx);
        drop(rx);
        assert_eq!(hub.client_count(), 1);
        hub.unregister(id);
        assert_eq!(hub.client_count(), 0);
    }

    /// Drive the real `handle_connection` over a real loopback socket, doing a
    /// real handshake with the allowed Origin. Returns the events the UI saw.
    fn round_trip(send: impl FnOnce(&mut tungstenite::WebSocket<TcpStream>)) -> Vec<BridgeEvent> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("ephemeral bind");
        let port = listener.local_addr().unwrap().port();
        let (ui_tx, ui_rx) = mpsc::channel();
        let hub = Hub::new(Some(ui_tx), false);
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let _ = handle_connection(stream, hub);
        });

        let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let request = http::Request::builder()
            .method("GET")
            .uri(format!("ws://127.0.0.1:{port}/"))
            .header("Host", format!("127.0.0.1:{port}"))
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tungstenite::handshake::client::generate_key(),
            )
            .header("Origin", allowed_extension_origin())
            .body(())
            .unwrap();
        let (mut socket, _) = tungstenite::client::client(request, stream).expect("handshake");

        send(&mut socket);
        server.join().expect("server thread");
        ui_rx.try_iter().collect()
    }

    #[test]
    fn an_unparsable_frame_reaches_the_ui_instead_of_being_dropped() {
        // A known `type` the debugger accepts, but `duration_ms` arrives as a
        // string. Before this it was relayed and then silently discarded, so a
        // mismatched extension was indistinguishable from silence.
        let events = round_trip(|socket| {
            socket
                .send(Message::Text(
                    r#"{"type":"tool_execution_finished","execution_id":"exec_1","result":1,"duration_ms":"412","timestamp":"2026-08-31T14:23:07Z"}"#
                        .into(),
                ))
                .unwrap();
            socket.close(None).ok();
            while socket.read().is_ok() {}
        });

        let reported = events.iter().find_map(|event| match event {
            BridgeEvent::Unparsable { reason, raw } => Some((reason, raw)),
            _ => None,
        });
        let (reason, raw) = reported.expect("the frame must be reported, not dropped");
        assert!(!reason.is_empty());
        assert!(raw.contains("exec_1"), "the raw payload must survive");
    }

    #[test]
    fn a_client_that_vanishes_without_closing_is_still_unregistered() {
        // The leak was an exit path that skipped `unregister`. Whatever way the
        // connection ends, the count must come back to zero.
        let events = round_trip(|socket| {
            socket.get_mut().shutdown(std::net::Shutdown::Both).ok();
        });

        let counts: Vec<usize> = events
            .iter()
            .filter_map(|event| match event {
                BridgeEvent::ClientsChanged { connected } => Some(*connected),
                _ => None,
            })
            .collect();
        assert_eq!(
            counts.last(),
            Some(&0),
            "a vanished client must not keep the app reporting a live extension"
        );
    }

    #[test]
    fn a_failed_write_still_unregisters_the_client() {
        // This is the exit that leaked. A client that never reads eventually
        // makes the server's write fail; the old code returned straight out of
        // the loop, so the entry stayed in the hub forever and `send_all` kept
        // reporting a peer nobody was reading for.
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("ephemeral bind");
        let port = listener.local_addr().unwrap().port();
        let (ui_tx, ui_rx) = mpsc::channel();
        let hub = Hub::new(Some(ui_tx), false);
        let writer = Arc::clone(&hub);
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let _ = handle_connection(stream, hub);
        });

        let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let request = http::Request::builder()
            .method("GET")
            .uri(format!("ws://127.0.0.1:{port}/"))
            .header("Host", format!("127.0.0.1:{port}"))
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tungstenite::handshake::client::generate_key(),
            )
            .header("Origin", allowed_extension_origin())
            .body(())
            .unwrap();
        let (socket, _) = tungstenite::client::client(request, stream).expect("handshake");

        while writer.client_count() == 0 {
            thread::sleep(Duration::from_millis(1));
        }
        // The client is never read from. Queue far more than any socket buffer
        // can absorb so the write cannot succeed.
        let payload = "y".repeat(64 * 1024);
        for _ in 0..512 {
            writer.send_one(&payload);
        }

        server.join().expect("server thread must exit, not hang");
        drop(socket);

        let counts: Vec<usize> = ui_rx
            .try_iter()
            .filter_map(|event| match event {
                BridgeEvent::ClientsChanged { connected } => Some(connected),
                _ => None,
            })
            .collect();
        assert_eq!(
            counts.last(),
            Some(&0),
            "a write failure must not leave the client registered"
        );
        assert_eq!(writer.client_count(), 0);
    }

    #[test]
    fn a_large_command_reaches_the_client_intact() {
        // A big `execute_tool` payload used to hit WouldBlock on the
        // non-blocking socket, and the error path dropped the whole connection.
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("ephemeral bind");
        let port = listener.local_addr().unwrap().port();
        let hub = Hub::new(None, false);
        let writer = Arc::clone(&hub);
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let _ = handle_connection(stream, hub);
        });

        let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let request = http::Request::builder()
            .method("GET")
            .uri(format!("ws://127.0.0.1:{port}/"))
            .header("Host", format!("127.0.0.1:{port}"))
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tungstenite::handshake::client::generate_key(),
            )
            .header("Origin", allowed_extension_origin())
            .body(())
            .unwrap();
        let (mut socket, _) = tungstenite::client::client(request, stream).expect("handshake");

        while writer.client_count() == 0 {
            thread::sleep(Duration::from_millis(1));
        }
        // Far larger than any loopback socket buffer, so a single write cannot
        // take all of it at once.
        let payload = format!("{{\"type\":\"execute_tool\",\"arguments\":\"{}\"}}", "y".repeat(4 * 1024 * 1024));
        assert!(writer.send_one(&payload));

        let received = socket.read().expect("the connection must survive a large frame");
        assert_eq!(received.to_text().unwrap(), payload);
        socket.close(None).ok();
        while socket.read().is_ok() {}
        server.join().expect("server thread");
    }

    #[test]
    fn the_heartbeat_is_a_real_protocol_message_the_filter_lets_through() {
        // If the filter drops it, the ping never reaches the service worker and
        // the worker dies — which looks exactly like "no tools found".
        let ping = serde_json::to_string(&DebuggerCommand::Ping).unwrap();
        assert!(is_protocol_message(&ping), "the filter must pass {ping}");
        assert!(is_command(&ping));
    }

    #[test]
    fn a_relayed_command_is_not_reported_as_a_broken_frame() {
        // Commands are ours. Treating one as an unparsable event would put a
        // protocol error in the log on every single heartbeat.
        for command in [
            DebuggerCommand::Ping,
            DebuggerCommand::OpenPage { url: "https://example.com".into() },
        ] {
            let text = serde_json::to_string(&command).unwrap();
            assert!(is_command(&text), "{text}");
        }
        // A genuine event is not a command, so a broken one still gets reported.
        assert!(!is_command(r#"{"type":"tool_execution_finished","duration_ms":"x"}"#));
    }

    #[test]
    fn the_heartbeat_stays_inside_chromes_eviction_window() {
        // Chrome evicts an idle service worker at roughly 30 seconds.
        assert!(
            HEARTBEAT < Duration::from_secs(30),
            "a heartbeat at or past the timeout does not keep anything alive"
        );
    }

    #[test]
    fn the_heartbeat_reaches_every_connected_browser() {
        let hub = Hub::new(None, false);
        let (first_tx, first_rx) = mpsc::channel();
        let (second_tx, second_rx) = mpsc::channel();
        hub.register(first_tx);
        hub.register(second_tx);
        hub.heartbeat_all("{\"type\":\"ping\"}");
        // Unlike a command, which goes to one browser, this must wake them all.
        assert_eq!(first_rx.try_iter().count(), 1);
        assert_eq!(second_rx.try_iter().count(), 1);
    }

    #[test]
    fn the_filter_admits_every_message_the_protocol_can_produce() {
        // The allowlist is hand-maintained, so it is the natural place for a new
        // event to be silently dropped. Round-trip one of every variant through it.
        use chrono::{TimeZone, Utc};
        use webmcp_protocol::{ExecutionId, Page, PageId};
        let at = Utc.with_ymd_and_hms(2026, 8, 31, 14, 0, 0).unwrap();
        let page = Page {
            id: PageId::from("tab:1"),
            url: "https://example.com/".into(),
            title: "Example".into(),
            origin: "https://example.com".into(),
        };
        let events = vec![
            BrowserEvent::Hello { protocol_version: 1, timestamp: at },
            BrowserEvent::PageChanged { page: page.clone(), timestamp: at },
            BrowserEvent::ToolsChanged {
                page_id: page.id.clone(),
                origin: page.origin.clone(),
                url: page.url.clone(),
                tools: Vec::new(),
                timestamp: at,
            },
            BrowserEvent::ToolExecutionStarted {
                execution_id: ExecutionId::from("e1"),
                tool: "t".into(),
                arguments: serde_json::json!({}),
                timestamp: at,
            },
            BrowserEvent::ToolExecutionFinished {
                execution_id: ExecutionId::from("e1"),
                result: serde_json::json!(1),
                duration_ms: 1,
                timestamp: at,
            },
            BrowserEvent::ToolExecutionFailed {
                execution_id: ExecutionId::from("e1"),
                error: "x".into(),
                duration_ms: 1,
                timestamp: at,
            },
            BrowserEvent::PageClosed { page_id: page.id.clone(), timestamp: at },
            BrowserEvent::Disconnected { timestamp: at },
        ];
        for event in events {
            let text = serde_json::to_string(&event).unwrap();
            assert!(is_protocol_message(&text), "filter drops {text}");
        }

        let commands = vec![
            DebuggerCommand::SubscribePage { page_id: page.id.clone() },
            DebuggerCommand::ExecuteTool {
                page_id: page.id.clone(),
                tool: "t".into(),
                arguments: serde_json::json!({}),
                execution_id: ExecutionId::from("e1"),
            },
            DebuggerCommand::CancelExecution {
                page_id: page.id,
                execution_id: ExecutionId::from("e1"),
            },
            DebuggerCommand::Ping,
            DebuggerCommand::OpenPage { url: "https://example.com".into() },
        ];
        for command in commands {
            let text = serde_json::to_string(&command).unwrap();
            assert!(is_protocol_message(&text), "filter drops {text}");
        }
    }

    #[test]
    fn protocol_message_filter_accepts_known_types() {
        assert!(is_protocol_message(r#"{"type":"tools_changed"}"#));
        assert!(is_protocol_message(r#"{"type":"execute_tool"}"#));
        assert!(is_protocol_message(r#"{"type":"open_page"}"#));
        assert!(!is_protocol_message(r#"{"type":"nope"}"#));
        assert!(!is_protocol_message("not json"));
    }
}
