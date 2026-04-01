use serde_json::{json, Value};

/// www.splats — 3D Gaussian splat viewer using WebGPU.
///
/// Returns a self-contained HTML page that renders Gaussian splats
/// using WebGPU compute + render pipelines with orbit camera controls.
pub fn splats(args: &[Value]) -> Value {
    let action = args.first().and_then(|v| v.as_str()).unwrap_or("render");

    match action {
        "scene" => {
            // Return the example scene as JSON array-of-arrays
            let arr: Vec<serde_json::Value> = SPLAT_DATA
                .iter()
                .map(|row| {
                    json!({
                        "pos": [row[0], row[1], row[2]],
                        "scale": [row[3], row[4], row[5]],
                        "color": [row[6], row[7], row[8], row[9]],
                        "quat": [row[10], row[11], row[12], row[13]],
                    })
                })
                .collect();
            json!({"ok": true, "splats": arr, "count": arr.len()})
        }
        _ => Value::String(build_viewer()),
    }
}

fn build_viewer() -> String {
    // Embed the scene data inline—replace the SCENE_DATA placeholder in the JS
    let js = VIEWER_JS.replace("SCENE_DATA_PLACEHOLDER", &build_scene_array());
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
</style>
<div id="splat-root">
  <canvas id="splat-canvas"></canvas>
  <div id="splat-info">Loading WebGPU…</div>
  <div id="splat-error"></div>
  <div id="splat-controls">Drag: orbit · Scroll: zoom · Shift+drag: pan</div>
</div>
<script>
{js}
</script>"##,
        js = js
    )
}

/// Build the JS array literal from SPLAT_DATA (each row: 14 floats).
fn build_scene_array() -> String {
    let mut out = String::from("[\n");
    for (i, row) in SPLAT_DATA.iter().enumerate() {
        out.push_str("  [");
        for (j, v) in row.iter().enumerate() {
            if j > 0 { out.push(','); }
            out.push_str(&format!("{}", v));
        }
        out.push(']');
        if i + 1 < SPLAT_DATA.len() { out.push(','); }
        out.push('\n');
    }
    out.push(']');
    out
}

const VIEWER_JS: &str = r##"
(async function() {
  const info = document.getElementById('splat-info');
  const errDiv = document.getElementById('splat-error');
  const canvas = document.getElementById('splat-canvas');

  function showError(msg) {
    errDiv.textContent = msg;
    errDiv.style.display = 'block';
    info.style.display = 'none';
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

  // ── Example splat scene ──
  // Each splat: [x, y, z, sx, sy, sz, r, g, b, a, q0, q1, q2, q3]
  // Position (xyz), scale (sxyz), color (rgba), quaternion rotation (q0123)
  const SPLATS = SCENE_DATA_PLACEHOLDER;

  const numSplats = SPLATS.length;
  info.textContent = numSplats + ' splats · WebGPU';

  // ── Pack splat data into GPU buffers ──
  // Vertex data: billboard quad
  const quadVerts = new Float32Array([
    -1,-1,  1,-1,  1, 1,
    -1,-1,  1, 1, -1, 1,
  ]);
  const quadBuf = device.createBuffer({ size: quadVerts.byteLength, usage: GPUBufferUsage.VERTEX, mappedAtCreation: true });
  new Float32Array(quadBuf.getMappedRange()).set(quadVerts);
  quadBuf.unmap();

  // Splat instance buffer: pos(3) + scale(3) + color(4) + quat(4) = 14 floats
  const splatDataFlat = new Float32Array(numSplats * 14);
  for (let i = 0; i < numSplats; i++) {
    const s = SPLATS[i];
    for (let j = 0; j < 14; j++) splatDataFlat[i * 14 + j] = s[j];
  }
  const splatBuf = device.createBuffer({ size: splatDataFlat.byteLength, usage: GPUBufferUsage.VERTEX, mappedAtCreation: true });
  new Float32Array(splatBuf.getMappedRange()).set(splatDataFlat);
  splatBuf.unmap();

  // ── Camera ──
  let camDist = 5.0, camTheta = 0.5, camPhi = 0.8;
  let panX = 0, panY = 0;

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
      camDist * Math.sin(camPhi) * Math.sin(camTheta),
    ];
    const center = [panX, panY, 0];
    const aspect = canvas.width / canvas.height;
    const proj = mat4Perspective(Math.PI / 4, aspect, 0.01, 100);
    const view = mat4LookAt(eye, center, [0, 1, 0]);
    return { vp: mat4Mul(proj, view), eye };
  }

  // ── Uniform buffer (viewproj 64B + eye 16B + viewport 8B + pad 8B = 96B) ──
  const uniformBuf = device.createBuffer({ size: 96, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });

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
      eye: vec3f,
      _pad0: f32,
      viewport: vec2f,
      _pad1: vec2f,
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

      let rot = quatToMat3(splatQuat);
      let worldPos = splatPos + rot * (vec3f(quad, 0.0) * splatScale);

      out.pos = u.viewProj * vec4f(worldPos, 1.0);
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
      // Pan
      panX -= dx * 0.005 * camDist;
      panY += dy * 0.005 * camDist;
    } else {
      // Orbit
      camTheta -= dx * 0.005;
      camPhi = Math.max(0.1, Math.min(Math.PI - 0.1, camPhi - dy * 0.005));
    }
  });
  canvas.addEventListener('pointerup', () => { dragging = false; });
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    camDist *= 1 + e.deltaY * 0.001;
    camDist = Math.max(0.5, Math.min(50, camDist));
  }, { passive: false });

  // ── Render loop ──
  function frame() {
    const { vp, eye } = getViewProj();
    const uniformData = new Float32Array(24); // 96 bytes = 24 floats
    uniformData.set(vp, 0);              // viewProj at offset 0
    uniformData.set(eye, 16);            // eye at offset 64
    uniformData[20] = canvas.width;      // viewport.x at offset 80
    uniformData[21] = canvas.height;     // viewport.y at offset 84
    device.queue.writeBuffer(uniformBuf, 0, uniformData);

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
    pass.setVertexBuffer(1, splatBuf);
    pass.draw(6, numSplats);
    pass.end();
    device.queue.submit([encoder.finish()]);
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
})();
"##;

