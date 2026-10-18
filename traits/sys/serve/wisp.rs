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
// Per-client limits (see `limits`): concurrent WebSockets (HTTP 429 past the
// cap), stream-open rate and concurrent UDP streams (CLOSE 0x49), and an
// outbound UDP packet + byte budget (excess datagrams dropped, as a congested
// path would). A client is its IPv4 address or IPv6 /64; on Fly it comes from
// Fly-Client-IP, which the edge sets. All limits take WISP_* env overrides.
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

mod limits {
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv6Addr};
    use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
    use std::time::{Duration, Instant};

    pub struct Config {
        pub max_conns: usize,
        pub opens_per_sec: f64,
        pub opens_burst: f64,
        pub max_udp_streams: usize,
        pub udp_pps: f64,
        pub udp_pps_burst: f64,
        pub udp_bps: f64,
        pub udp_bps_burst: f64,
    }

    fn env_num(name: &str, default: f64) -> f64 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| *v > 0.0)
            .unwrap_or(default)
    }

    // Generous on purpose: one person's tabs-as-containers stack is a socket
    // per tab, and offices / mobile carriers put many people behind one IPv4.
    pub fn config() -> &'static Config {
        static CFG: OnceLock<Config> = OnceLock::new();
        CFG.get_or_init(|| Config {
            max_conns: env_num("WISP_MAX_CONNS_PER_IP", 64.0) as usize,
            opens_per_sec: env_num("WISP_OPENS_PER_SEC", 20.0),
            opens_burst: env_num("WISP_OPENS_BURST", 200.0),
            max_udp_streams: env_num("WISP_MAX_UDP_STREAMS_PER_IP", 256.0) as usize,
            udp_pps: env_num("WISP_UDP_PPS", 500.0),
            udp_pps_burst: env_num("WISP_UDP_PPS_BURST", 1000.0),
            udp_bps: env_num("WISP_UDP_BYTES_PER_SEC", 1048576.0),
            udp_bps_burst: env_num("WISP_UDP_BYTES_BURST", 2097152.0),
        })
    }

    struct Bucket {
        tokens: f64,
        last: Instant,
    }

    impl Bucket {
        fn new(burst: f64) -> Self {
            Bucket { tokens: burst, last: Instant::now() }
        }
        fn take(&mut self, n: f64, rate: f64, burst: f64) -> bool {
            let now = Instant::now();
            self.tokens = (self.tokens + now.duration_since(self.last).as_secs_f64() * rate).min(burst);
            self.last = now;
            if self.tokens >= n {
                self.tokens -= n;
                true
            } else {
                false
            }
        }
    }

    struct State {
        conns: usize,
        udp_streams: usize,
        opens: Bucket,
        udp_pkts: Bucket,
        udp_bytes: Bucket,
        last_seen: Instant,
    }

    type Entry = Arc<Mutex<State>>;

    struct Table {
        map: HashMap<IpAddr, Entry>,
        last_prune: Instant,
    }

    fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
        m.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn table() -> &'static Mutex<Table> {
        static T: OnceLock<Mutex<Table>> = OnceLock::new();
        T.get_or_init(|| Mutex::new(Table { map: HashMap::new(), last_prune: Instant::now() }))
    }

    /// The address a client is limited by: IPv4 as is, IPv6 by its /64 (one
    /// subscriber's allocation, so rotating addresses inside it doesn't help).
    pub fn key_for(ip: IpAddr) -> IpAddr {
        match ip {
            IpAddr::V4(_) => ip,
            IpAddr::V6(v6) => {
                if let Some(v4) = v6.to_ipv4_mapped() {
                    return IpAddr::V4(v4);
                }
                let s = v6.segments();
                IpAddr::V6(Ipv6Addr::new(s[0], s[1], s[2], s[3], 0, 0, 0, 0))
            }
        }
    }

    pub fn tracked() -> usize {
        lock(table()).map.len()
    }

    fn entry(key: IpAddr) -> Entry {
        let mut t = lock(table());
        // Keep recently seen clients so reconnecting doesn't refill their
        // buckets; drop idle ones (no live socket/stream holds the Arc).
        if t.last_prune.elapsed() > Duration::from_secs(60) {
            t.last_prune = Instant::now();
            let cutoff = Duration::from_secs(600);
            t.map.retain(|_, e| Arc::strong_count(e) > 1 || lock(e).last_seen.elapsed() < cutoff);
        }
        let c = config();
        t.map
            .entry(key)
            .or_insert_with(|| {
                Arc::new(Mutex::new(State {
                    conns: 0,
                    udp_streams: 0,
                    opens: Bucket::new(c.opens_burst),
                    udp_pkts: Bucket::new(c.udp_pps_burst),
                    udp_bytes: Bucket::new(c.udp_bps_burst),
                    last_seen: Instant::now(),
                }))
            })
            .clone()
    }

    /// One admitted WebSocket; dropping it frees the slot.
    pub struct Conn {
        pub key: IpAddr,
        st: Entry,
    }

    impl Drop for Conn {
        fn drop(&mut self) {
            let mut s = lock(&self.st);
            s.conns = s.conns.saturating_sub(1);
            s.last_seen = Instant::now();
        }
    }

    pub fn admit(key: IpAddr) -> Option<Conn> {
        let st = entry(key);
        {
            let mut s = lock(&st);
            s.last_seen = Instant::now();
            if s.conns >= config().max_conns {
                return None;
            }
            s.conns += 1;
        }
        Some(Conn { key, st })
    }

    /// One open UDP stream; dropping it frees the slot.
    pub struct UdpSlot {
        st: Entry,
    }

    impl Drop for UdpSlot {
        fn drop(&mut self) {
            let mut s = lock(&self.st);
            s.udp_streams = s.udp_streams.saturating_sub(1);
            s.last_seen = Instant::now();
        }
    }

    impl Conn {
        /// Admit a new stream. TCP gets Ok(None), UDP Ok(Some(slot)); Err names
        /// the limit that refused it.
        pub fn open(&self, udp: bool) -> Result<Option<UdpSlot>, &'static str> {
            let c = config();
            let mut s = lock(&self.st);
            s.last_seen = Instant::now();
            if udp && s.udp_streams >= c.max_udp_streams {
                return Err("udp streams per ip");
            }
            if !s.opens.take(1.0, c.opens_per_sec, c.opens_burst) {
                return Err("open rate per ip");
            }
            if !udp {
                return Ok(None);
            }
            s.udp_streams += 1;
            drop(s);
            Ok(Some(UdpSlot { st: self.st.clone() }))
        }
    }

    impl UdpSlot {
        /// Whether one more outbound datagram of `len` bytes fits the client's
        /// packet and byte budgets.
        pub fn allow_send(&self, len: usize) -> bool {
            let c = config();
            let mut s = lock(&self.st);
            s.udp_pkts.take(1.0, c.udp_pps, c.udp_pps_burst)
                && s.udp_bytes.take(len as f64, c.udp_bps, c.udp_bps_burst)
        }
    }
}

