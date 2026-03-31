use serde_json::Value;

pub fn homepage(_args: &[Value]) -> Value {
    Value::String(PAGE_HTML.to_string())
}

const PAGE_HTML: &str = r####"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>traits.build</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
html,body{width:100%;height:100%;overflow:hidden;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif}
body{background:#000;color:#fff}

/* ── Loading overlay ── */
#hp-loading{position:fixed;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;z-index:10;background:#000;transition:opacity .8s}
#hp-loading.done{opacity:0;pointer-events:none}
.loader-ring{width:44px;height:44px;border:3px solid rgba(255,255,255,.08);border-top-color:#7c5cfc;border-radius:50%;animation:spin .9s linear infinite}
@keyframes spin{to{transform:rotate(360deg)}}
#hp-load-text{margin-top:18px;color:rgba(255,255,255,.4);font-size:.85rem;text-align:center;max-width:300px}
#hp-progress{width:220px;height:3px;background:rgba(255,255,255,.08);border-radius:2px;overflow:hidden;margin-top:14px}
#hp-progress-bar{height:100%;width:0;background:linear-gradient(90deg,#7c5cfc,#00d4ff);transition:width .3s}

/* ── Experience container ── */
#hp-exp{position:fixed;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:20px;opacity:0;transition:opacity .8s}
#hp-exp.on{opacity:1}

/* ── Logo ── */
#hp-logo{font-size:clamp(2.8rem,8vw,5rem);font-weight:800;letter-spacing:-.03em;opacity:0;transform:scale(.85);transition:all 1.2s cubic-bezier(.17,.67,.35,1.15)}
#hp-logo.on{opacity:1;transform:scale(1)}
#hp-logo .t{background:linear-gradient(135deg,#7c5cfc,#00d4ff);-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text}
#hp-logo .d{color:rgba(255,255,255,.35)}
#hp-logo.sparkle{filter:drop-shadow(0 0 30px rgba(124,92,252,.5)) drop-shadow(0 0 60px rgba(0,212,255,.25))}

/* ── Subtitle ── */
#hp-sub{font-size:clamp(.95rem,2.8vw,1.3rem);color:rgba(255,255,255,.55);opacity:0;transform:translateY(8px);transition:all .5s;text-align:center;max-width:560px;padding:0 20px;min-height:1.6em;line-height:1.5}
#hp-sub.on{opacity:1;transform:translateY(0)}

/* ── Buttons ── */
#hp-btns{display:flex;flex-direction:column;gap:14px;align-items:center;margin-top:12px}
.hero-btn{display:block;padding:16px 32px;font-size:clamp(.9rem,2.4vw,1.15rem);font-weight:600;color:#fff;background:rgba(255,255,255,.05);border:1px solid rgba(255,255,255,.1);border-radius:14px;cursor:pointer;opacity:0;transform:translateY(28px);transition:all .6s cubic-bezier(.17,.67,.35,1.15);min-width:300px;text-align:center;-webkit-backdrop-filter:blur(10px);backdrop-filter:blur(10px);font-family:inherit}
.hero-btn.on{opacity:1;transform:translateY(0)}
.hero-btn:hover{background:rgba(124,92,252,.15);border-color:rgba(124,92,252,.4);transform:translateY(-2px)}
.hero-btn.selected{background:rgba(124,92,252,.25);border-color:#7c5cfc;box-shadow:0 0 30px rgba(124,92,252,.3);transform:translateY(-2px)}
.hero-btn.dim{opacity:.25;pointer-events:none}

/* ── Mic indicator ── */
#hp-mic{display:flex;flex-direction:column;align-items:center;gap:10px;margin-top:16px;opacity:0;transition:opacity .5s}
#hp-mic.on{opacity:1}
.mic-ring{width:56px;height:56px;border-radius:50%;background:rgba(124,92,252,.12);border:2px solid rgba(124,92,252,.45);display:flex;align-items:center;justify-content:center;animation:mp 2s ease-in-out infinite}
.mic-ring svg{width:22px;height:22px;fill:#7c5cfc}
@keyframes mp{0%,100%{box-shadow:0 0 0 0 rgba(124,92,252,.25)}50%{box-shadow:0 0 0 18px rgba(124,92,252,0)}}
#hp-mic p{color:rgba(255,255,255,.4);font-size:.82rem}

/* ── Transcript ── */
#hp-ts{font-size:.95rem;color:rgba(255,255,255,.6);font-style:italic;min-height:1.4em;opacity:0;transition:opacity .4s;text-align:center;padding:0 20px}
#hp-ts.on{opacity:1}

/* ── Skip link ── */
#hp-skip{position:fixed;bottom:20px;right:24px;color:rgba(255,255,255,.2);font-size:.78rem;text-decoration:none;z-index:20;transition:color .2s}
#hp-skip:hover{color:rgba(255,255,255,.45)}

/* ── Particles ── */
.spark{position:fixed;width:4px;height:4px;border-radius:50%;pointer-events:none;animation:fup 2.5s ease-out forwards}
@keyframes fup{0%{opacity:1;transform:translateY(0) scale(1)}100%{opacity:0;transform:translateY(-180px) scale(0)}}

/* ── Responsive ── */
@media(max-width:600px){.hero-btn{min-width:unset;width:88vw;padding:14px 18px}#hp-btns{gap:10px}}
</style>
</head>
<body>

<div id="hp-loading">
  <div class="loader-ring"></div>
  <p id="hp-load-text">Initializing voice…</p>
  <div id="hp-progress"><div id="hp-progress-bar"></div></div>
</div>

<div id="hp-exp">
  <div id="hp-logo"><span class="t">traits</span><span class="d">.</span><span class="t">build</span></div>
  <p id="hp-sub"></p>
  <div id="hp-btns">
    <button class="hero-btn" data-topic="1">AI-first developer experience</button>
    <button class="hero-btn" data-topic="2">Running on your mobile device</button>
    <button class="hero-btn" data-topic="3">Fully private &amp; open source</button>
  </div>
  <div id="hp-mic">
    <div class="mic-ring"><svg viewBox="0 0 24 24"><path d="M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3zm-1-9c0-.55.45-1 1-1s1 .45 1 1v6c0 .55-.45 1-1 1s-1-.45-1-1V5zm6 6c0 2.76-2.24 5-5 5s-5-2.24-5-5H5c0 3.53 2.61 6.43 6 6.92V21h2v-3.08c3.39-.49 6-3.39 6-6.92h-2z"/></svg></div>
    <p id="hp-mic-label">Listening…</p>
  </div>
  <p id="hp-ts"></p>
</div>

<a id="hp-skip" href="#">Skip intro</a>

<script>
(async function(){
  /* ── helpers ── */
  let dead = false, curAudio = null, micStr = null, aCtx = null;
  const $ = id => document.getElementById(id);
  const wait = ms => new Promise(r => { if(!dead) setTimeout(r, ms); });

  /* ── cleanup on SPA navigation ── */
  window._pageCleanup = () => {
    dead = true;
    if(curAudio){ try{curAudio.pause();}catch(_){} curAudio=null; }
    if(micStr){ micStr.getTracks().forEach(t=>t.stop()); micStr=null; }
    if(aCtx){ try{aCtx.close();}catch(_){} aCtx=null; }
  };

  /* ── progress ── */
  function prog(pct, txt){
    if(txt) $('hp-load-text').textContent = txt;
    $('hp-progress-bar').style.width = pct+'%';
  }

  /* ── skip handler ── */
  function showAll(){
    dead = true;
    if(curAudio){ try{curAudio.pause();}catch(_){} }
    $('hp-loading').classList.add('done');
    $('hp-exp').classList.add('on');
    $('hp-logo').classList.add('on','sparkle');
    $('hp-sub').classList.add('on');
    $('hp-sub').textContent = 'Composable function kernel — CLI, REST, MCP, WASM';
    document.querySelectorAll('.hero-btn').forEach(b=>b.classList.add('on'));
    $('hp-skip').style.display='none';
    linkBtns();
  }
  $('hp-skip').onclick = e => { e.preventDefault(); showAll(); };

  /* ── button click navigation ── */
  function linkBtns(){
    document.querySelectorAll('.hero-btn').forEach(btn=>{
      btn.onclick = () => {
        const t = btn.dataset.topic;
        if(t==='1') location.hash='/docs';
        else if(t==='2') location.hash='/terminal';
        else location.hash='/playground';
      };
    });
  }

  /* ── TTS speak ── */
  let tts = null;
  async function say(text){
    if(dead || !tts) return;
    try{
      const a = await tts.generate(text, {voice:'af_heart'});
      if(dead) return;
      const blob = await a.toBlob();
      const url = URL.createObjectURL(blob);
      curAudio = new Audio(url);
      await new Promise(r=>{
        curAudio.onended = ()=>{ URL.revokeObjectURL(url); curAudio=null; r(); };
        curAudio.onerror = ()=>{ URL.revokeObjectURL(url); curAudio=null; r(); };
        curAudio.play().catch(r);
      });
    }catch(e){ console.warn('[HP] TTS:', e); }
  }

  /* ── particles ── */
  function sparkle(){
    const el = $('hp-logo'), rc = el.getBoundingClientRect();
    for(let i=0;i<14;i++){
      const p = document.createElement('div');
      p.className='spark';
      p.style.left = (rc.left+Math.random()*rc.width)+'px';
      p.style.top  = (rc.top+Math.random()*rc.height)+'px';
      p.style.background = Math.random()>.5?'#7c5cfc':'#00d4ff';
      p.style.animationDelay = (Math.random()*.6)+'s';
      document.body.appendChild(p);
      setTimeout(()=>p.remove(),3200);
    }
  }

  /* ── audio helpers ── */
  function resample(data,from){
    if(from===16000) return data;
    const r=from/16000, n=Math.round(data.length/r), o=new Float32Array(n);
    for(let i=0;i<n;i++){const s=i*r,lo=Math.floor(s),hi=Math.min(lo+1,data.length-1);o[i]=data[lo]*(1-(s-lo))+data[hi]*(s-lo);}
    return o;
  }
  function merge(arrs){
    const n=arrs.reduce((s,a)=>s+a.length,0), o=new Float32Array(n);
    let off=0; for(const a of arrs){o.set(a,off);off+=a.length;} return o;
  }

  /* ── listen for one utterance ── */
  let sttFn = null;
  async function hear(){
    if(dead || !sttFn) return '';
    try{
      micStr = await navigator.mediaDevices.getUserMedia({
        audio:{echoCancellation:true,noiseSuppression:true,autoGainControl:true}
      });
    }catch(e){ console.warn('[HP] Mic:', e); return ''; }
    if(dead){ micStr.getTracks().forEach(t=>t.stop()); return ''; }

    aCtx = new AudioContext();
    const sr = aCtx.sampleRate;
    const src = aCtx.createMediaStreamSource(micStr);
    const proc = aCtx.createScriptProcessor(4096,1,1);
    src.connect(proc); proc.connect(aCtx.destination);

    return new Promise(res=>{
      let buf=[], ss=0, sil=0, done=false;
      const TH=0.015, SIL=1200, MIN=500;

      proc.onaudioprocess = async e => {
        if(done||dead) return;
        const inp = e.inputBuffer.getChannelData(0);
        const rms = Math.sqrt(inp.reduce((s,v)=>s+v*v,0)/inp.length);

        if(rms>TH){ if(!ss) ss=Date.now(); sil=0; buf.push(new Float32Array(inp)); }
        else if(ss){
          buf.push(new Float32Array(inp));
          if(!sil) sil=Date.now();
          else if(Date.now()-sil>SIL && Date.now()-ss>MIN){
            done=true;
            proc.disconnect(); src.disconnect();
            micStr.getTracks().forEach(t=>t.stop()); micStr=null;
            aCtx.close().catch(()=>{}); aCtx=null;
            const pcm=merge(buf);
            if(pcm.length<8000){ res(''); return; }
            try{ const r=await sttFn(resample(pcm,sr),{language:'en'}); res((r.text||'').trim()); }
            catch(err){ console.warn('[HP] STT:', err); res(''); }
          }
        }
      };

      /* timeout 15s */
      setTimeout(()=>{
        if(!done){ done=true;
          try{proc.disconnect();src.disconnect();}catch(_){}
          if(micStr){micStr.getTracks().forEach(t=>t.stop());micStr=null;}
          if(aCtx){aCtx.close().catch(()=>{});aCtx=null;}
          res('');
        }
      },15000);
    });
  }

  /* ── match topics ── */
  function match(text){
    if(!text) return 0;
    const t=text.toLowerCase();
    if(/\b(ai|developer|experience|code|coding|develop|first)\b/.test(t)) return 1;
    if(/\b(mobile|phone|device|run|running|tablet|android|ios|iphone)\b/.test(t)) return 2;
    if(/\b(private|open.?source|local|secure|privacy|extensible|free)\b/.test(t)) return 3;
    return 0;
  }

  /* ════════════════════════════════════════
     MAIN FLOW
     ════════════════════════════════════════ */
  try{
    /* 1 ── Load TTS ── */
    prog(10,'Loading voice synthesis…');
    const kokoro = await import('https://cdn.jsdelivr.net/npm/kokoro-js@1.2.1');
    if(dead) return;

    prog(25,'Loading TTS model (~92 MB first visit)…');
    tts = await kokoro.KokoroTTS.from_pretrained('onnx-community/Kokoro-82M-v1.0-ONNX',{
      dtype:'q8', device:'wasm',
      progress_callback: p => {
        if(p.progress!=null) prog(25+p.progress*.4, 'TTS: '+(p.status||'')+' '+Math.round(p.progress)+'%');
      }
    });
    if(dead) return;

    /* 2 ── Start STT load in background ── */
    prog(70,'Preparing speech recognition…');
    const sttP = (async()=>{
      const {pipeline} = await import('https://cdn.jsdelivr.net/npm/@huggingface/transformers');
      if(dead) return null;
      return pipeline('automatic-speech-recognition','onnx-community/whisper-base',{
        device:'wasm',
        dtype:{encoder_model:'fp32',decoder_model_merged:'q4'},
        progress_callback: p => {
          if(p.progress!=null) prog(70+p.progress*.25, 'STT: '+(p.status||'')+' '+Math.round(p.progress)+'%');
        }
      });
    })();

    prog(100,'Ready'); await wait(350);

    /* 3 ── Transition to experience ── */
    $('hp-loading').classList.add('done');
    $('hp-exp').classList.add('on');
    await wait(500);
    if(dead) return;

    /* 4 ── "Welcome to traits.build" ── */
    await say('Welcome to traits dot build.');
    if(dead) return;
    await wait(250);

    /* 5 ── Logo sparkles in ── */
    $('hp-logo').classList.add('on');
    await wait(350);
    $('hp-logo').classList.add('sparkle');
    sparkle();
    await wait(600);
    if(dead) return;

    /* 6 ── "We help everyone build anything" ── */
    $('hp-sub').classList.add('on');
    $('hp-sub').textContent = 'We help everyone build anything.';
    await say('We help everyone build anything.');
    if(dead) return;
    await wait(250);

    /* 7 ── Button 1 ── */
    const btns = document.querySelectorAll('.hero-btn');
    btns[0].classList.add('on');
    await say('An AI first developer experience.');
    if(dead) return;
    await wait(200);

    /* 8 ── Button 2 ── */
    btns[1].classList.add('on');
    await say('Running on your mobile device.');
    if(dead) return;
    await wait(200);

    /* 9 ── Button 3 ── */
    btns[2].classList.add('on');
    await say('Fully private and open source.');
    if(dead) return;
    await wait(350);

    /* 10 ── Ask for mic ── */
    $('hp-sub').textContent = '';
    await say('If you would like to interact with me, please enable your microphone.');
    if(dead) return;

    /* 11 ── Wait for STT to finish loading ── */
    try{ sttFn = await sttP; }catch(e){ console.warn('[HP] STT load fail:', e); }
    if(dead) return;

    if(!sttFn){
      /* STT unavailable — fall back to clickable buttons */
      $('hp-sub').textContent = 'Click a topic to explore.';
      linkBtns();
      $('hp-skip').style.display='none';
      return;
    }

    /* 12 ── Listen loop ── */
    const confirms = [
      '',
      'Great choice! The AI first developer experience is at the heart of everything we build.',
      'Absolutely! traits dot build runs natively in your browser via WebAssembly, even on mobile.',
      'That is right! Everything runs locally in your browser. No data leaves your device.'
    ];

    while(!dead){
      /* reset button states */
      btns.forEach(b=>{ b.classList.remove('selected','dim'); b.onclick=null; });
      $('hp-ts').textContent = ''; $('hp-ts').classList.remove('on');

      $('hp-mic').classList.add('on');
      $('hp-sub').textContent = 'Speak your interest…';

      /* race: voice vs click */
      let clickRes = null;
      const clickP = new Promise(r=>{ clickRes=r; });
      btns.forEach(b=>{
        b.addEventListener('click',()=>clickRes(parseInt(b.dataset.topic)),{once:true});
      });

      const speechP = hear().then(text=>{
        $('hp-ts').textContent = text ? '\u201C'+text+'\u201D' : '';
        $('hp-ts').classList.add('on');
        return match(text) || -1;
      });

      const sel = await Promise.race([speechP, clickP]);
      if(dead) return;
      $('hp-mic').classList.remove('on');

      /* handle selection */
      if(sel>0 && sel<=3){
        btns.forEach((b,i)=>{
          if(i===sel-1) b.classList.add('selected');
          else b.classList.add('dim');
        });
        sparkle();
        await say(confirms[sel]);
        if(dead) return;

        $('hp-sub').textContent = 'Try another, or click to explore.';
        await wait(2500);
        if(dead) return;
      } else {
        $('hp-sub').textContent = '';
        await say('To have real time conversations on the platform, you can register, or add your personal OpenAI API key under settings, where it stays securely stored only in your browser.');
        if(dead) return;

        $('hp-sub').textContent = 'Visit Settings to add your key, or try again.';
        await wait(2500);
        if(dead) return;
      }
    }

    $('hp-skip').style.display='none';

  }catch(err){
    console.error('[HP] Error:', err);
    /* fallback: show static */
    $('hp-loading').classList.add('done');
    $('hp-exp').classList.add('on');
    $('hp-logo').classList.add('on');
    $('hp-sub').classList.add('on');
    $('hp-sub').textContent = 'Composable function kernel — CLI, REST, MCP, WASM';
    document.querySelectorAll('.hero-btn').forEach(b=>b.classList.add('on'));
    linkBtns();
    $('hp-skip').style.display='none';
  }
})();
</script>
</body>
</html>"####;
