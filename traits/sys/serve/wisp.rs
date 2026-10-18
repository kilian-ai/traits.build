// WISP v1 server implementation — real TCP egress for v86/Alpine browser guests.
//
// This exists because Cloudflare Workers' `cloudflare:sockets.connect()`
// refuses to reach Cloudflare-hosted IPs from inside a Worker (loopback
// prevention), which breaks `api.openai.com`, `example.com`, and any other
// CF-fronted target. By running the WISP server on the Fly.io backend with
// `tokio::net::TcpStream`, we get unrestricted outbound TCP.
//
// Protocol: https://github.com/MercuryWorkshop/wisp-protocol (v1, and v2
// negotiation without extensions)
// Frame layout: [type:u8][stream_id:u32 LE][payload...]
// Types: 1=CONNECT, 2=DATA, 3=CONTINUE, 4=CLOSE, 5=INFO.
//
// A client offering the `wisp-v2` subprotocol gets it echoed and an INFO
// frame (version 2.0, UDP extension) ahead of the initial CONTINUE: browsers
// abort a WebSocket whose offered subprotocol the server does not select,
// which cost every LinuxOnTab page load a reconnect once its client started
// offering v2. Clients that offer nothing get plain v1 exactly as before.
//
// UDP streams (stream_type=2): one connected UdpSocket per stream, one DATA
// frame per datagram each way, no flow control (v2 says none for UDP), closed
// after 60 s idle. Refusals carry a CLOSE reason so the client can hand the
// guest an ICMP unreachable instead of a timeout: 0x42 when the host does not
// resolve or answers with ICMP (ConnectionRefused on a connected socket),
// 0x48 for loopback / link-local / ULA targets (keeps guests off this host and
// Fly's private network), 0x49 past UDP_STREAMS_PER_CONN.
//
// Routing vs. the Cloudflare relay:
// - Cloudflare `relay.traits.build/wisp` still exists for non-CF targets
//   (fast edge, low-latency).
// - Fly.io `/wisp` handles everything, including CF-fronted targets. v86
//   points `relay_url` here to guarantee correctness.

use actix_web::http::header::{self, HeaderValue};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::Message;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

const WISP_BUFFER: u32 = 1024 * 1024; // per-stream credit window
const WISP_WS_QUEUE_CAP: usize = 2048; // bounded WS outbound frame queue
const WISP_STREAM_QUEUE_CAP: usize = 256; // bounded per-stream client->server queue
const UDP_IDLE: Duration = Duration::from_secs(60); // close a UDP stream idle this long
const UDP_STREAMS_PER_CONN: usize = 64; // open UDP streams per WebSocket
const UDP_MAX_DATAGRAM: usize = 65507;

const T_CONNECT: u8 = 1;
const T_DATA: u8 = 2;
const T_CONTINUE: u8 = 3;
const T_CLOSE: u8 = 4;
const T_INFO: u8 = 5;

const CLOSE_VOLUNTARY: u8 = 0x02;
const CLOSE_NET_ERR: u8 = 0x03;
const CLOSE_INVALID: u8 = 0x41;
const CLOSE_UNREACH: u8 = 0x42;
const CLOSE_BLOCKED: u8 = 0x48;
const CLOSE_THROTTLED: u8 = 0x49;

// Build a WISP frame.
fn encode_frame(frame_type: u8, stream_id: u32, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5 + payload.len());
    buf.push(frame_type);
    buf.extend_from_slice(&stream_id.to_le_bytes());
    buf.extend_from_slice(payload);
    buf
}

fn encode_continue(stream_id: u32, remaining: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(9);
    buf.push(T_CONTINUE);
    buf.extend_from_slice(&stream_id.to_le_bytes());
    buf.extend_from_slice(&remaining.to_le_bytes());
    buf
}

fn encode_close(stream_id: u32, reason: u8) -> Vec<u8> {
    encode_frame(T_CLOSE, stream_id, &[reason])
}

// Outbound WS sender — cloned into each TCP→WS pump task.
type WsSender = mpsc::Sender<Vec<u8>>;

