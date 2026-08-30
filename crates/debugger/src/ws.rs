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
}

impl ChromeBridge {
    pub fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind(listen_addr())?;
        eprintln!("debugger listening on ws://{WS_HOST}:{WS_PORT}");
        eprintln!("allowed Origin: {}", allowed_extension_origin());
        let (tx, rx) = mpsc::channel();
        let hub = Hub::new(Some(tx), false);
        let accept_hub = Arc::clone(&hub);
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
        self.hub.send_all(&text) > 0
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

    fn send_all(&self, text: &str) -> usize {
        let clients = self.clients.lock().expect("hub lock");
        for tx in clients.values() {
            let _ = tx.send(text.to_string());
        }
        clients.len()
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
    socket.get_mut().set_nonblocking(true)?;

    let (tx, rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    let id = hub.register(tx);
    eprintln!("client {id} connected from {peer}");

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
                    if let Ok(event) = serde_json::from_str::<BrowserEvent>(&text) {
                        hub.emit(BridgeEvent::Browser(event));
                    }
                } else {
                    eprintln!("client {id} sent non-protocol message, dropping");
                }
            }
            Ok(Message::Ping(payload)) => {
                socket.send(Message::Pong(payload))?;
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) | Ok(Message::Frame(_)) => break,
            Ok(Message::Binary(_)) => {
                eprintln!("client {id} sent binary, dropping");
            }
            Err(tungstenite::Error::Io(error)) if error.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(15));
            }
            Err(tungstenite::Error::ConnectionClosed) => break,
            Err(error) => {
                hub.unregister(id);
                return Err(error.into());
            }
        }
    }

    hub.unregister(id);
    eprintln!("client {id} disconnected");
    Ok(())
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
        | Some("disconnected")
        | Some("subscribe_page")
        | Some("execute_tool")
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
    fn protocol_message_filter_accepts_known_types() {
        assert!(is_protocol_message(r#"{"type":"tools_changed"}"#));
        assert!(is_protocol_message(r#"{"type":"execute_tool"}"#));
        assert!(is_protocol_message(r#"{"type":"open_page"}"#));
        assert!(!is_protocol_message(r#"{"type":"nope"}"#));
        assert!(!is_protocol_message("not json"));
    }
}