// The client's address. Behind Fly's proxy the peer is the proxy, and the edge
// sets Fly-Client-IP; anywhere else that header is client-controlled, so the
// peer address is used.
fn client_key(req: &HttpRequest) -> std::net::IpAddr {
    let fly = if std::env::var_os("FLY_APP_NAME").is_some() {
        req.headers()
            .get("fly-client-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse().ok())
    } else {
        None
    };
    let ip = fly
        .or_else(|| req.peer_addr().map(|a| a.ip()))
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
    limits::key_for(ip)
}

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
    let key = client_key(&req);
    let conn = match limits::admit(key) {
        Some(c) => c,
        None => {
            warn!("WISP: conn_limit ip={}", key);
            return Ok(HttpResponse::TooManyRequests()
                .body("too many WISP connections from this address"));
        }
    };
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

    info!(
        "WISP: new tunnel conn_id={} ip={} v{}",
        conn_id_short,
        key,
        if v2 { 2 } else { 1 }
    );

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
                            let slot = match conn.open(udp) {
                                Ok(slot) => slot,
                                Err(why) => {
                                    warn!(
                                        "WISP: throttled ({}) conn_id={} ip={} sid={} {}:{}",
                                        why, conn_id_for_task, conn.key, stream_id, hostname, port
                                    );
                                    let _ = out_tx_inner
                                        .send(encode_close(stream_id, CLOSE_THROTTLED))
                                        .await;
                                    continue;
                                }
                            };
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
                                match slot {
                                    Some(slot) => {
                                        handle_udp(
                                            stream_id, hostname, port, out, streams_h, cid, in_rx,
                                            slot,
                                        )
                                        .await
                                    }
                                    None => {
                                        handle_connect(
                                            stream_id, hostname, port, out, streams_h, cid, in_rx,
                                        )
                                        .await
                                    }
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
    slot: limits::UdpSlot,
) {
    let mut dropped: u64 = 0;
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
                if !slot.allow_send(d.len()) {
                    if dropped == 0 {
                        warn!(
                            "WISP: udp_rate_limited conn_id={} sid={} {}:{}",
                            conn_id, stream_id, hostname, port
                        );
                    }
                    dropped += 1;
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
    if dropped > 0 {
        warn!(
            "WISP: udp_rate_dropped conn_id={} sid={} {}:{} datagrams={}",
            conn_id, stream_id, hostname, port, dropped
        );
    }
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
pub async fn wisp_debug(req: HttpRequest) -> HttpResponse {
    let c = limits::config();
    HttpResponse::Ok().json(serde_json::json!({
        "client": client_key(&req).to_string(),
        "limits_per_client": {
            "max_conns": c.max_conns,
            "opens_per_sec": c.opens_per_sec,
            "opens_burst": c.opens_burst,
            "max_udp_streams": c.max_udp_streams,
            "udp_pps": c.udp_pps,
            "udp_pps_burst": c.udp_pps_burst,
            "udp_bytes_per_sec": c.udp_bps,
            "udp_bytes_burst": c.udp_bps_burst,
        },
        "tracked_clients": limits::tracked(),
        "server": "traits-build/sys.serve/wisp",
        "protocol": "wisp-v1, wisp-v2 with the UDP extension",
        "transport": "tokio::net::TcpStream + UdpSocket",
        "udp_supported": true,
        "udp_streams_per_conn": UDP_STREAMS_PER_CONN,
        "udp_idle_secs": UDP_IDLE.as_secs(),
        "note": "Real TCP egress — bypasses Cloudflare Worker loopback restriction.",
    }))
}
