// SPDX-License-Identifier: GPL-2.0-only
// net-proxy.js — JavaScript TCP/IP proxy for Linux/WASM networking.
//
// Bridges raw IP packets from the guest kernel to browser APIs (fetch, WebSocket).
// Runs on the main thread. Worker calls NetProxy.send/recv/poll via postMessage.
//
// Architecture:
//   Guest TCP/IP stack ↔ net_wasm.c driver ↔ Worker host callbacks
//   ↔ postMessage ↔ this proxy ↔ browser fetch() / WebSocket
//
// Network config: Guest 10.0.0.2/24, Gateway 10.0.0.1

const NetProxy = (() => {
  // ── Configuration ──
  const GUEST_IP = [10, 0, 0, 2];
  const GATEWAY_IP = [10, 0, 0, 1];
  const GATEWAY_MAC = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
  const MTU = 1500;

  // ── Receive queue (packets waiting to be read by guest) ──
  const rxQueue = [];

  // ── Connection tracking ──
  const connections = new Map();  // "srcPort:dstIp:dstPort" → TcpConnection
  let nextEphemeralPort = 40000;

  // ── Stats ──
  let stats = { txPackets: 0, rxPackets: 0, txBytes: 0, rxBytes: 0, connections: 0 };

  // ── WebSocket tunnel (optional, for real TCP) ──
  let tunnelUrl = null;
  let tunnelWs = null;

  // ── IP packet helpers ──

  function ipChecksum(header) {
    let sum = 0;
    for (let i = 0; i < header.length; i += 2) {
      sum += (header[i] << 8) | (header[i + 1] || 0);
    }
    while (sum >> 16) sum = (sum & 0xffff) + (sum >> 16);
    return (~sum) & 0xffff;
  }

  function parseIpPacket(buf) {
    if (buf.length < 20) return null;
    const version = (buf[0] >> 4) & 0xf;
    if (version !== 4) return null;
    const ihl = (buf[0] & 0xf) * 4;
    const totalLen = (buf[2] << 8) | buf[3];
    const protocol = buf[9];
    const srcIp = [buf[12], buf[13], buf[14], buf[15]];
    const dstIp = [buf[16], buf[17], buf[18], buf[19]];
    const payload = buf.slice(ihl, totalLen);
    const id = (buf[4] << 8) | buf[5];
    return { version, ihl, totalLen, protocol, srcIp, dstIp, payload, id };
  }

  function buildIpPacket(protocol, srcIp, dstIp, payload, id) {
    const totalLen = 20 + payload.length;
    const pkt = new Uint8Array(totalLen);
    pkt[0] = 0x45;  // IPv4, IHL=5
    pkt[1] = 0;     // DSCP/ECN
    pkt[2] = (totalLen >> 8) & 0xff;
    pkt[3] = totalLen & 0xff;
    pkt[4] = ((id || 0) >> 8) & 0xff;
    pkt[5] = (id || 0) & 0xff;
    pkt[6] = 0x40;  // Don't fragment
    pkt[7] = 0;
    pkt[8] = 64;    // TTL
    pkt[9] = protocol;
    // Checksum placeholder (bytes 10-11) = 0
    pkt[12] = srcIp[0]; pkt[13] = srcIp[1]; pkt[14] = srcIp[2]; pkt[15] = srcIp[3];
    pkt[16] = dstIp[0]; pkt[17] = dstIp[1]; pkt[18] = dstIp[2]; pkt[19] = dstIp[3];
    const cksum = ipChecksum(pkt.slice(0, 20));
    pkt[10] = (cksum >> 8) & 0xff;
    pkt[11] = cksum & 0xff;
    pkt.set(payload, 20);
    return pkt;
  }

  // ── TCP helpers ──

  function parseTcp(payload) {
    if (payload.length < 20) return null;
    const srcPort = (payload[0] << 8) | payload[1];
    const dstPort = (payload[2] << 8) | payload[3];
    const seq = ((payload[4] << 24) | (payload[5] << 16) | (payload[6] << 8) | payload[7]) >>> 0;
    const ack = ((payload[8] << 24) | (payload[9] << 16) | (payload[10] << 8) | payload[11]) >>> 0;
    const dataOffset = ((payload[12] >> 4) & 0xf) * 4;
    const flags = payload[13];
    const window = (payload[14] << 8) | payload[15];
    const data = payload.slice(dataOffset);
    return { srcPort, dstPort, seq, ack, dataOffset, flags, window, data };
  }

  function buildTcpPacket(srcPort, dstPort, seq, ack, flags, data, srcIp, dstIp) {
    const headerLen = 20;
    const segment = new Uint8Array(headerLen + (data ? data.length : 0));
    segment[0] = (srcPort >> 8) & 0xff;
    segment[1] = srcPort & 0xff;
    segment[2] = (dstPort >> 8) & 0xff;
    segment[3] = dstPort & 0xff;
    segment[4] = (seq >>> 24) & 0xff;
    segment[5] = (seq >>> 16) & 0xff;
    segment[6] = (seq >>> 8) & 0xff;
    segment[7] = seq & 0xff;
    segment[8] = (ack >>> 24) & 0xff;
    segment[9] = (ack >>> 16) & 0xff;
    segment[10] = (ack >>> 8) & 0xff;
    segment[11] = ack & 0xff;
    segment[12] = (headerLen / 4) << 4;
    segment[13] = flags;
    segment[14] = 0xff; // window high
    segment[15] = 0xff; // window low
    // Checksum placeholder (bytes 16-17) = 0
    if (data && data.length > 0) segment.set(data, headerLen);

    // TCP pseudo-header checksum
    const pseudo = new Uint8Array(12 + segment.length);
    pseudo.set(srcIp, 0);
    pseudo.set(dstIp, 4);
    pseudo[8] = 0;
    pseudo[9] = 6;  // TCP
    pseudo[10] = (segment.length >> 8) & 0xff;
    pseudo[11] = segment.length & 0xff;
    pseudo.set(segment, 12);
    const cksum = ipChecksum(pseudo);
    segment[16] = (cksum >> 8) & 0xff;
    segment[17] = cksum & 0xff;
    return segment;
  }

  // TCP flags
  const FIN = 0x01, SYN = 0x02, RST = 0x04, PSH = 0x08, ACK = 0x10;

  // ── UDP helpers ──

  function parseUdp(payload) {
    if (payload.length < 8) return null;
    const srcPort = (payload[0] << 8) | payload[1];
    const dstPort = (payload[2] << 8) | payload[3];
    const length = (payload[4] << 8) | payload[5];
    const data = payload.slice(8, length);
    return { srcPort, dstPort, length, data };
  }

  function buildUdpPacket(srcPort, dstPort, data, srcIp, dstIp) {
    const length = 8 + data.length;
    const segment = new Uint8Array(length);
    segment[0] = (srcPort >> 8) & 0xff;
    segment[1] = srcPort & 0xff;
    segment[2] = (dstPort >> 8) & 0xff;
    segment[3] = dstPort & 0xff;
    segment[4] = (length >> 8) & 0xff;
    segment[5] = length & 0xff;
    // Checksum optional for IPv4 UDP, leave as 0
    segment.set(data, 8);
    return segment;
  }

  // ── ICMP handler (ping reply) ──

  function handleIcmp(ipPkt) {
    const payload = ipPkt.payload;
    if (payload.length < 8) return;
    const type = payload[0];
    if (type !== 8) return;  // Only handle echo request
    // Build echo reply
    const reply = new Uint8Array(payload);
    reply[0] = 0;  // Echo reply
    reply[2] = 0; reply[3] = 0;  // Clear checksum
    const cksum = ipChecksum(reply);
    reply[2] = (cksum >> 8) & 0xff;
    reply[3] = cksum & 0xff;
    const pkt = buildIpPacket(1, ipPkt.dstIp, ipPkt.srcIp, reply, ipPkt.id);
    enqueueRx(pkt);
  }

  // ── DNS resolver (Cloudflare DoH) ──

  function buildDnsQuery(hostname) {
    const id = (Math.random() * 0xffff) | 0;
    const parts = hostname.split('.');
    let qname = [];
    for (const part of parts) {
      qname.push(part.length);
      for (let i = 0; i < part.length; i++) qname.push(part.charCodeAt(i));
    }
    qname.push(0);
    // Header: ID, flags=0x0100 (RD), QDCOUNT=1
    const header = [
      (id >> 8) & 0xff, id & 0xff,
      0x01, 0x00,  // flags: RD
      0x00, 0x01,  // QDCOUNT
      0x00, 0x00,  // ANCOUNT
      0x00, 0x00,  // NSCOUNT
      0x00, 0x00,  // ARCOUNT
    ];
    // Question: QNAME + QTYPE (A=1) + QCLASS (IN=1)
    const question = [...qname, 0x00, 0x01, 0x00, 0x01];
    return { id, data: new Uint8Array([...header, ...question]) };
  }

  async function resolveDns(query, srcPort, srcIp) {
    try {
      const resp = await fetch('https://cloudflare-dns.com/dns-query', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/dns-message',
          'Accept': 'application/dns-message',
        },
        body: query,
      });
      if (!resp.ok) return;
      const answer = new Uint8Array(await resp.arrayBuffer());
      // Send DNS response back as UDP packet
      const udpPayload = buildUdpPacket(53, srcPort, answer, GATEWAY_IP, srcIp);
      const pkt = buildIpPacket(17, GATEWAY_IP, srcIp, udpPayload);
      enqueueRx(pkt);
    } catch (e) {
      console.warn('[net-proxy] DNS resolution failed:', e);
    }
  }

  // ── TCP Connection State Machine ──

  class TcpConnection {
    constructor(srcPort, dstIp, dstPort) {
      this.srcPort = srcPort;
      this.dstIp = dstIp;
      this.dstPort = dstPort;
      this.state = 'SYN_RECEIVED';
      this.guestSeq = 0;     // Last seq from guest
      this.ourSeq = (Math.random() * 0x7fffffff) | 0;
      this.rxBuffer = [];     // Data to send back to guest
      this.httpRequest = '';   // Accumulate HTTP request
      this.sentFin = false;
      stats.connections++;
    }

    get key() {
      return `${this.srcPort}:${this.dstIp.join('.')}:${this.dstPort}`;
    }

    handlePacket(tcp, ipPkt) {
      const { flags, seq, ack, data } = tcp;

      if (flags & RST) {
        this.close();
        return;
      }

      switch (this.state) {
        case 'SYN_RECEIVED':
          // Guest sent SYN, we send SYN+ACK
          this.guestSeq = seq + 1;
          this.sendTcp(SYN | ACK, null);
          this.ourSeq++;
          this.state = 'ESTABLISHED';
          break;

        case 'ESTABLISHED':
          if (flags & FIN) {
            this.guestSeq = seq + (data.length || 0) + 1;
            this.sendTcp(ACK, null);
            this.sendTcp(FIN | ACK, null);
            this.ourSeq++;
            this.state = 'LAST_ACK';
            return;
          }
          if (data.length > 0) {
            this.guestSeq = seq + data.length;
            this.sendTcp(ACK, null);
            this.handleData(data);
          }
          break;

        case 'FIN_WAIT':
          if (flags & ACK) {
            this.state = 'CLOSED';
            this.close();
          }
          break;

        case 'LAST_ACK':
          if (flags & ACK) {
            this.state = 'CLOSED';
            this.close();
          }
          break;
      }
    }

    handleData(data) {
      // Accumulate data and check if we have a complete HTTP request
      const text = new TextDecoder().decode(data);
      this.httpRequest += text;

      // Check for end of HTTP headers
      if (this.httpRequest.includes('\r\n\r\n')) {
        this.processHttpRequest(this.httpRequest);
        this.httpRequest = '';
      }
    }

    async processHttpRequest(requestText) {
      const lines = requestText.split('\r\n');
      const [method, path] = (lines[0] || '').split(' ');
      const headers = {};
      let host = '';
      for (let i = 1; i < lines.length; i++) {
        if (!lines[i]) break;
        const [key, ...val] = lines[i].split(': ');
        if (key) headers[key.toLowerCase()] = val.join(': ');
        if (key.toLowerCase() === 'host') host = val.join(': ');
      }

      if (!host) {
        host = this.dstIp.join('.');
      }

      const scheme = this.dstPort === 443 ? 'https' : 'http';
      const url = `${scheme}://${host}${path}`;

      // The Linux page is served over HTTPS, so browser fetch() blocks plain HTTP
      // as mixed content. Prefer HTTPS for guest HTTP requests when possible.
      const candidateUrls = [];
      if (scheme === 'http' && typeof location !== 'undefined' && location.protocol === 'https:') {
        candidateUrls.push(`https://${host}${path}`);
      }
      candidateUrls.push(url);

      try {
        const fetchOpts = { method, headers: {} };
        // Forward safe headers
        if (headers['accept']) fetchOpts.headers['Accept'] = headers['accept'];
        if (headers['content-type']) fetchOpts.headers['Content-Type'] = headers['content-type'];
        if (headers['user-agent']) fetchOpts.headers['User-Agent'] = headers['user-agent'];

        let resp = null;
        let lastErr = null;
        let fetchedUrl = candidateUrls[0];
        for (const candidate of candidateUrls) {
          try {
            fetchedUrl = candidate;
            resp = await fetch(candidate, fetchOpts);
            break;
          } catch (err) {
            lastErr = err;
          }
        }
        if (!resp) throw lastErr || new Error('fetch failed');
        const bodyBytes = new Uint8Array(await resp.arrayBuffer());

        // Build HTTP response
        let statusLine = `HTTP/1.1 ${resp.status} ${resp.statusText}\r\n`;
        let respHeaders = '';
        respHeaders += `Content-Length: ${bodyBytes.length}\r\n`;
        respHeaders += `Connection: close\r\n`;
        const ct = resp.headers.get('content-type');
        if (ct) respHeaders += `Content-Type: ${ct}\r\n`;
        respHeaders += '\r\n';

        const headerBytes = new TextEncoder().encode(statusLine + respHeaders);
        const fullResp = new Uint8Array(headerBytes.length + bodyBytes.length);
        fullResp.set(headerBytes);
        fullResp.set(bodyBytes, headerBytes.length);

        // Send response data in MTU-sized chunks
        this.sendDataChunked(fullResp);
      } catch (e) {
        console.warn('[net-proxy] fetch failed for', candidateUrls, e);
        // Send 502 Bad Gateway
        const err = `HTTP/1.1 502 Bad Gateway\r\nContent-Length: ${e.message.length}\r\nConnection: close\r\n\r\n${e.message}`;
        const errBytes = new TextEncoder().encode(err);
        this.sendDataChunked(errBytes);
      }
    }

    sendDataChunked(data) {
      const maxSegment = MTU - 40;  // IP header (20) + TCP header (20)
      for (let off = 0; off < data.length; off += maxSegment) {
        const chunk = data.slice(off, off + maxSegment);
        this.sendTcp(PSH | ACK, chunk);
        this.ourSeq += chunk.length;
      }
      // Send FIN after all data
      this.sendTcp(FIN | ACK, null);
      this.ourSeq++;
      this.sentFin = true;
      this.state = 'FIN_WAIT';
    }

    sendTcp(flags, data) {
      const segment = buildTcpPacket(
        this.dstPort, this.srcPort,
        this.ourSeq, this.guestSeq,
        flags, data || new Uint8Array(0),
        this.dstIp, GUEST_IP
      );
      const pkt = buildIpPacket(6, this.dstIp, GUEST_IP, segment);
      enqueueRx(pkt);
    }

    close() {
      connections.delete(this.key);
    }
  }

  // ── Tunnel support (WebSocket for real TCP) ──

  function sendViaTunnel(ipPkt) {
    if (!tunnelWs || tunnelWs.readyState !== WebSocket.OPEN) return false;
    try {
      tunnelWs.send(new Uint8Array([
        ...ipPkt.dstIp,
        (ipPkt.dstPort || 0) >> 8, (ipPkt.dstPort || 0) & 0xff,
        ...ipPkt.payload,
      ]));
      return true;
    } catch (e) {
      return false;
    }
  }

  // ── Packet handling ──

  function enqueueRx(pkt) {
    rxQueue.push(pkt);
    stats.rxPackets++;
    stats.rxBytes += pkt.length;
  }

  function handleTxPacket(buf) {
    stats.txPackets++;
    stats.txBytes += buf.length;

    const ipPkt = parseIpPacket(buf);
    if (!ipPkt) return;

    const proto = ipPkt.protocol;

    // ICMP
    if (proto === 1) {
      handleIcmp(ipPkt);
      return;
    }

    // UDP (DNS)
    if (proto === 17) {
      const udp = parseUdp(ipPkt.payload);
      if (!udp) return;
      if (udp.dstPort === 53) {
        resolveDns(udp.data, udp.srcPort, ipPkt.srcIp);
      }
      return;
    }

    // TCP
    if (proto === 6) {
      const tcp = parseTcp(ipPkt.payload);
      if (!tcp) return;

      const key = `${tcp.srcPort}:${ipPkt.dstIp.join('.')}:${tcp.dstPort}`;

      if (tcp.flags & SYN && !(tcp.flags & ACK)) {
        // New connection
        const conn = new TcpConnection(tcp.srcPort, ipPkt.dstIp, tcp.dstPort);
        connections.set(key, conn);
        conn.handlePacket(tcp, ipPkt);
      } else if (connections.has(key)) {
        connections.get(key).handlePacket(tcp, ipPkt);
      } else {
        // Send RST for unknown connections
        const rst = buildTcpPacket(
          tcp.dstPort, tcp.srcPort,
          tcp.ack, tcp.seq + 1,
          RST | ACK, new Uint8Array(0),
          ipPkt.dstIp, ipPkt.srcIp
        );
        enqueueRx(buildIpPacket(6, ipPkt.dstIp, ipPkt.srcIp, rst));
      }
    }
  }

  // ── Public API ──

  return {
    send(buf) {
      handleTxPacket(buf instanceof Uint8Array ? buf : new Uint8Array(buf));
    },

    recv(maxLen) {
      if (rxQueue.length === 0) return null;
      const pkt = rxQueue.shift();
      if (pkt.length <= maxLen) return pkt;
      return pkt.slice(0, maxLen);
    },

    poll() {
      return rxQueue.length;
    },

    setTunnelURL(url) {
      tunnelUrl = url;
      if (tunnelWs) tunnelWs.close();
      if (url) {
        tunnelWs = new WebSocket(url);
        tunnelWs.binaryType = 'arraybuffer';
        tunnelWs.onmessage = (ev) => {
          const data = new Uint8Array(ev.data);
          enqueueRx(data);
        };
        tunnelWs.onerror = (e) => console.warn('[net-proxy] tunnel error:', e);
      }
    },

    getStats() { return { ...stats, queueLen: rxQueue.length, connections: connections.size }; },
    getConfig() { return { guestIp: GUEST_IP.join('.'), gatewayIp: GATEWAY_IP.join('.'), mtu: MTU }; },
  };
})();
