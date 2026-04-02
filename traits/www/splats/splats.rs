use serde_json::{json, Value};

/// Default .splat file URL — a small bonsai scene (~8.7 MB, 7K splats).
const DEFAULT_SPLAT_URL: &str =
    "https://huggingface.co/datasets/dylanebert/3dgs/resolve/main/bonsai/bonsai-7k-mini.splat";

/// Gallery of example scenes the user can pick from.
const GALLERY: &[(&str, &str)] = &[
    ("bonsai", "https://huggingface.co/datasets/dylanebert/3dgs/resolve/main/bonsai/bonsai-7k-mini.splat"),
    ("train",  "https://huggingface.co/cakewalk/splat-data/resolve/main/train.splat"),
    ("truck",  "https://huggingface.co/cakewalk/splat-data/resolve/main/truck.splat"),
];

/// www.splats — 3D Gaussian splat viewer using WebGPU.
///
/// Returns a self-contained HTML page that fetches and renders real .splat
/// binary files using WebGPU with orbit camera controls and back-to-front
/// depth sorting for proper transparency.
pub fn splats(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("render");
    let url = args.get(1).and_then(|v| v.as_str()).unwrap_or("");

    match action {
        "gallery" => {
            let list: Vec<Value> = GALLERY
                .iter()
                .map(|(name, u)| json!({"name": name, "url": u}))
                .collect();
            json!({"ok": true, "scenes": list})
        }
        _ => {
            let splat_url = if url.is_empty() { DEFAULT_SPLAT_URL } else { url };
            Value::String(build_viewer(splat_url))
        }
    }
}

fn build_viewer(splat_url: &str) -> String {
    let js = VIEWER_JS.replace("__SPLAT_URL__", splat_url);
    let gallery_json = serde_json::to_string(
        &GALLERY.iter().map(|(n, u)| json!({"name": n, "url": u})).collect::<Vec<_>>()
    ).unwrap_or_else(|_| "[]".to_string());
    let js = js.replace("__GALLERY_JSON__", &gallery_json);
    format!(
        r##"<style>
  #splat-root {{ margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; background: #000; }}
  #splat-root canvas {{ width: 100%; height: 100%; display: block; touch-action: none; }}
  #splat-info {{
    position: absolute; top: 10px; left: 10px; color: #fff; font: 12px/1.4 monospace;
    background: rgba(0,0,0,0.6); padding: 6px 10px; border-radius: 4px; pointer-events: none;
    z-index: 10;
  }}
  #splat-error {{
    display: none; position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%);
    color: #ff6b6b; font: 16px system-ui; text-align: center; background: rgba(0,0,0,0.8);
    padding: 24px 32px; border-radius: 8px; z-index: 20;
  }}
  #splat-controls {{
    position: absolute; bottom: 10px; left: 10px; color: #888; font: 11px/1.4 monospace;
    background: rgba(0,0,0,0.5); padding: 4px 8px; border-radius: 4px; pointer-events: none;
  }}
  #splat-gallery {{
    position: absolute; top: 10px; right: 10px; z-index: 10;
  }}
  #splat-gallery select {{
    background: rgba(0,0,0,0.7); color: #fff; border: 1px solid #555;
    padding: 4px 8px; border-radius: 4px; font: 12px monospace; cursor: pointer;
  }}
  #splat-progress {{
    position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%);
    color: #aaa; font: 14px/1.6 monospace; text-align: center; z-index: 15;
  }}
  #splat-progress .bar {{
    width: 200px; height: 4px; background: #333; border-radius: 2px; margin: 8px auto 0;
    overflow: hidden;
  }}
  #splat-progress .bar-fill {{
    height: 100%; background: #4af; width: 0%; transition: width 0.2s;
  }}
</style>
<div id="splat-root">
  <canvas id="splat-canvas"></canvas>
  <div id="splat-info">Loading…</div>
  <div id="splat-error"></div>
  <div id="splat-progress">
    <div>Downloading splats…</div>
    <div class="bar"><div class="bar-fill"></div></div>
    <div class="bytes"></div>
  </div>
  <div id="splat-gallery"><select id="splat-scene-select"></select></div>
  <div id="splat-controls">Drag: orbit · Scroll: zoom · Shift+drag: pan</div>
</div>
<script>
{js}
</script>"##,
        js = js
    )
}

