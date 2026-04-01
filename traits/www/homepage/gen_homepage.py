#!/usr/bin/env python3
"""Generate homepage.rs with embedded pre-recorded audio."""
import base64, os

AUDIO_DIR = os.path.join(os.path.dirname(__file__), 'audio')
CLIPS = ['welcome','tagline','btn1','btn2','btn3','mic_prompt','confirm1','confirm2','confirm3','fallback']

# Read all base64 data
audio_lines = []
for name in CLIPS:
    b64 = open(os.path.join(AUDIO_DIR, name + '.b64')).read().strip()
    audio_lines.append(f"{name}:'{b64}'")
audio_js_map = ',\n'.join(audio_lines)

# HTML template (everything between r####" and "####)
HTML = r'''<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>traits.build</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
html,body{width:100%;height:100%;overflow:hidden;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif}
body{background:#000;color:#fff}

#hp-loading{position:fixed;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;z-index:10;background:#000;transition:opacity .8s}
#hp-loading.done{opacity:0;pointer-events:none}
.loader-ring{width:44px;height:44px;border:3px solid rgba(255,255,255,.08);border-top-color:#7c5cfc;border-radius:50%;animation:spin .9s linear infinite}
@keyframes spin{to{transform:rotate(360deg)}}
#hp-load-text{margin-top:18px;color:rgba(255,255,255,.4);font-size:.85rem;text-align:center;max-width:300px}
#hp-progress{width:220px;height:3px;background:rgba(255,255,255,.08);border-radius:2px;overflow:hidden;margin-top:14px}
#hp-progress-bar{height:100%;width:0;background:linear-gradient(90deg,#7c5cfc,#00d4ff);transition:width .3s}

#hp-exp{position:fixed;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:20px;opacity:0;transition:opacity .8s}
#hp-exp.on{opacity:1}

#hp-logo{font-size:clamp(2.8rem,8vw,5rem);font-weight:800;letter-spacing:-.03em;opacity:0;transform:scale(.85);transition:all 1.2s cubic-bezier(.17,.67,.35,1.15)}
#hp-logo.on{opacity:1;transform:scale(1)}
#hp-logo .t{background:linear-gradient(135deg,#7c5cfc,#00d4ff);-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text}
#hp-logo .d{color:rgba(255,255,255,.35)}
#hp-logo.sparkle{filter:drop-shadow(0 0 30px rgba(124,92,252,.5)) drop-shadow(0 0 60px rgba(0,212,255,.25))}

#hp-sub{font-size:clamp(.95rem,2.8vw,1.3rem);color:rgba(255,255,255,.55);opacity:0;transform:translateY(8px);transition:all .5s;text-align:center;max-width:560px;padding:0 20px;min-height:1.6em;line-height:1.5}
#hp-sub.on{opacity:1;transform:translateY(0)}

#hp-btns{display:flex;flex-direction:column;gap:14px;align-items:center;margin-top:12px}
.hero-btn{display:block;padding:16px 32px;font-size:clamp(.9rem,2.4vw,1.15rem);font-weight:600;color:#fff;background:rgba(255,255,255,.05);border:1px solid rgba(255,255,255,.1);border-radius:14px;cursor:pointer;opacity:0;transform:translateY(28px);transition:all .6s cubic-bezier(.17,.67,.35,1.15);min-width:300px;text-align:center;-webkit-backdrop-filter:blur(10px);backdrop-filter:blur(10px);font-family:inherit}
.hero-btn.on{opacity:1;transform:translateY(0)}
.hero-btn:hover{background:rgba(124,92,252,.15);border-color:rgba(124,92,252,.4);transform:translateY(-2px)}
.hero-btn.selected{background:rgba(124,92,252,.25);border-color:#7c5cfc;box-shadow:0 0 30px rgba(124,92,252,.3);transform:translateY(-2px)}
.hero-btn.dim{opacity:.25;pointer-events:none}

#hp-mic{display:flex;flex-direction:column;align-items:center;gap:10px;margin-top:16px;opacity:0;transition:opacity .5s}
#hp-mic.on{opacity:1}
.mic-ring{width:56px;height:56px;border-radius:50%;background:rgba(124,92,252,.12);border:2px solid rgba(124,92,252,.45);display:flex;align-items:center;justify-content:center;animation:mp 2s ease-in-out infinite}
.mic-ring svg{width:22px;height:22px;fill:#7c5cfc}
@keyframes mp{0%,100%{box-shadow:0 0 0 0 rgba(124,92,252,.25)}50%{box-shadow:0 0 0 18px rgba(124,92,252,0)}}
#hp-mic p{color:rgba(255,255,255,.4);font-size:.82rem}

#hp-ts{font-size:.95rem;color:rgba(255,255,255,.6);font-style:italic;min-height:1.4em;opacity:0;transition:opacity .4s;text-align:center;padding:0 20px}
#hp-ts.on{opacity:1}

#hp-skip{position:fixed;bottom:20px;right:24px;color:rgba(255,255,255,.2);font-size:.78rem;text-decoration:none;z-index:20;transition:color .2s}
#hp-skip:hover{color:rgba(255,255,255,.45)}

.spark{position:fixed;width:4px;height:4px;border-radius:50%;pointer-events:none;animation:fup 2.5s ease-out forwards}
@keyframes fup{0%{opacity:1;transform:translateY(0) scale(1)}100%{opacity:0;transform:translateY(-180px) scale(0)}}

@media(max-width:600px){.hero-btn{min-width:unset;width:88vw;padding:14px 18px}#hp-btns{gap:10px}}
</style>
</head>
<body>

<div id="hp-loading">
  <div class="loader-ring"></div>
  <p id="hp-load-text">Loading&hellip;</p>
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
    <p id="hp-mic-label">Listening&hellip;</p>
  </div>
  <p id="hp-ts"></p>
</div>

<a id="hp-skip" href="#">Skip intro</a>

<script>
/* pre-recorded audio (base64 MP3, OpenAI TTS alloy voice) */
const A={
__AUDIO_MAP__
};
function au(k){return 'data:audio/mpeg;base64,'+A[k];}

(async function(){
  let dead=false, curAudio=null, micStr=null, aCtx=null;
  const $=id=>document.getElementById(id);
  const wait=ms=>new Promise(r=>{if(!dead)setTimeout(r,ms);});

  window._pageCleanup=()=>{
    dead=true;
    if(curAudio){try{curAudio.pause();}catch(_){} curAudio=null;}
    if(micStr){micStr.getTracks().forEach(t=>t.stop()); micStr=null;}
    if(aCtx){try{aCtx.close();}catch(_){} aCtx=null;}
  };

  function prog(pct,txt){
    if(txt) $('hp-load-text').textContent=txt;
    $('hp-progress-bar').style.width=pct+'%';
  }

  function showAll(){
    dead=true;
    if(curAudio){try{curAudio.pause();}catch(_){}}
    $('hp-loading').classList.add('done');
    $('hp-exp').classList.add('on');
    $('hp-logo').classList.add('on','sparkle');
    $('hp-sub').classList.add('on');
    $('hp-sub').textContent='Composable function kernel \u2014 CLI, REST, MCP, WASM';
    document.querySelectorAll('.hero-btn').forEach(b=>b.classList.add('on'));
    $('hp-skip').style.display='none';
    linkBtns();
  }
  $('hp-skip').onclick=e=>{e.preventDefault();showAll();};

  function linkBtns(){
    document.querySelectorAll('.hero-btn').forEach(btn=>{
      btn.onclick=()=>{
        const t=btn.dataset.topic;
        if(t==='1') location.hash='/docs';
        else if(t==='2') location.hash='/terminal';
        else location.hash='/playground';
      };
    });
  }

  function play(key){
    return new Promise(r=>{
      if(dead||!A[key]){r();return;}
      curAudio=new Audio(au(key));
      curAudio.onended=()=>{curAudio=null;r();};
      curAudio.onerror=()=>{curAudio=null;r();};
      curAudio.play().catch(()=>r());
    });
  }

  function sayBrowser(text){
    return new Promise(r=>{
      if(dead||!window.speechSynthesis){r();return;}
      const u=new SpeechSynthesisUtterance(text);
      u.rate=1.0;u.pitch=1.0;
      u.onend=r;u.onerror=r;
      speechSynthesis.speak(u);
    });
  }

  function sparkle(){
    const el=$('hp-logo'),rc=el.getBoundingClientRect();
    for(let i=0;i<14;i++){
      const p=document.createElement('div');
      p.className='spark';
      p.style.left=(rc.left+Math.random()*rc.width)+'px';
      p.style.top=(rc.top+Math.random()*rc.height)+'px';
      p.style.background=Math.random()>.5?'#7c5cfc':'#00d4ff';
      p.style.animationDelay=(Math.random()*.6)+'s';
      document.body.appendChild(p);
      setTimeout(()=>p.remove(),3200);
    }
  }

  function resample(data,from){
    if(from===16000) return data;
    const r=from/16000,n=Math.round(data.length/r),o=new Float32Array(n);
    for(let i=0;i<n;i++){const s=i*r,lo=Math.floor(s),hi=Math.min(lo+1,data.length-1);o[i]=data[lo]*(1-(s-lo))+data[hi]*(s-lo);}
    return o;
  }
  function merge(arrs){
    const n=arrs.reduce((s,a)=>s+a.length,0),o=new Float32Array(n);
    let off=0;for(const a of arrs){o.set(a,off);off+=a.length;}return o;
  }

  let sttFn=null;
  async function hear(){
    if(dead||!sttFn) return '';
    try{
      micStr=await navigator.mediaDevices.getUserMedia({
        audio:{echoCancellation:true,noiseSuppression:true,autoGainControl:true}
      });
    }catch(e){console.warn('[HP] Mic:',e);return '';}
    if(dead){micStr.getTracks().forEach(t=>t.stop());return '';}

    aCtx=new AudioContext();
    const sr=aCtx.sampleRate;
    const src=aCtx.createMediaStreamSource(micStr);
    const proc=aCtx.createScriptProcessor(4096,1,1);
    src.connect(proc);proc.connect(aCtx.destination);

    return new Promise(res=>{
      let buf=[],ss=0,sil=0,done=false;
      const TH=0.015,SIL=1200,MIN=500;
      proc.onaudioprocess=async e=>{
        if(done||dead) return;
        const inp=e.inputBuffer.getChannelData(0);
        const rms=Math.sqrt(inp.reduce((s,v)=>s+v*v,0)/inp.length);
        if(rms>TH){if(!ss) ss=Date.now();sil=0;buf.push(new Float32Array(inp));}
        else if(ss){
          buf.push(new Float32Array(inp));
          if(!sil) sil=Date.now();
          else if(Date.now()-sil>SIL&&Date.now()-ss>MIN){
            done=true;
            proc.disconnect();src.disconnect();
            micStr.getTracks().forEach(t=>t.stop());micStr=null;
            aCtx.close().catch(()=>{});aCtx=null;
            const pcm=merge(buf);
            if(pcm.length<8000){res('');return;}
            try{const r=await sttFn(resample(pcm,sr),{language:'en'});res((r.text||'').trim());}
            catch(err){console.warn('[HP] STT:',err);res('');}
          }
        }
      };
      setTimeout(()=>{
        if(!done){done=true;
          try{proc.disconnect();src.disconnect();}catch(_){}
          if(micStr){micStr.getTracks().forEach(t=>t.stop());micStr=null;}
          if(aCtx){aCtx.close().catch(()=>{});aCtx=null;}
          res('');
        }
      },15000);
    });
  }

  /* intent classification: try WebLLM (lean prompt), regex fallback */
  async function classify(text){
    if(!text) return 0;
    if(window._traitsSDK){
      try{
        const r=await window._traitsSDK.call('llm.prompt',[
          'Classify into one category. Reply ONLY with the number.\n'+
          '1=AI/developer/coding 2=mobile/device/browser 3=privacy/open-source 4=speed/realtime/API 0=other\n'+
          'Text: "'+text.replace(/"/g,'')+'"'
        ]);
        if(r&&r.ok){const n=parseInt(String(r.result).trim());if(n>=0&&n<=4) return n;}
      }catch(e){console.warn('[HP] LLM:',e);}
    }
    const t=text.toLowerCase();
    if(/\b(ai|developer|experience|code|coding|develop|first)\b/.test(t)) return 1;
    if(/\b(mobile|phone|device|run|running|tablet|android|ios|iphone)\b/.test(t)) return 2;
    if(/\b(private|open.?source|local|secure|privacy|extensible|free)\b/.test(t)) return 3;
    if(/\b(fast|faster|quick|speed|realtime|real.?time|cloud|api|openai|gpt)\b/.test(t)) return 4;
    return 0;
  }

  function hasApiKey(){
    try{return !!((localStorage.getItem('traits.secret.OPENAI_API_KEY')||'').trim()
           ||(localStorage.getItem('traits.voice.api_key')||'').trim());}
    catch(_){return false;}
  }
  function setVoiceMode(m){try{localStorage.setItem('traits.voice.mode',m);}catch(_){}}

  /* ═══════════════════════════════════════
     MAIN FLOW — instant intro, background STT
     ═══════════════════════════════════════ */
  try{
    /* 1 — start STT download in background */
    prog(10,'Starting\u2026');
    const sttP=(async()=>{
      try{
        const{pipeline}=await import('https://cdn.jsdelivr.net/npm/@huggingface/transformers');
        if(dead) return null;
        return pipeline('automatic-speech-recognition','onnx-community/whisper-base',{
          device:'wasm',
          dtype:{encoder_model:'fp32',decoder_model_merged:'q4'},
          progress_callback:p=>{
            if(p.progress!=null) prog(10+p.progress*.85,'Voice: '+Math.round(p.progress)+'%');
          }
        });
      }catch(e){console.warn('[HP] STT load:',e);return null;}
    })();

    /* 2 — transition immediately (audio is pre-recorded) */
    prog(100,'Ready');
    await wait(200);
    $('hp-loading').classList.add('done');
    $('hp-exp').classList.add('on');
    await wait(300);
    if(dead) return;

    /* 3 — welcome (instant pre-recorded) */
    await play('welcome');
    if(dead) return;
    await wait(200);

    /* 4 — logo */
    $('hp-logo').classList.add('on');
    await wait(350);
    $('hp-logo').classList.add('sparkle');
    sparkle();
    await wait(400);
    if(dead) return;

    /* 5 — tagline */
    $('hp-sub').classList.add('on');
    $('hp-sub').textContent='We help everyone build anything.';
    await play('tagline');
    if(dead) return;
    await wait(200);

    /* 6-8 — buttons */
    const btns=document.querySelectorAll('.hero-btn');
    btns[0].classList.add('on');
    await play('btn1');
    if(dead) return;
    await wait(150);
    btns[1].classList.add('on');
    await play('btn2');
    if(dead) return;
    await wait(150);
    btns[2].classList.add('on');
    await play('btn3');
    if(dead) return;
    await wait(250);

    /* 9 — mic prompt */
    $('hp-sub').textContent='';
    await play('mic_prompt');
    if(dead) return;

    /* 10 — wait for STT to finish loading */
    try{sttFn=await sttP;}catch(e){console.warn('[HP] STT fail:',e);}
    if(dead) return;

    if(!sttFn){
      $('hp-sub').textContent='Click a topic to explore.';
      linkBtns();
      $('hp-skip').style.display='none';
      return;
    }

    /* 11 — interactive listen loop */
    while(!dead){
      btns.forEach(b=>{b.classList.remove('selected','dim');b.onclick=null;});
      $('hp-ts').textContent='';$('hp-ts').classList.remove('on');

      $('hp-mic').classList.add('on');
      $('hp-sub').textContent='Speak your interest\u2026';

      let clickRes=null;
      const clickP=new Promise(r=>{clickRes=r;});
      btns.forEach(b=>{
        b.addEventListener('click',()=>clickRes(parseInt(b.dataset.topic)),{once:true});
      });

      const speechP=hear().then(async text=>{
        $('hp-ts').textContent=text?'\u201C'+text+'\u201D':'';
        $('hp-ts').classList.add('on');
        return text ? await classify(text) : -1;
      });

      const sel=await Promise.race([speechP,clickP]);
      if(dead) return;
      $('hp-mic').classList.remove('on');

      if(sel>0&&sel<=3){
        btns.forEach((b,i)=>{
          if(i===sel-1) b.classList.add('selected');
          else b.classList.add('dim');
        });
        sparkle();
        await play('confirm'+sel);
        if(dead) return;
        $('hp-sub').textContent='Try another, or click to explore.';
        await wait(2500);
        if(dead) return;
      } else if(sel===4){
        sparkle();
        if(hasApiKey()){
          $('hp-sub').textContent='API key found! Switching to realtime\u2026';
          await sayBrowser('Would you like to switch to realtime voice? Say yes.');
          if(dead) return;
          $('hp-mic').classList.add('on');
          $('hp-sub').textContent='Say yes to switch, or no to stay local.';
          const confirm=await hear();
          $('hp-mic').classList.remove('on');
          if(dead) return;
          if(confirm&&/\b(yes|yeah|yep|sure|ok|okay|do it|switch|absolutely|please)\b/i.test(confirm)){
            setVoiceMode('realtime');
            $('hp-sub').textContent='Switched to realtime voice!';
            await sayBrowser('Done! Voice set to realtime mode.');
          } else {
            $('hp-sub').textContent='Staying in local mode.';
            await sayBrowser('No problem, keeping local mode.');
          }
          if(dead) return;
          await wait(2500);
          if(dead) return;
        } else {
          $('hp-sub').textContent='';
          await sayBrowser('For faster responses, add your API key in settings.');
          if(dead) return;
          $('hp-sub').innerHTML='Visit <a href="#/settings" style="color:#7c5cfc;text-decoration:underline">Settings</a> to add your API key.';
          await wait(3000);
          if(dead) return;
        }
      } else {
        await play('fallback');
        if(dead) return;
        $('hp-sub').textContent='Click a button or speak again.';
        await wait(1500);
        if(dead) return;
      }
    }

    $('hp-skip').style.display='none';

  }catch(err){
    console.error('[HP] Error:',err);
    $('hp-loading').classList.add('done');
    $('hp-exp').classList.add('on');
    $('hp-logo').classList.add('on');
    $('hp-sub').classList.add('on');
    $('hp-sub').textContent='Composable function kernel \u2014 CLI, REST, MCP, WASM';
    document.querySelectorAll('.hero-btn').forEach(b=>b.classList.add('on'));
    linkBtns();
    $('hp-skip').style.display='none';
  }
})();
</script>
</body>
</html>'''

# Replace __AUDIO_MAP__ with actual audio data
HTML = HTML.replace('__AUDIO_MAP__', audio_js_map)

# Build final Rust file
RUST = f'''use serde_json::Value;

pub fn homepage(_args: &[Value]) -> Value {{
    Value::String(PAGE_HTML.to_string())
}}

const PAGE_HTML: &str = r####"{HTML}"####;
'''

out_path = os.path.join(os.path.dirname(__file__), 'homepage.rs')
with open(out_path, 'w') as f:
    f.write(RUST)

print(f"Written: {out_path}")
print(f"Size: {os.path.getsize(out_path)} bytes ({os.path.getsize(out_path)//1024}KB)")
print(f"Lines: {len(open(out_path).readlines())}")