struct StreamState {
    // Channel into the per-stream TCP writer task (or the UDP stream task).
    tx: mpsc::Sender<Vec<u8>>,
    // UDP streams count against UDP_STREAMS_PER_CONN and get no credit.
    udp: bool,
    // Bytes received from client since last CONTINUE grant — when half the
    // initial WISP_BUFFER has been drained to upstream TCP we issue a fresh
    // CONTINUE so the client can keep sending. This is the ONLY direction
    // with credit control in WISP v1: server->client uses raw WS/TCP
    // backpressure (bounded out_tx channel + actix-ws backpressure).
    bytes_since_credit: u64,
    // Notify used during teardown to wake any task that was waiting on this
    // stream (kept for forward compatibility; currently only signaled on
    // close so writer/reader tasks exit cleanly).
    credit_notify: Arc<tokio::sync::Notify>,
}

pub async fn wisp_ws(req: HttpRequest, body: web::Payload) -> actix_web::Result<HttpResponse> {
    let (mut response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;
    // actix-ws selects no subprotocol; pick wisp-v2 ourselves when offered.
    let v2 = req
        .headers()
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.split(',').any(|p| p.trim() == "wisp-v2"))
        .unwrap_or(false);
    if v2 {
        response
            .headers_mut()
            .insert(header::SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static("wisp-v2"));
    }
    let conn_id = uuid::Uuid::new_v4().to_string();
    let conn_id_short = conn_id.split('-').next().unwrap_or("anon").to_string();

    // Outbound pump: one task owns the WS session writer.
    let (out_tx, mut out_rx) = mpsc::channel::<Vec<u8>>(WISP_WS_QUEUE_CAP);
    let mut session_writer = session.clone();
    actix_rt::spawn(async move {
        while let Some(bytes) = out_rx.recv().await {
            if session_writer.binary(bytes).await.is_err() {
                break;
            }
        }
    });

    // v2: INFO(stream_id=0) with version 2.0 and the UDP extension (id 0x01,
    // u32 LE metadata length 0). Then the initial CONTINUE(stream_id=0) that
    // signals server ready on both versions.
    if v2 {
        let _ = out_tx
            .send(encode_frame(T_INFO, 0, &[2, 0, 0x01, 0, 0, 0, 0]))
            .await;
    }
    let _ = out_tx.send(encode_continue(0, WISP_BUFFER)).await;

    let streams: Arc<Mutex<HashMap<u32, StreamState>>> = Arc::new(Mutex::new(HashMap::new()));

    info!("WISP: new tunnel conn_id={} v{}", conn_id_short, if v2 { 2 } else { 1 });

    let streams_inner = streams.clone();
    let out_tx_inner = out_tx.clone();
    let conn_id_for_task = conn_id_short.clone();
    actix_rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.recv().await {
            match msg {
                Message::Text(_) => {}
                Message::Ping(bytes) => {
                    let _ = session.pong(&bytes).await;
                }
                Message::Binary(bytes) => {
                    if bytes.len() < 5 {
                        continue;
                    }
                    let frame_type = bytes[0];
                    let mut sid_bytes = [0u8; 4];
                    sid_bytes.copy_from_slice(&bytes[1..5]);
                    let stream_id = u32::from_le_bytes(sid_bytes);
                    let payload = &bytes[5..];

                    match frame_type {
                        T_CONNECT => {
                            if payload.len() < 3 {
                                let _ = out_tx_inner
                                    .send(encode_close(stream_id, CLOSE_INVALID))
                                    .await;
                                continue;
                            }
                            let stream_type = payload[0];
                            let port = u16::from_le_bytes([payload[1], payload[2]]);
                            let hostname = match std::str::from_utf8(&payload[3..]) {
                                Ok(s) => s.to_string(),
                                Err(_) => {
                                    let _ = out_tx_inner
                                        .send(encode_close(stream_id, CLOSE_INVALID))
                                        .await;
                                    continue;
                                }
                            };
                            let udp = match stream_type {
                                1 => false,
                                2 => true,
                                _ => {
                                    let _ = out_tx_inner
                                        .send(encode_close(stream_id, CLOSE_INVALID))
                                        .await;
                                    continue;
                                }
                            };
                            if port == 0 {
                                let _ = out_tx_inner
                                    .send(encode_close(stream_id, CLOSE_INVALID))
                                    .await;
                                continue;
                            }
                            // Insert stream into map SYNCHRONOUSLY so any
                            // DATA frames arriving while TCP connects are
                            // buffered in this channel, not silently dropped.
                            let (in_tx, in_rx) = mpsc::channel::<Vec<u8>>(WISP_STREAM_QUEUE_CAP);
                            {
                                let mut guard = streams_inner.lock().await;
                                if udp
                                    && guard.values().filter(|s| s.udp).count()
                                        >= UDP_STREAMS_PER_CONN
                                {
                                    drop(guard);
                                    warn!(
                                        "WISP: udp_throttled conn_id={} sid={} {}:{}",
                                        conn_id_for_task, stream_id, hostname, port
                                    );
                                    let _ = out_tx_inner
                                        .send(encode_close(stream_id, CLOSE_THROTTLED))
                                        .await;
                                    continue;
                                }
                                guard.insert(
                                    stream_id,
                                    StreamState {
                                        tx: in_tx,
                                        udp,
                                        bytes_since_credit: 0,
                                        credit_notify: Arc::new(tokio::sync::Notify::new()),
                                    },
                                );
                            }
                            // Spawn the connect + pump tasks.
                            let out = out_tx_inner.clone();
                            let streams_h = streams_inner.clone();
                            let cid = conn_id_for_task.clone();
                            actix_rt::spawn(async move {
                                if udp {
                                    handle_udp(stream_id, hostname, port, out, streams_h, cid, in_rx)
                                        .await;
                                } else {
                                    handle_connect(
                                        stream_id, hostname, port, out, streams_h, cid, in_rx,
                                    )
                                    .await;
                                }
                            });
                        }
                        T_DATA => {
                            let tx = {
                                let guard = streams_inner.lock().await;
                                guard.get(&stream_id).map(|s| s.tx.clone())
                            };
                            if let Some(tx) = tx {
                                // Bounded queue: backpressure applies here if the TCP writer lags.
                                if tx.send(payload.to_vec()).await.is_err() {
                                    let mut guard = streams_inner.lock().await;
                                    if let Some(s) = guard.remove(&stream_id) {
                                        s.credit_notify.notify_waiters();
                                    }
                                    let _ = out_tx_inner
                                        .send(encode_close(stream_id, CLOSE_NET_ERR))
                                        .await;
                                    continue;
                                }
                            }
                        }
                        T_CLOSE => {
                            let mut guard = streams_inner.lock().await;
                            if let Some(s) = guard.remove(&stream_id) {
                                s.credit_notify.notify_waiters();
                            }
                        }
                        T_CONTINUE => {
                            // WISP v1 has client->server credit control only;
                            // clients do not send CONTINUE. Ignore if present.
                        }
                        T_INFO => {
                            // The v2 client's reply to our INFO. It carries no
                            // extension we act on.
                        }
                        _ => {}
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        // WS closed — tear down all streams.
        let mut guard = streams_inner.lock().await;
        for (_, s) in guard.drain() {
            s.credit_notify.notify_waiters();
        }
        info!("WISP: tunnel closed conn_id={}", conn_id_for_task);
    });

    Ok(response)
}

// Targets a public relay must not reach on behalf of a guest: this host, the
// link-local range (cloud metadata) and unique-local/private v6 (Fly's 6PN).
fn udp_target_blocked(addr: &std::net::SocketAddr) -> bool {
    match addr.ip() {
        std::net::IpAddr::V4(ip) => {
            ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || ip.is_multicast()
        }
        std::net::IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (ip.segments()[0] & 0xffc0) == 0xfe80 // link-local
                || (ip.segments()[0] & 0xfe00) == 0xfc00 // unique-local (fc00::/7)
        }
    }
}

enum UdpEv {
    FromClient(Option<Vec<u8>>),
    FromNet(std::io::Result<usize>),
}

async fn handle_udp(
    stream_id: u32,
    hostname: String,
    port: u16,
    out_tx: WsSender,
    streams: Arc<Mutex<HashMap<u32, StreamState>>>,
    conn_id: String,
    mut in_rx: mpsc::Receiver<Vec<u8>>,
) {
    let fail = |reason: u8| {
        let out_tx = out_tx.clone();
        let streams = streams.clone();
        async move {
            streams.lock().await.remove(&stream_id);
            let _ = out_tx.send(encode_close(stream_id, reason)).await;
        }
    };

    // Resolve (5 s cap), preferring IPv4 — most guest targets are v4 and the
    // socket family must match the chosen address.
    let lookup = tokio::net::lookup_host((hostname.as_str(), port));
    let addr = match tokio::time::timeout(Duration::from_secs(5), lookup).await {
        Ok(Ok(addrs)) => {
            let addrs: Vec<_> = addrs.collect();
            addrs
                .iter()
                .find(|a| a.is_ipv4())
                .or_else(|| addrs.first())
                .copied()
        }
        Ok(Err(e)) => {
            warn!(
                "WISP: udp_resolve_fail conn_id={} sid={} {}:{} {}",
                conn_id, stream_id, hostname, port, e
            );
            None
        }
        Err(_) => {
            warn!(
                "WISP: udp_resolve_timeout conn_id={} sid={} {}:{}",
                conn_id, stream_id, hostname, port
            );
            None
        }
    };
    let addr = match addr {
        Some(a) => a,
        None => return fail(CLOSE_UNREACH).await,
    };
    if udp_target_blocked(&addr) {
        warn!(
            "WISP: udp_blocked conn_id={} sid={} {}:{} -> {}",
            conn_id, stream_id, hostname, port, addr
        );
        return fail(CLOSE_BLOCKED).await;
    }

    let bind: std::net::SocketAddr = if addr.is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };
    let sock = match tokio::net::UdpSocket::bind(bind).await {
        Ok(s) => s,
        Err(e) => {
            warn!(
                "WISP: udp_bind_fail conn_id={} sid={} {}",
                conn_id, stream_id, e
            );
            return fail(CLOSE_NET_ERR).await;
        }
    };
    if let Err(e) = sock.connect(addr).await {
        warn!(
            "WISP: udp_connect_fail conn_id={} sid={} {}:{} {}",
            conn_id, stream_id, hostname, port, e
        );
        return fail(CLOSE_UNREACH).await;
    }
    info!(
        "WISP: udp_open conn_id={} sid={} {}:{} -> {}",
        conn_id, stream_id, hostname, port, addr
    );

    // One task drives both directions; a connected UdpSocket is &self for
    // send and recv, so no split is needed.
    let mut buf = vec![0u8; UDP_MAX_DATAGRAM];
    let reason = loop {
        let ev = tokio::time::timeout(UDP_IDLE, async {
            tokio::select! {
                m = in_rx.recv() => UdpEv::FromClient(m),
                r = sock.recv(&mut buf) => UdpEv::FromNet(r),
            }
        })
        .await;
        match ev {
            Err(_) => break Some(CLOSE_VOLUNTARY), // idle
            Ok(UdpEv::FromClient(None)) => break None, // client CLOSE / WS gone
            Ok(UdpEv::FromClient(Some(d))) => {
                if d.len() > UDP_MAX_DATAGRAM {
                    continue;
                }
                if let Err(e) = sock.send(&d).await {
                    warn!(
                        "WISP: udp_send_err conn_id={} sid={} {}:{} {}",
                        conn_id, stream_id, hostname, port, e
                    );
                    break Some(if e.kind() == std::io::ErrorKind::ConnectionRefused {
                        CLOSE_UNREACH
                    } else {
                        CLOSE_NET_ERR
                    });
                }
            }
            Ok(UdpEv::FromNet(Ok(n))) => {
                let frame = encode_frame(T_DATA, stream_id, &buf[..n]);
                if out_tx.send(frame).await.is_err() {
                    break None;
                }
            }
            Ok(UdpEv::FromNet(Err(e))) => {
                // ICMP port unreachable surfaces here on a connected socket.
                break Some(if e.kind() == std::io::ErrorKind::ConnectionRefused {
                    CLOSE_UNREACH
                } else {
                    CLOSE_NET_ERR
                });
            }
        }
    };
    streams.lock().await.remove(&stream_id);
    if let Some(r) = reason {
        let _ = out_tx.send(encode_close(stream_id, r)).await;
    }
}

