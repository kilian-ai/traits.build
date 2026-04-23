// WISP v1 server implementation — real TCP egress for v86/Alpine browser guests.
//
// This exists because Cloudflare Workers' `cloudflare:sockets.connect()`
// refuses to reach Cloudflare-hosted IPs from inside a Worker (loopback
// prevention), which breaks `api.openai.com`, `example.com`, and any other
// CF-fronted target. By running the WISP server on the Fly.io backend with
// `tokio::net::TcpStream`, we get unrestricted outbound TCP.
//
// Protocol: https://github.com/MercuryWorkshop/wisp-protocol (v1)
// Frame layout: [type:u8][stream_id:u32 LE][payload...]
// Types: 1=CONNECT, 2=DATA, 3=CONTINUE, 4=CLOSE. UDP (stream_type=2) is not
// supported here — rejected with 0x41.
//
// Routing vs. the Cloudflare relay:
// - Cloudflare `relay.traits.build/wisp` still exists for non-CF targets
//   (fast edge, low-latency).
// - Fly.io `/wisp` handles everything, including CF-fronted targets. v86
//   points `relay_url` here to guarantee correctness.

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

const T_CONNECT: u8 = 1;
const T_DATA: u8 = 2;
const T_CONTINUE: u8 = 3;
const T_CLOSE: u8 = 4;

const CLOSE_VOLUNTARY: u8 = 0x02;
const CLOSE_NET_ERR: u8 = 0x03;
const CLOSE_INVALID: u8 = 0x41;
const CLOSE_UNREACH: u8 = 0x42;

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
    // Channel into the per-stream TCP writer task.
    tx: mpsc::Sender<Vec<u8>>,
    bytes_since_credit: u64,
    // Server->client credit window (granted by client CONTINUE frames).
    outbound_credit: u64,
    credit_notify: Arc<tokio::sync::Notify>,
    credits_issued: u64,
    credits_received: u64,
}

pub async fn wisp_ws(req: HttpRequest, body: web::Payload) -> actix_web::Result<HttpResponse> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;
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

    // Initial CONTINUE(stream_id=0) — signals v1 server ready.
    let _ = out_tx.send(encode_continue(0, WISP_BUFFER)).await;

    let streams: Arc<Mutex<HashMap<u32, StreamState>>> = Arc::new(Mutex::new(HashMap::new()));

    info!("WISP: new tunnel conn_id={}", conn_id_short);

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
                            if stream_type != 1 {
                                // UDP not supported.
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
                                guard.insert(
                                    stream_id,
                                    StreamState {
                                        tx: in_tx,
                                        bytes_since_credit: 0,
                                        outbound_credit: WISP_BUFFER as u64,
                                        credit_notify: Arc::new(tokio::sync::Notify::new()),
                                        credits_issued: WISP_BUFFER as u64,
                                        credits_received: 0,
                                    },
                                );
                            }
                            // Spawn the TCP connect + reader/writer tasks.
                            let out = out_tx_inner.clone();
                            let streams_h = streams_inner.clone();
                            let cid = conn_id_for_task.clone();
                            actix_rt::spawn(async move {
                                handle_connect(
                                    stream_id, hostname, port, out, streams_h, cid, in_rx,
                                )
                                .await;
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
                            if payload.len() >= 4 {
                                let grant = u32::from_le_bytes([
                                    payload[0], payload[1], payload[2], payload[3],
                                ]) as u64;
                                let notify = {
                                    let mut guard = streams_inner.lock().await;
                                    if let Some(s) = guard.get_mut(&stream_id) {
                                        s.outbound_credit = s.outbound_credit.saturating_add(grant);
                                        s.credits_received = s.credits_received.saturating_add(grant);
                                        Some(s.credit_notify.clone())
                                    } else {
                                        None
                                    }
                                };
                                if let Some(n) = notify {
                                    n.notify_waiters();
                                }
                            }
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
    {
        let mut guard = streams.lock().await;
        if let Some(s) = guard.get_mut(&stream_id) {
            s.credits_issued = s.credits_issued.saturating_add(WISP_BUFFER as u64);
        }
    }
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
                        s.credits_issued = s.credits_issued.saturating_add(WISP_BUFFER as u64);
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
                    let _ = out_tx.send(encode_close(stream_id, CLOSE_VOLUNTARY));
                    streams.lock().await.remove(&stream_id);
                    return;
                }
                Ok(n) => {
                    // Respect outbound credit (granted by client CONTINUE).
                    // We wait for enough credit to send this frame, otherwise close on timeout.
                    loop {
                        let notify = {
                            let mut guard = streams.lock().await;
                            if let Some(s) = guard.get_mut(&stream_id) {
                                let need = n as u64;
                                if s.outbound_credit >= need {
                                    s.outbound_credit -= need;
                                    None
                                } else {
                                    Some(s.credit_notify.clone())
                                }
                            } else {
                                return;
                            }
                        };

                        let Some(notify) = notify else { break };
                        if tokio::time::timeout(Duration::from_secs(30), notify.notified())
                            .await
                            .is_err()
                        {
                            warn!(
                                "WISP: outbound_credit_timeout conn_id={} sid={} {}:{}",
                                conn_id_r, stream_id, host_r, port_r
                            );
                            let _ = out_tx.send(encode_close(stream_id, CLOSE_NET_ERR)).await;
                            streams.lock().await.remove(&stream_id);
                            return;
                        }
                    }

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
        "protocol": "wisp-v1",
        "transport": "tokio::net::TcpStream",
        "udp_supported": false,
        "note": "Real TCP egress — bypasses Cloudflare Worker loopback restriction.",
    }))
}