// Example scene data: each row is [x,y,z, sx,sy,sz, r,g,b,a, q0,q1,q2,q3]
// A colorful molecular/nebula-like arrangement demonstrating Gaussian splatting.
#[rustfmt::skip]
const SPLAT_DATA: &[[f32; 14]] = &[
    // Central bright core
    [ 0.0,  0.0,  0.0,   0.35, 0.35, 0.35,  0.15, 0.45, 1.0, 0.95,  0.0, 0.0, 0.0, 1.0],
    [ 0.05, 0.05,-0.05,  0.25, 0.25, 0.25,  0.3,  0.6,  1.0, 0.6,   0.0, 0.0, 0.0, 1.0],
    // Warm cluster (right)
    [ 0.8,  0.2,  0.3,   0.25, 0.15, 0.2,   1.0, 0.35, 0.15, 0.85,  0.0, 0.0, 0.38, 0.92],
    [ 1.0,  0.05, 0.15,  0.18, 0.18, 0.12,  1.0, 0.55, 0.2,  0.7,   0.0, 0.0, 0.17, 0.98],
    [ 0.65, 0.35, 0.5,   0.12, 0.2,  0.12,  0.95,0.25, 0.1,  0.75,  0.1, 0.0, 0.2, 0.97],
    // Green cluster (upper left)
    [-0.6,  0.5, -0.2,   0.2,  0.35, 0.15,  0.2,  0.9,  0.4, 0.8,   0.1, 0.0, 0.0, 0.99],
    [-0.8,  0.7,  0.1,   0.15, 0.2,  0.15,  0.15, 0.75, 0.35,0.65,  0.0, 0.05,0.0, 1.0],
    [-0.45, 0.65,-0.4,   0.1,  0.15, 0.1,   0.3,  1.0,  0.5, 0.7,   0.0, 0.0, 0.1, 0.99],
    // Gold accent (lower)
    [ 0.3, -0.7,  0.5,   0.15, 0.15, 0.4,   1.0, 0.8,  0.1, 0.9,   0.0, 0.2, 0.0, 0.98],
    [ 0.15,-0.85, 0.35,  0.12, 0.1,  0.25,  0.9, 0.7,  0.05,0.7,   0.0, 0.15,0.0, 0.99],
    // Purple nebula (lower-left)
    [-0.4, -0.3,  0.8,   0.35, 0.2,  0.2,   0.8,  0.2,  0.9, 0.75,  0.0, 0.0, 0.17, 0.98],
    [-0.55,-0.15, 1.0,   0.2,  0.15, 0.15,  0.65, 0.15, 0.85,0.6,   0.05,0.0, 0.1, 0.99],
    [-0.25,-0.5,  0.65,  0.15, 0.25, 0.15,  0.9,  0.3,  1.0, 0.65,  0.0, 0.1, 0.0, 0.99],
    // Cyan accent (upper-right)
    [ 1.2,  0.8, -0.4,   0.18, 0.18, 0.18,  0.0,  0.8,  0.9, 0.85,  0.0, 0.0, 0.0, 1.0],
    [ 1.0,  0.95,-0.25,  0.12, 0.12, 0.12,  0.1,  0.7,  0.85,0.6,   0.0, 0.0, 0.0, 1.0],
    // Orange wisps (far left)
    [-1.0, -0.5,  0.2,   0.22, 0.3,  0.12,  1.0, 0.5,  0.3, 0.8,   0.15,0.1, 0.0, 0.98],
    [-1.2, -0.3,  0.0,   0.15, 0.2,  0.1,   0.9, 0.4,  0.2, 0.6,   0.1, 0.0, 0.05,0.99],
    // Deep blue filament (upper)
    [ 0.5,  1.0,  0.6,   0.12, 0.12, 0.5,   0.4,  0.2,  1.0, 0.9,   0.3, 0.0, 0.0, 0.95],
    [ 0.35, 1.2,  0.45,  0.1,  0.1,  0.35,  0.3,  0.15, 0.85,0.7,   0.25,0.0, 0.0, 0.97],
    // Yellow-green (mid-depth)
    [-0.2,  0.3,  1.2,   0.4,  0.15, 0.15,  0.9,  0.9,  0.2, 0.7,   0.0, 0.25,0.0, 0.97],
    [ 0.0,  0.15, 1.4,   0.2,  0.1,  0.1,   0.8,  0.85, 0.15,0.55,  0.0, 0.2, 0.0, 0.98],
    // Teal cluster (lower-right)
    [ 0.9, -0.4, -0.6,   0.2,  0.25, 0.2,   0.3,  0.7,  0.3, 0.85,  0.0, 0.0, 0.1, 0.99],
    [ 0.75,-0.55,-0.8,   0.15, 0.15, 0.15,  0.25, 0.6,  0.25,0.65,  0.0, 0.0, 0.0, 1.0],
    // Scattered distant splats (depth)
    [ 1.5,  0.0,  1.0,   0.1,  0.1,  0.1,   0.5,  0.5,  1.0, 0.5,   0.0, 0.0, 0.0, 1.0],
    [-1.3,  0.8, -0.8,   0.12, 0.12, 0.12,  1.0, 0.7,  0.5, 0.45,  0.0, 0.0, 0.0, 1.0],
    [ 0.0, -1.2,  0.0,   0.2,  0.1,  0.2,   0.6,  0.3,  0.8, 0.55,  0.0, 0.0, 0.0, 1.0],
    [-0.3,  0.0, -1.0,   0.15, 0.15, 0.15,  0.2,  0.8,  0.6, 0.5,   0.0, 0.0, 0.0, 1.0],
    [ 1.1, -0.9,  0.4,   0.1,  0.25, 0.1,   0.9,  0.5,  0.1, 0.6,   0.1, 0.0, 0.0, 0.99],
    // Reddish-pink haze (back)
    [-0.7,  0.0, -0.7,   0.3,  0.2,  0.2,   0.85, 0.2,  0.4, 0.5,   0.0, 0.1, 0.0, 0.99],
    [ 0.4, -0.2, -1.1,   0.2,  0.15, 0.2,   0.7,  0.15, 0.35,0.45,  0.0, 0.0, 0.15,0.99],
    // Large soft background glow
    [ 0.0,  0.0,  0.0,   0.8,  0.8,  0.8,   0.05, 0.08, 0.2, 0.2,   0.0, 0.0, 0.0, 1.0],
];
