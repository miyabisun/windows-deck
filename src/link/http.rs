//! Requests to windows-link and its `/events` socket, behind small traits so that tests can
//! stand in for windows-link.

use std::{io::ErrorKind, net::TcpStream, sync::Arc, thread, time::Duration};

use futures::channel::mpsc::UnboundedSender;
use serde_json::Value;

/// windows-link's answer to a request.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub status: u16,
    /// The JSON body, if it had one.
    pub body: Option<Value>,
}

impl Answer {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// One request at a time to windows-link; blocking, so the caller runs it off the UI thread.
pub trait Http: Send + Sync + 'static {
    /// `None` when nothing answered (windows-link is not running, or the request timed out).
    fn call(&self, method: &str, url: &str, body: Option<&Value>) -> Option<Answer>;
}

/// What the `/events` socket says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketMessage {
    Opened,
    Text(String),
    /// The socket closed or could not open; nothing more comes from this connection.
    Closed,
}

/// Opens the `/events` socket and forwards what it says until it closes.
pub trait Events: Send + Sync + 'static {
    /// Connect to `url` and send its messages to `sink`, ending with [`SocketMessage::Closed`].
    /// Returns a handle that closes the connection when stopped.
    fn open(&self, url: &str, sink: UnboundedSender<SocketMessage>) -> Box<dyn Connection>;
}

/// A connection to stop (the panel closing or reconnecting elsewhere).
pub trait Connection: Send {
    fn stop(&self);
}

/// HTTP to windows-link with ureq.
pub struct Ureq {
    agent: ureq::Agent,
}

impl Default for Ureq {
    fn default() -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .timeout_connect(Some(Duration::from_secs(2)))
            .build()
            .into();
        Self { agent }
    }
}

impl Http for Ureq {
    fn call(&self, method: &str, url: &str, body: Option<&Value>) -> Option<Answer> {
        let result = match (method, body) {
            ("GET", _) => self.agent.get(url).call(),
            ("DELETE", _) => self.agent.delete(url).call(),
            (method, Some(body)) => {
                let request = match method {
                    "PUT" => self.agent.put(url),
                    "PATCH" => self.agent.patch(url),
                    _ => self.agent.post(url),
                };
                request
                    .header("content-type", "application/json")
                    .send(body.to_string())
            }
            ("PUT", None) => self.agent.put(url).send_empty(),
            ("PATCH", None) => self.agent.patch(url).send_empty(),
            (_, None) => self.agent.post(url).send_empty(),
        };
        let mut response = result.ok()?;
        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string().unwrap_or_default();
        Some(Answer {
            status,
            body: serde_json::from_str(&text).ok(),
        })
    }
}

/// The `/events` socket with tungstenite, on a thread of its own.
pub struct Tungstenite;

struct SocketConnection {
    stopped: Arc<std::sync::atomic::AtomicBool>,
}

impl Connection for SocketConnection {
    fn stop(&self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Events for Tungstenite {
    fn open(&self, url: &str, sink: UnboundedSender<SocketMessage>) -> Box<dyn Connection> {
        let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let url = url.to_owned();
        let flag = stopped.clone();
        let spawned = thread::Builder::new()
            .name("events".into())
            .spawn(move || run_socket(&url, &sink, &flag));
        if spawned.is_err() {
            stopped.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Box::new(SocketConnection { stopped })
    }
}

fn run_socket(
    url: &str,
    sink: &UnboundedSender<SocketMessage>,
    stopped: &std::sync::atomic::AtomicBool,
) {
    use std::sync::atomic::Ordering;
    use tungstenite::{Message, client::IntoClientRequest};

    let connect = || -> Option<tungstenite::WebSocket<TcpStream>> {
        let request = url.into_client_request().ok()?;
        let host = request.uri().host()?.to_owned();
        let port = request.uri().port_u16().unwrap_or(80);
        let addrs = std::net::ToSocketAddrs::to_socket_addrs(&(host.as_str(), port)).ok()?;
        let stream = addrs
            .into_iter()
            .find_map(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(2)).ok())?;
        let (socket, _) = tungstenite::client(request, stream).ok()?;
        // Wake up now and then to notice a stop.
        socket
            .get_ref()
            .set_read_timeout(Some(Duration::from_millis(500)))
            .ok()?;
        Some(socket)
    };
    let Some(mut socket) = connect() else {
        let _ = sink.unbounded_send(SocketMessage::Closed);
        return;
    };
    let _ = sink.unbounded_send(SocketMessage::Opened);
    while !stopped.load(Ordering::Relaxed) {
        match socket.read() {
            Ok(Message::Text(text)) => {
                if sink
                    .unbounded_send(SocketMessage::Text(text.to_string()))
                    .is_err()
                {
                    break;
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(tungstenite::Error::Io(err))
                if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
    }
    let _ = socket.close(None);
    let _ = sink.unbounded_send(SocketMessage::Closed);
}