async fn handle_connect(
    stream_id: u32,
    hostname: String,
    port: u16,
    out_tx: WsSender,
    streams: Arc<Mutex<HashMap<u32, StreamState>>>,
    conn_id: String,
    mut in_rx: mpsc::Receiver<Vec<u8>>,
) {
    // Bounded connect timeout (5s) to avoid piling up hung sockets.
    let connect_fut = TcpStream::connect((hostname.as_str(), port));
    let tcp = match tokio::time::timeout(Duration::from_secs(5), connect_fut).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            warn!(
                "WISP: connect_fail conn_id={} sid={} {}:{} {}",
                conn_id, stream_id, hostname, port, e
            );
            streams.lock().await.remove(&stream_id);
            let _ = out_tx.send(encode_close(stream_id, CLOSE_UNREACH)).await;
            return;
        }
        Err(_) => {
            warn!(
                "WISP: connect_timeout conn_id={} sid={} {}:{}",
                conn_id, stream_id, hostname, port
            );
            streams.lock().await.remove(&stream_id);
            let _ = out_tx.send(encode_close(stream_id, CLOSE_UNREACH)).await;
            return;
        }
    };
    let _ = tcp.set_nodelay(true);

    let (mut rd, mut wr) = tcp.into_split();

    // Flow credit so client knows connect succeeded and can send more DATA.
    let _ = out_tx.send(encode_continue(stream_id, WISP_BUFFER)).await;
    info!(
        "WISP: connect_ok conn_id={} sid={} {}:{}",
        conn_id, stream_id, hostname, port
    );

    // Writer: channel → TCP.
    let host_w = hostname.clone();
    let port_w = port;
    let conn_id_w = conn_id.clone();
    let out_tx_w = out_tx.clone();
    let streams_w = streams.clone();
    actix_rt::spawn(async move {
        while let Some(chunk) = in_rx.recv().await {
            if let Err(e) = wr.write_all(&chunk).await {
                warn!(
                    "WISP: writer_err conn_id={} sid={} {}:{} {}",
                    conn_id_w, stream_id, host_w, port_w, e
                );
                let _ = out_tx_w.send(encode_close(stream_id, CLOSE_NET_ERR)).await;
                streams_w.lock().await.remove(&stream_id);
                return;
            }

            // Replenish client->server credit only after bytes are actually drained to TCP.
            let mut grant = false;
            {
                let mut guard = streams_w.lock().await;
                if let Some(s) = guard.get_mut(&stream_id) {
                    s.bytes_since_credit = s.bytes_since_credit.saturating_add(chunk.len() as u64);
                    if s.bytes_since_credit >= (WISP_BUFFER as u64 / 2) {
                        s.bytes_since_credit = 0;
                        grant = true;
                    }
                } else {
                    return;
                }
            }
            if grant {
                let _ = out_tx_w.send(encode_continue(stream_id, WISP_BUFFER)).await;
            }
        }
        // Channel closed (stream removed) — voluntary shutdown.
        let _ = wr.shutdown().await;
    });

    // Reader: TCP → WS DATA frames.
    let host_r = hostname;
    let port_r = port;
    let conn_id_r = conn_id;
    actix_rt::spawn(async move {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            match rd.read(&mut buf).await {
                Ok(0) => {
                    // EOF from upstream. CRITICAL: must .await — otherwise the
                    // CLOSE frame is never sent and the v86 guest's TCP stack
                    // waits forever for data that won't come (manifests as
                    // `apk fetch` / HTTP `Connection: close` stalls that
                    // "work after a few retries").
                    let _ = out_tx.send(encode_close(stream_id, CLOSE_VOLUNTARY)).await;
                    streams.lock().await.remove(&stream_id);
                    return;
                }
                Ok(n) => {
                    // WISP v1 has no server->client credit control; rely on
                    // bounded `out_tx` channel + actix-ws backpressure to
                    // throttle when the client (or WS link) is slow.
                    let frame = encode_frame(T_DATA, stream_id, &buf[..n]);
                    if out_tx.send(frame).await.is_err() {
                        streams.lock().await.remove(&stream_id);
                        return;
                    }
                }
                Err(e) => {
                    warn!(
                        "WISP: reader_err conn_id={} sid={} {}:{} {}",
                        conn_id_r, stream_id, host_r, port_r, e
                    );
                    let _ = out_tx.send(encode_close(stream_id, CLOSE_NET_ERR)).await;
                    streams.lock().await.remove(&stream_id);
                    return;
                }
            }
        }
    });
}

// Lightweight liveness/debug endpoint for the WISP server.
pub async fn wisp_debug() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "server": "traits-build/sys.serve/wisp",
        "protocol": "wisp-v1, wisp-v2 with the UDP extension",
        "transport": "tokio::net::TcpStream + UdpSocket",
        "udp_supported": true,
        "udp_streams_per_conn": UDP_STREAMS_PER_CONN,
        "udp_idle_secs": UDP_IDLE.as_secs(),
        "note": "Real TCP egress — bypasses Cloudflare Worker loopback restriction.",
    }))
}