const VIEWER_JS: &str = r##"
(async function() {
  const info = document.getElementById('splat-info');
  const errDiv = document.getElementById('splat-error');
  const canvas = document.getElementById('splat-canvas');
  const progress = document.getElementById('splat-progress');
  const barFill = progress.querySelector('.bar-fill');
  const bytesDiv = progress.querySelector('.bytes');

  function showError(msg) {
    errDiv.textContent = msg;
    errDiv.style.display = 'block';
    info.style.display = 'none';
    progress.style.display = 'none';
  }

  // ── Gallery selector ──
  const gallery = __GALLERY_JSON__;
  const select = document.getElementById('splat-scene-select');
  gallery.forEach((g, i) => {
    const opt = document.createElement('option');
    opt.value = g.url;
    opt.textContent = g.name;
    if (g.url === '__SPLAT_URL__') opt.selected = true;
    select.appendChild(opt);
  });
  // "Custom URL" option
  {
    const opt = document.createElement('option');
    opt.value = '__custom__';
    opt.textContent = '+ custom URL…';
    select.appendChild(opt);
  }

  // ── WebGPU Init ──
  if (!navigator.gpu) { showError('WebGPU not supported in this browser.\nTry Chrome 113+ or Edge 113+.'); return; }
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) { showError('No WebGPU adapter found.'); return; }
  const device = await adapter.requestDevice();
  const ctx = canvas.getContext('webgpu');
  const format = navigator.gpu.getPreferredCanvasFormat();
  ctx.configure({ device, format, alphaMode: 'premultiplied' });

  // ── Resize ──
  function resize() {
    const root = document.getElementById('splat-root');
    const w = root.clientWidth || window.innerWidth;
    const h = root.clientHeight || window.innerHeight;
    canvas.width = w * devicePixelRatio;
    canvas.height = h * devicePixelRatio;
  }
  resize();
  new ResizeObserver(resize).observe(document.getElementById('splat-root'));

  // ── Fetch and parse .splat binary ──
  // Format: 32 bytes per splat
  //   3×f32 position (12B) + 3×f32 scale (12B) + 4×u8 RGBA (4B) + 4×u8 quaternion (4B)
  async function fetchSplat(url) {
    progress.style.display = 'block';
    barFill.style.width = '0%';
    bytesDiv.textContent = '';
    info.textContent = 'Downloading…';

    const resp = await fetch(url, { mode: 'cors', credentials: 'omit' });
    if (!resp.ok) throw new Error('HTTP ' + resp.status + ' fetching ' + url);

    const contentLength = parseInt(resp.headers.get('content-length') || '0');
    const reader = resp.body.getReader();
    const chunks = [];
    let received = 0;

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(value);
      received += value.length;
      if (contentLength > 0) {
        const pct = Math.min(100, (received / contentLength * 100));
        barFill.style.width = pct.toFixed(1) + '%';
      }
      bytesDiv.textContent = (received / 1024 / 1024).toFixed(1) + ' MB';
    }

    progress.style.display = 'none';

    // Concatenate chunks
    const data = new Uint8Array(received);
    let offset = 0;
    for (const c of chunks) { data.set(c, offset); offset += c.length; }
    return data;
  }

  function parseSplat(data) {
    const ROW = 32; // bytes per splat
    const count = Math.floor(data.byteLength / ROW);
    const f32 = new Float32Array(data.buffer, data.byteOffset, count * 8);
    const u8 = data;

    // Each splat → 14 floats: pos(3) scale(3) color(4) quat(4)
    const out = new Float32Array(count * 14);
    for (let i = 0; i < count; i++) {
      // Position (3×f32)
      out[i*14+0] = f32[i*8+0];
      out[i*14+1] = f32[i*8+1];
      out[i*14+2] = f32[i*8+2];
      // Scale (3×f32)
      out[i*14+3] = f32[i*8+3];
      out[i*14+4] = f32[i*8+4];
      out[i*14+5] = f32[i*8+5];
      // Color RGBA (4×u8 → normalized float)
      const cOff = i * ROW + 24;
      out[i*14+6] = u8[cOff]   / 255;
      out[i*14+7] = u8[cOff+1] / 255;
      out[i*14+8] = u8[cOff+2] / 255;
      out[i*14+9] = u8[cOff+3] / 255;
      // Quaternion (4×u8 → float: (v - 128) / 128)
      const qOff = i * ROW + 28;
      out[i*14+10] = (u8[qOff]   - 128) / 128;
      out[i*14+11] = (u8[qOff+1] - 128) / 128;
      out[i*14+12] = (u8[qOff+2] - 128) / 128;
      out[i*14+13] = (u8[qOff+3] - 128) / 128;
    }
    return { data: out, count };
  }

  // ── Camera ──
  let camDist = 5.0, camTheta = 0.5, camPhi = 0.8;
  let panX = 0, panY = 0, panZ = 0;
  let sceneScale = 5.0; // updated after load for zoom limits

  // Compute axis-aligned bounding box of splat positions
  function computeBounds(data, count) {
    let minX = Infinity, minY = Infinity, minZ = Infinity;
    let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;
    for (let i = 0; i < count; i++) {
      const x = data[i*14], y = data[i*14+1], z = data[i*14+2];
      if (x < minX) minX = x; if (x > maxX) maxX = x;
      if (y < minY) minY = y; if (y > maxY) maxY = y;
      if (z < minZ) minZ = z; if (z > maxZ) maxZ = z;
    }
    const diag = Math.hypot(maxX-minX, maxY-minY, maxZ-minZ);
    return {
      center: [(minX+maxX)/2, (minY+maxY)/2, (minZ+maxZ)/2],
      extent: [maxX-minX, maxY-minY, maxZ-minZ],
      diag: diag || 1.0,
    };
  }

  function mat4Perspective(fov, aspect, near, far) {
    const f = 1 / Math.tan(fov / 2);
    const nf = 1 / (near - far);
    return new Float32Array([
      f/aspect, 0, 0, 0,
      0, f, 0, 0,
      0, 0, (far+near)*nf, -1,
      0, 0, 2*far*near*nf, 0
    ]);
  }

  function mat4LookAt(eye, center, up) {
    const zx = eye[0]-center[0], zy = eye[1]-center[1], zz = eye[2]-center[2];
    let len = Math.hypot(zx, zy, zz);
    const z = [zx/len, zy/len, zz/len];
    const xx = up[1]*z[2] - up[2]*z[1], xy = up[2]*z[0] - up[0]*z[2], xz = up[0]*z[1] - up[1]*z[0];
    len = Math.hypot(xx, xy, xz);
    const x = [xx/len, xy/len, xz/len];
    const y = [x[1]*z[2]-x[2]*z[1], x[2]*z[0]-x[0]*z[2], x[0]*z[1]-x[1]*z[0]];
    return new Float32Array([
      x[0], y[0], z[0], 0,
      x[1], y[1], z[1], 0,
      x[2], y[2], z[2], 0,
      -(x[0]*eye[0]+x[1]*eye[1]+x[2]*eye[2]),
      -(y[0]*eye[0]+y[1]*eye[1]+y[2]*eye[2]),
      -(z[0]*eye[0]+z[1]*eye[1]+z[2]*eye[2]),
      1
    ]);
  }

  function mat4Mul(a, b) {
    const o = new Float32Array(16);
    for (let i = 0; i < 4; i++)
      for (let j = 0; j < 4; j++)
        o[j*4+i] = a[i]*b[j*4] + a[4+i]*b[j*4+1] + a[8+i]*b[j*4+2] + a[12+i]*b[j*4+3];
    return o;
  }

  function getViewProj() {
    const eye = [
      panX + camDist * Math.sin(camPhi) * Math.cos(camTheta),
      panY + camDist * Math.cos(camPhi),
      panZ + camDist * Math.sin(camPhi) * Math.sin(camTheta),
    ];
    const center = [panX, panY, panZ];
    const aspect = canvas.width / canvas.height;
    const far = Math.max(200, sceneScale * 10);
    const near = far * 0.00005;
    const fovY = Math.PI / 4;
    const proj = mat4Perspective(fovY, aspect, near, far);
    const view = mat4LookAt(eye, center, [0, 1, 0]);
    const fy = canvas.height / (2 * Math.tan(fovY / 2));
    const fx = fy; // square pixels
    return { vp: mat4Mul(proj, view), view, eye, focal: [fx, fy] };
  }

  // ── Uniform buffer: viewProj(64) + view(64) + focal(8) + viewport(8) = 144B ──
  const uniformBuf = device.createBuffer({ size: 144, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });

  const bindGroupLayout = device.createBindGroupLayout({
    entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: 'uniform' } }],
  });
  const bindGroup = device.createBindGroup({
    layout: bindGroupLayout,
    entries: [{ binding: 0, resource: { buffer: uniformBuf } }],
  });

  // ── Shaders ──
  const shaderCode = `
    struct Uniforms {
      viewProj: mat4x4f,
      view: mat4x4f,
      focal: vec2f,
      viewport: vec2f,
    };
    @group(0) @binding(0) var<uniform> u: Uniforms;

    struct VOut {
      @builtin(position) pos: vec4f,
      @location(0) color: vec4f,
      @location(1) uv: vec2f,
    };

    // Build rotation matrix from quaternion
    fn quatToMat3(q: vec4f) -> mat3x3f {
      let x = q.x; let y = q.y; let z = q.z; let w = q.w;
      let x2 = x+x; let y2 = y+y; let z2 = z+z;
      let xx = x*x2; let xy = x*y2; let xz = x*z2;
      let yy = y*y2; let yz = y*z2; let zz = z*z2;
      let wx = w*x2; let wy = w*y2; let wz = w*z2;
      return mat3x3f(
        vec3f(1.0-yy-zz, xy+wz, xz-wy),
        vec3f(xy-wz, 1.0-xx-zz, yz+wx),
        vec3f(xz+wy, yz-wx, 1.0-xx-yy),
      );
    }

    @vertex fn vs(
      @location(0) quad: vec2f,
      @location(1) splatPos: vec3f,
      @location(2) splatScale: vec3f,
      @location(3) splatColor: vec4f,
      @location(4) splatQuat: vec4f,
    ) -> VOut {
      var out: VOut;

      // Transform splat center to view space
      let cam = u.view * vec4f(splatPos, 1.0);
      let camPos = cam.xyz;

      // Clip behind camera
      if (camPos.z > -0.2) {
        out.pos = vec4f(0.0, 0.0, 2.0, 1.0);
        return out;
      }

      // Project center to clip space
      let pos2d = u.viewProj * vec4f(splatPos, 1.0);
      let clip = 1.2 * pos2d.w;
      if (pos2d.x < -clip || pos2d.x > clip || pos2d.y < -clip || pos2d.y > clip) {
        out.pos = vec4f(0.0, 0.0, 2.0, 1.0);
        return out;
      }

      // Build 3D covariance: Σ = R * S * Sᵀ * Rᵀ = (R*S)(R*S)ᵀ
      let R = quatToMat3(splatQuat);
      let M = mat3x3f(
        R[0] * splatScale.x,
        R[1] * splatScale.y,
        R[2] * splatScale.z,
      );
      let Sigma = M * transpose(M);

      // Jacobian of projection: J maps 3D view-space offsets to 2D screen
      let tz = camPos.z;
      let tz2 = tz * tz;
      let J = mat3x3f(
        vec3f(u.focal.x / tz, 0.0, 0.0),
        vec3f(0.0, u.focal.y / tz, 0.0),
        vec3f(-u.focal.x * camPos.x / tz2, -u.focal.y * camPos.y / tz2, 0.0),
      );

      // View rotation (upper-left 3×3 of view matrix)
      let W = mat3x3f(
        u.view[0].xyz,
        u.view[1].xyz,
        u.view[2].xyz,
      );

      // 2D covariance: T = J * W, cov2d = T * Σ * Tᵀ
      let T = J * W;
      let cov2d = T * Sigma * transpose(T);

      // Extract 2×2 from upper-left of cov2d + small regularization
      let a = cov2d[0][0] + 0.3;
      let b = cov2d[0][1];
      let d = cov2d[1][1] + 0.3;

      // Eigendecomposition of 2×2 symmetric matrix
      let mid = 0.5 * (a + d);
      let radius = length(vec2f(0.5 * (a - d), b));
      let lambda1 = mid + radius;
      let lambda2 = max(mid - radius, 0.1);

      // Eigenvectors → major/minor axes in pixels
      let diagVec = normalize(vec2f(b, lambda1 - a));
      let majorAxis = sqrt(lambda1) * diagVec;
      let minorAxis = sqrt(lambda2) * vec2f(diagVec.y, -diagVec.x);

      // Scale quad corners by the 2D axes, then to NDC
      let offset = (quad.x * majorAxis + quad.y * minorAxis) * 2.0 / u.viewport;
      let center = pos2d.xy / pos2d.w;

      out.pos = vec4f(center + offset, pos2d.z / pos2d.w, 1.0);
      out.color = splatColor;
      out.uv = quad;
      return out;
    }

    @fragment fn fs(inp: VOut) -> @location(0) vec4f {
      // Gaussian falloff: exp(-0.5 * r^2)
      let r2 = dot(inp.uv, inp.uv);
      if (r2 > 4.0) { discard; }
      let alpha = inp.color.a * exp(-0.5 * r2);
      return vec4f(inp.color.rgb * alpha, alpha);
    }
  `;

  const shaderModule = device.createShaderModule({ code: shaderCode });

  // ── Pipeline ──
  const pipeline = device.createRenderPipeline({
    layout: device.createPipelineLayout({ bindGroupLayouts: [bindGroupLayout] }),
    vertex: {
      module: shaderModule,
      entryPoint: 'vs',
      buffers: [
        { // Quad vertices (per-vertex)
          arrayStride: 8,
          stepMode: 'vertex',
          attributes: [{ shaderLocation: 0, offset: 0, format: 'float32x2' }],
        },
        { // Instance data (per-instance)
          arrayStride: 56, // 14 floats × 4 bytes
          stepMode: 'instance',
          attributes: [
            { shaderLocation: 1, offset: 0,  format: 'float32x3' },  // pos
            { shaderLocation: 2, offset: 12, format: 'float32x3' },  // scale
            { shaderLocation: 3, offset: 24, format: 'float32x4' },  // color
            { shaderLocation: 4, offset: 40, format: 'float32x4' },  // quat
          ],
        },
      ],
    },
    fragment: {
      module: shaderModule,
      entryPoint: 'fs',
      targets: [{
        format,
        blend: {
          color: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
          alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
        },
      }],
    },
    primitive: { topology: 'triangle-list' },
    depthStencil: undefined,
  });

  // ── Quad buffer ──
  const quadVerts = new Float32Array([
    -1,-1,  1,-1,  1, 1,
    -1,-1,  1, 1, -1, 1,
  ]);
  const quadBuf = device.createBuffer({ size: quadVerts.byteLength, usage: GPUBufferUsage.VERTEX, mappedAtCreation: true });
  new Float32Array(quadBuf.getMappedRange()).set(quadVerts);
  quadBuf.unmap();

  // ── State ──
  let splatBuf = null;
  let numSplats = 0;
  let sortedIndices = null;
  let sortedSplatBuf = null;
  let needsSort = true;

  // ── Depth-sort splats back-to-front ──
  function sortSplats(splatData, count, viewProj) {
    const depths = new Float32Array(count);
    for (let i = 0; i < count; i++) {
      const x = splatData[i*14], y = splatData[i*14+1], z = splatData[i*14+2];
      // Depth = dot(viewProj row3, [x,y,z,1])
      depths[i] = viewProj[2]*x + viewProj[6]*y + viewProj[10]*z + viewProj[14];
    }
    // Create index array and sort back-to-front (most negative depth first)
    const indices = new Uint32Array(count);
    for (let i = 0; i < count; i++) indices[i] = i;
    indices.sort((a, b) => depths[a] - depths[b]);
    return indices;
  }

  // ── Load .splat data into GPU ──
  let currentSplatData = null;
  async function loadScene(url) {
    // Reset state
    numSplats = 0;
    if (splatBuf) splatBuf.destroy();
    if (sortedSplatBuf) sortedSplatBuf.destroy();
    splatBuf = null;
    sortedSplatBuf = null;

    const raw = await fetchSplat(url);
    const { data, count } = parseSplat(raw);
    currentSplatData = data;
    numSplats = count;
    needsSort = true;

    // Auto-fit camera to scene bounding box
    if (count > 0) {
      const bounds = computeBounds(data, count);
      panX = bounds.center[0];
      panY = bounds.center[1];
      panZ = bounds.center[2];
      camDist = bounds.diag * 0.7;
      sceneScale = bounds.diag;
      camTheta = 0.5;
      camPhi = 0.8;
    }

    info.textContent = count.toLocaleString() + ' splats · WebGPU';
  }

  // Upload sorted data to GPU
  function uploadSorted(sorted, data, count) {
    const buf = new Float32Array(count * 14);
    for (let i = 0; i < count; i++) {
      const src = sorted[i] * 14;
      const dst = i * 14;
      for (let j = 0; j < 14; j++) buf[dst+j] = data[src+j];
    }
    if (sortedSplatBuf) sortedSplatBuf.destroy();
    sortedSplatBuf = device.createBuffer({ size: buf.byteLength, usage: GPUBufferUsage.VERTEX, mappedAtCreation: true });
    new Float32Array(sortedSplatBuf.getMappedRange()).set(buf);
    sortedSplatBuf.unmap();
    return sortedSplatBuf;
  }

  // ── Input handling ──
  let dragging = false, dragBtn = 0, lastX = 0, lastY = 0;
  canvas.addEventListener('pointerdown', (e) => {
    dragging = true; dragBtn = e.button; lastX = e.clientX; lastY = e.clientY;
    canvas.setPointerCapture(e.pointerId);
  });
  canvas.addEventListener('pointermove', (e) => {
    if (!dragging) return;
    const dx = e.clientX - lastX, dy = e.clientY - lastY;
    lastX = e.clientX; lastY = e.clientY;
    if (e.shiftKey || dragBtn === 1) {
      panX -= dx * 0.005 * camDist;
      panY += dy * 0.005 * camDist;
    } else {
      camTheta -= dx * 0.005;
      camPhi = Math.max(0.1, Math.min(Math.PI - 0.1, camPhi - dy * 0.005));
    }
    needsSort = true;
  });
  canvas.addEventListener('pointerup', () => { dragging = false; });
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    camDist *= 1 + e.deltaY * 0.001;
    camDist = Math.max(0.01 * sceneScale, Math.min(10 * sceneScale, camDist));
    needsSort = true;
  }, { passive: false });

  // ── Scene selector ──
  select.addEventListener('change', async () => {
    if (select.value === '__custom__') {
      const url = prompt('Enter .splat file URL:');
      if (url) {
        try { await loadScene(url); }
        catch(e) { showError('Failed to load: ' + e.message); }
      }
    } else {
      try { await loadScene(select.value); }
      catch(e) { showError('Failed to load: ' + e.message); }
    }
  });

  // ── Sort throttle ──
  let lastSortTime = 0;
  const SORT_INTERVAL = 200; // ms between sorts

  // ── Render loop ──
  function frame() {
    if (numSplats === 0) {
      requestAnimationFrame(frame);
      return;
    }
    const { vp, view, eye, focal } = getViewProj();
    // Layout: viewProj(16f) + view(16f) + focal(2f) + viewport(2f) = 36 floats = 144B
    const uniformData = new Float32Array(36);
    uniformData.set(vp, 0);
    uniformData.set(view, 16);
    uniformData[32] = focal[0];
    uniformData[33] = focal[1];
    uniformData[34] = canvas.width;
    uniformData[35] = canvas.height;
    device.queue.writeBuffer(uniformBuf, 0, uniformData);

    // Re-sort periodically when camera moves
    const now = performance.now();
    if (needsSort && now - lastSortTime > SORT_INTERVAL && currentSplatData) {
      sortedIndices = sortSplats(currentSplatData, numSplats, vp);
      uploadSorted(sortedIndices, currentSplatData, numSplats);
      lastSortTime = now;
      needsSort = false;
    }

    if (!sortedSplatBuf) {
      requestAnimationFrame(frame);
      return;
    }

    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [{
        view: ctx.getCurrentTexture().createView(),
        loadOp: 'clear',
        storeOp: 'store',
        clearValue: { r: 0.02, g: 0.02, b: 0.04, a: 1 },
      }],
    });
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bindGroup);
    pass.setVertexBuffer(0, quadBuf);
    pass.setVertexBuffer(1, sortedSplatBuf);
    pass.draw(6, numSplats);
    pass.end();
    device.queue.submit([encoder.finish()]);
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);

  // ── Initial load ──
  try {
    await loadScene('__SPLAT_URL__');
  } catch(e) {
    showError('Failed to load splat file:\n' + e.message);
  }
})();
"##;
