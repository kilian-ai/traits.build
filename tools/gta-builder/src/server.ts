/**
 * gta-builder UI server.
 *
 * Serves:
 *  - Static SPA at GET /
 *  - REST API at /api/...
 *
 * Run: npx tsx src/server.ts
 */

import express from 'express';
import { readFileSync, writeFileSync, existsSync, mkdirSync, unlinkSync, readdirSync } from 'fs';
import { resolve, basename } from 'path';
import { fileURLToPath } from 'url';
import { zipSync, strToU8 } from 'fflate';
import multer from 'multer';
import OpenAI from 'openai';

import { interpretPrompt } from './generators/prompt.js';
import { generateStuntTrack, type TrackOptions } from './generators/stunt-track.js';
import { exportMenyooXML, exportSceneJSON, getExportStats, injectReferenceCoords } from './export/menyoo.js';
import { validateScene } from './validate/index.js';

// ─── Paths ────────────────────────────────────────────────────────────────────

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const ROOT      = resolve(__dirname, '..');
const OUTPUT_DIR = resolve(ROOT, 'output');
const PUBLIC_DIR = resolve(ROOT, 'public');
const VIDEOS_DIR = resolve(ROOT, 'output', 'videos');
const INDEX_FILE = resolve(OUTPUT_DIR, '_index.json');

mkdirSync(OUTPUT_DIR, { recursive: true });
mkdirSync(VIDEOS_DIR, { recursive: true });

// ─── Startup: patch existing XMLs missing ReferenceCoords ────────────────────
{
  let patched = 0;
  for (const f of readdirSync(OUTPUT_DIR)) {
    if (!f.endsWith('.xml')) continue;
    const p        = resolve(OUTPUT_DIR, f);
    const original = readFileSync(p, 'utf8');
    const updated  = injectReferenceCoords(original);
    if (updated !== original) { writeFileSync(p, updated, 'utf8'); patched++; }
  }
  if (patched > 0) console.log(`[gta-builder] patched ReferenceCoords into ${patched} existing XML file(s)`);
}

// Multer for video uploads
const upload = multer({
  storage: multer.memoryStorage(),
  limits: { fileSize: 500 * 1024 * 1024 }, // 500MB
});

// OpenAI client (lazy — only instantiated when OPENAI_API_KEY is set)
function getOpenAI(): OpenAI {
  const key = process.env.OPENAI_API_KEY;
  if (!key) throw new Error('OPENAI_API_KEY environment variable is not set');
  return new OpenAI({ apiKey: key });
}

function formatTimestamp(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, '0')}`;
}

function parseTimestamp(ts: string): number {
  const parts = ts.split(':').map(Number);
  if (parts.length === 2) return (parts[0] || 0) * 60 + (parts[1] || 0);
  return Number(ts) || 0;
}

// ─── Challenge Index ──────────────────────────────────────────────────────────

interface ChallengeRecord {
  id: string;
  title: string;
  prompt: string;
  seed: number;
  difficulty: string;
  biome: string;
  layout: string;
  objectCount: number;
  groups: number;
  ramps: number;
  vehicles: number;
  sections: number;
  xmlFile: string;
  jsonFile: string;
  generatedAt: string;
  valid: boolean;
  errorCount: number;
  warningCount: number;
  basedOn?: string; // id of the challenge this was modified from
}

function loadIndex(): ChallengeRecord[] {
  if (!existsSync(INDEX_FILE)) return [];
  try {
    return JSON.parse(readFileSync(INDEX_FILE, 'utf8'));
  } catch {
    return [];
  }
}

function saveIndex(records: ChallengeRecord[]): void {
  writeFileSync(INDEX_FILE, JSON.stringify(records, null, 2), 'utf8');
}

// ─── App ──────────────────────────────────────────────────────────────────────

const app  = express();
const PORT = process.env.PORT ? parseInt(process.env.PORT) : 3742;

app.use(express.json({ limit: '50mb' }));

// Serve the SPA
app.use(express.static(PUBLIC_DIR));

// ─── API: List Challenges ─────────────────────────────────────────────────────

app.get('/api/challenges', (_req, res) => {
  const records = loadIndex();
  res.json(records);
});

// ─── Shared generation helper ────────────────────────────────────────────────

function sanitiseOptions(options?: TrackOptions): TrackOptions | undefined {
  if (!options) return undefined;
  return {
    grid: {
      vehicleCount: typeof options.grid?.vehicleCount === 'number' ? options.grid.vehicleCount : undefined,
      rowSpacing:   typeof options.grid?.rowSpacing   === 'number' ? options.grid.rowSpacing   : undefined,
    },
    approachLength:     typeof options.approachLength     === 'number' ? options.approachLength     : undefined,
    finishRunwayLength: typeof options.finishRunwayLength === 'number' ? options.finishRunwayLength : undefined,
  };
}

function runGeneration(
  prompt: string,
  seed: number | undefined,
  difficulty: string | undefined,
  trackOptions: TrackOptions | undefined,
  basedOn?: string,
): { record: ChallengeRecord; diagnostics: unknown[] } {
  const interpreted = interpretPrompt(prompt, seed);
  if (difficulty && ['easy', 'medium', 'hard', 'extreme'].includes(difficulty)) {
    interpreted.params.difficulty = difficulty as typeof interpreted.params.difficulty;
  }

  const scene = generateStuntTrack(interpreted.params, trackOptions);
  scene.metadata.prompt = prompt;

  const validation = validateScene(scene);
  const xml   = exportMenyooXML(scene);
  const json  = exportSceneJSON(scene);
  const stats = getExportStats(scene, xml);

  const slug = scene.metadata.title.toLowerCase().replace(/[^a-z0-9]+/g, '-').slice(0, 48);
  const ts   = Date.now();
  const id   = `${slug}-${ts}`;

  const xmlFile  = resolve(OUTPUT_DIR, `${id}.xml`);
  const jsonFile = resolve(OUTPUT_DIR, `${id}.scene.json`);
  writeFileSync(xmlFile,  xml,  'utf8');
  writeFileSync(jsonFile, json, 'utf8');

  const record: ChallengeRecord = {
    id,
    title:        scene.metadata.title,
    prompt,
    seed:         interpreted.params.seed,
    difficulty:   interpreted.params.difficulty,
    biome:        interpreted.params.biome,
    layout:       interpreted.params.layout,
    objectCount:  stats.entityCount,
    groups:       scene.groups.length,
    ramps:        scene.objects.filter((o) => o.tags.includes('ramp')).length,
    vehicles:     stats.vehicles,
    sections:     scene.groups.filter((g) => g.tags.includes('section')).length,
    xmlFile:      basename(xmlFile),
    jsonFile:     basename(jsonFile),
    generatedAt:  scene.metadata.generatedAt,
    valid:        validation.valid,
    errorCount:   validation.errorCount,
    warningCount: validation.warningCount,
    ...(basedOn ? { basedOn } : {}),
  };

  return { record, diagnostics: validation.diagnostics };
}

// ─── API: Generate Challenge ──────────────────────────────────────────────────

app.post('/api/generate', (req, res) => {
  const { prompt, seed, difficulty, options } = req.body as {
    prompt?: string;
    seed?: number;
    difficulty?: string;
    options?: TrackOptions;
  };

  if (!prompt || typeof prompt !== 'string' || prompt.trim().length === 0) {
    res.status(400).json({ error: 'prompt is required' });
    return;
  }

  try {
    const { record, diagnostics } = runGeneration(
      prompt.trim(), seed, difficulty, sanitiseOptions(options),
    );
    const records = loadIndex();
    records.unshift(record);
    saveIndex(records);
    res.json({ record, diagnostics });
  } catch (err) {
    console.error(err);
    res.status(500).json({ error: String(err) });
  }
});

// ─── API: Modify Challenge ────────────────────────────────────────────────────

app.post('/api/challenges/:id/modify', (req, res) => {
  const records = loadIndex();
  const base    = records.find((r) => r.id === req.params.id);
  if (!base) { res.status(404).json({ error: 'Not found' }); return; }

  const { modificationPrompt, seed, difficulty, options } = req.body as {
    modificationPrompt?: string;
    seed?: number;
    difficulty?: string;
    options?: TrackOptions;
  };

  if (!modificationPrompt || typeof modificationPrompt !== 'string' || !modificationPrompt.trim()) {
    res.status(400).json({ error: 'modificationPrompt is required' });
    return;
  }

  // Combine the original prompt with the modification
  const combinedPrompt = `${base.prompt}. ${modificationPrompt.trim()}`;

  try {
    const { record, diagnostics } = runGeneration(
      combinedPrompt, seed, difficulty ?? base.difficulty, sanitiseOptions(options), base.id,
    );
    const allRecords = loadIndex();
    allRecords.unshift(record);
    saveIndex(allRecords);
    res.json({ record, diagnostics });
  } catch (err) {
    console.error(err);
    res.status(500).json({ error: String(err) });
  }
});

// ─── API: Export all as ZIP ───────────────────────────────────────────────────

app.get('/api/challenges/export', (_req, res) => {
  const records  = loadIndex();
  const available = records.filter((r) => existsSync(resolve(OUTPUT_DIR, r.xmlFile)));

  if (available.length === 0) {
    res.status(404).json({ error: 'No challenges to export' });
    return;
  }

  // Build a flat map of filename → Uint8Array for fflate
  const files: Record<string, Uint8Array> = {};
  for (const record of available) {
    const xml = injectReferenceCoords(readFileSync(resolve(OUTPUT_DIR, record.xmlFile), 'utf8'));
    // Organise by difficulty folder inside the ZIP
    files[`${record.difficulty}/${record.xmlFile}`] = strToU8(xml);
  }

  const zipped = zipSync(files, { level: 6 });

  res.setHeader('Content-Type', 'application/zip');
  res.setHeader('Content-Disposition', 'attachment; filename="gta-challenges.zip"');
  res.setHeader('Content-Length', zipped.byteLength);
  res.end(Buffer.from(zipped));
});

// ─── API: Scene JSON (for 3-D viewer) ────────────────────────────────────────

app.get('/api/challenges/:id/scene', (req, res) => {
  const records = loadIndex();
  const record  = records.find((r) => r.id === req.params.id);
  if (!record) { res.status(404).json({ error: 'Not found' }); return; }

  const jsonPath = resolve(OUTPUT_DIR, record.jsonFile);
  if (!existsSync(jsonPath)) { res.status(404).json({ error: 'Scene file missing' }); return; }

  res.setHeader('Content-Type', 'application/json');
  res.send(readFileSync(jsonPath, 'utf8'));
});

// ─── API: Download XML ────────────────────────────────────────────────────────

app.get('/api/challenges/:id/xml', (req, res) => {
  const records = loadIndex();
  const record  = records.find((r) => r.id === req.params.id);
  if (!record) { res.status(404).json({ error: 'Not found' }); return; }

  const xmlPath = resolve(OUTPUT_DIR, record.xmlFile);
  if (!existsSync(xmlPath)) { res.status(404).json({ error: 'File missing' }); return; }

  res.setHeader('Content-Type', 'application/xml');
  res.setHeader('Content-Disposition', `attachment; filename="${record.xmlFile}"`);
  res.send(readFileSync(xmlPath, 'utf8'));
});

// ─── API: Get raw XML text ────────────────────────────────────────────────────

app.get('/api/challenges/:id/xml/text', (req, res) => {
  const records = loadIndex();
  const record  = records.find((r) => r.id === req.params.id);
  if (!record) { res.status(404).json({ error: 'Not found' }); return; }

  const xmlPath = resolve(OUTPUT_DIR, record.xmlFile);
  if (!existsSync(xmlPath)) { res.status(404).json({ error: 'File missing' }); return; }

  res.json({ xml: readFileSync(xmlPath, 'utf8') });
});

// ─── API: Delete Challenge ────────────────────────────────────────────────────

app.delete('/api/challenges/:id', (req, res) => {
  let records = loadIndex();
  const record = records.find((r) => r.id === req.params.id);
  if (!record) { res.status(404).json({ error: 'Not found' }); return; }

  try {
    const xmlPath  = resolve(OUTPUT_DIR, record.xmlFile);
    const jsonPath = resolve(OUTPUT_DIR, record.jsonFile);
    if (existsSync(xmlPath))  unlinkSync(xmlPath);
    if (existsSync(jsonPath)) unlinkSync(jsonPath);
  } catch { /* ignore */ }

  records = records.filter((r) => r.id !== req.params.id);
  saveIndex(records);
  res.json({ ok: true });
});
// ─── API: Video Upload ───────────────────────────────────────────────────────

app.post('/api/challenges/:id/video', upload.single('video'), (req, res) => {
  const records = loadIndex();
  const record = records.find((r) => r.id === req.params.id);
  if (!record) { res.status(404).json({ error: 'Challenge not found' }); return; }

  if (!req.file) { res.status(400).json({ error: 'No video file provided' }); return; }

  try {
    const challengeVideoDir = resolve(VIDEOS_DIR, req.params.id);
    mkdirSync(challengeVideoDir, { recursive: true });

    const videoPath = resolve(challengeVideoDir, 'video.mp4');
    writeFileSync(videoPath, req.file.buffer);

    res.json({ ok: true, filename: 'video.mp4' });
  } catch (err) {
    console.error(err);
    res.status(500).json({ error: String(err) });
  }
});

// ─── API: Get Video Asset (stream) ───────────────────────────────────────────

app.get('/api/challenges/:id/video', (req, res) => {
  const videoPath = resolve(VIDEOS_DIR, req.params.id, 'video.mp4');
  if (!existsSync(videoPath)) { res.status(404).json({ error: 'No video' }); return; }
  res.setHeader('Content-Type', 'video/mp4');
  res.sendFile(videoPath);
});

// ─── API: Get persisted Video state (analysis + commentary + hasVideo) ───────

app.get('/api/challenges/:id/video/state', (req, res) => {
  const dir = resolve(VIDEOS_DIR, req.params.id);
  const videoPath        = resolve(dir, 'video.mp4');
  const analysisPath     = resolve(dir, 'analysis.json');
  const commentaryPath   = resolve(dir, 'commentary.txt');
  const segmentsPath     = resolve(dir, 'commentary.json');

  const state: {
    hasVideo: boolean;
    consolidated: { timestamp: string; description: string }[] | null;
    commentary: string | null;
    segments: { time: number; timestamp: string; text: string; hasAudio: boolean }[] | null;
  } = { hasVideo: false, consolidated: null, commentary: null, segments: null };

  if (existsSync(videoPath)) state.hasVideo = true;
  if (existsSync(analysisPath)) {
    try { state.consolidated = JSON.parse(readFileSync(analysisPath, 'utf8')); } catch { /* ignore */ }
  }
  if (existsSync(commentaryPath)) {
    state.commentary = readFileSync(commentaryPath, 'utf8');
  }
  if (existsSync(segmentsPath)) {
    try {
      const segs: { time: number; timestamp: string; text: string }[] = JSON.parse(readFileSync(segmentsPath, 'utf8'));
      state.segments = segs.map((s, idx) => ({
        ...s,
        hasAudio: existsSync(resolve(dir, 'audio', `seg-${idx}.mp3`)),
      }));
    } catch { /* ignore */ }
  }

  res.json(state);
});

// ─── API: Video Analysis (client sends extracted frames, server calls OpenAI vision) ──

interface VideoFrame {
  time: number;    // seconds from start of video
  dataUrl: string; // base64 data URL (image/jpeg or image/png)
}

app.post('/api/challenges/:id/video/analyze', express.json({ limit: '50mb' }), async (req, res) => {
  const records = loadIndex();
  const record = records.find((r) => r.id === req.params.id);
  if (!record) { res.status(404).json({ error: 'Challenge not found' }); return; }

  const { frames } = req.body as { frames?: VideoFrame[] };
  if (!Array.isArray(frames) || frames.length === 0) {
    res.status(400).json({ error: 'frames array is required' });
    return;
  }

  let openai: OpenAI;
  try {
    openai = getOpenAI();
  } catch (err) {
    res.status(503).json({ error: String(err) });
    return;
  }

  try {
    const SYSTEM = `You are a live sports commentator calling a GTA V stunt track challenge in real time.

CONTEXT: One driver attempts the SAME stunt track using a different vehicle each run — sports cars, supercars, motorcycles, monster trucks, muscle cars, off-roaders, planes, helicopters, etc. The big question every run: does this vehicle MAKE IT to the end, or does it WIPE OUT?

For each frame, deliver ONE punchy commentator-style sentence. Rules:
- Identify the VEHICLE specifically (make/model if recognizable, otherwise class — "the lifted monster truck", "the neon sportbike", "the lime-green hypercar", "the rusted pickup", "the agile rally car").
- NEVER mention "the player" or "the user". Speak about the VEHICLE as the protagonist ("the Bati screams up the ramp", "the Phoenix slams sideways into the rail").
- Use vivid, energetic commentator language: "screams", "rockets", "kisses the apex", "tumbles end-over-end", "threads the needle", "ate that wall", "stuck the landing", "absolutely sends it", "no chance", "clean as a whistle".
- Above all, track the SURVIVAL VERDICT — is this run still alive or is it cooked? Call out the make-or-break moments: clean landings = "MAKES IT", wipeouts/explosions/falls = "DOESN'T MAKE IT", sketchy saves = "somehow survives".
- Be specific about the action: ramp launches, mid-air rotations, hard landings, crashes, near-misses, boost pads, spin-outs, flips, explosions, bounces, gap clears.
- Start with an action verb or vehicle name. Present tense. No filler.

Examples:
- "The cobalt Adder rockets off the kicker, threads a clean 360, and STICKS the landing — still alive."
- "The lifted monster truck overcommits the loop, tumbles off the edge — game over, doesn't make it."
- "The neon Bati 801 kisses the rail mid-air, wobbles hard, but somehow saves it on the downramp."`;

    // Analyze frames in batches of 10 to avoid overwhelming the API
    const BATCH = 10;
    const segments: { time: number; startTime: string; description: string }[] = [];

    for (let i = 0; i < frames.length; i += BATCH) {
      const batch = frames.slice(i, i + BATCH);
      const results = await Promise.all(batch.map(async (frame) => {
        const match = frame.dataUrl.match(/^data:(image\/[a-z]+);base64,(.+)$/);
        if (!match) return { time: frame.time, description: 'Frame data invalid' };

        const mediaType = match[1] as 'image/jpeg' | 'image/png' | 'image/gif' | 'image/webp';
        const base64 = match[2];

        const response = await openai.chat.completions.create({
          model: 'gpt-4o',
          max_tokens: 120,
          messages: [{
            role: 'system',
            content: SYSTEM,
          }, {
            role: 'user',
            content: [{
              type: 'image_url',
              image_url: { url: `data:${mediaType};base64,${base64}`, detail: 'low' },
            }, {
              type: 'text',
              text: `Timestamp: ${formatTimestamp(frame.time)}. What is the player doing?`,
            }],
          }],
        });

        return {
          time: frame.time,
          description: response.choices[0]?.message?.content?.trim() ?? 'No description',
        };
      }));

      for (const r of results) {
        segments.push({ time: r.time, startTime: formatTimestamp(r.time), description: r.description });
      }
    }

    segments.sort((a, b) => a.time - b.time);
    const consolidated = segments.map(s => ({
      timestamp: s.startTime,
      description: s.description,
    }));

    // Persist (merge with any existing analysis from previous partial batches)
    try {
      const dir = resolve(VIDEOS_DIR, req.params.id);
      mkdirSync(dir, { recursive: true });
      const analysisPath = resolve(dir, 'analysis.json');
      let existing: { timestamp: string; description: string }[] = [];
      if (existsSync(analysisPath)) {
        try { existing = JSON.parse(readFileSync(analysisPath, 'utf8')); } catch { /* ignore */ }
      }
      // Merge: dedupe by timestamp (new wins)
      const byTs = new Map<string, { timestamp: string; description: string }>();
      for (const e of existing) byTs.set(e.timestamp, e);
      for (const c of consolidated) byTs.set(c.timestamp, c);
      const merged = [...byTs.values()].sort((a, b) => {
        const pa = a.timestamp.split(':').map(Number); const pb = b.timestamp.split(':').map(Number);
        return (pa[0] * 60 + pa[1]) - (pb[0] * 60 + pb[1]);
      });
      writeFileSync(analysisPath, JSON.stringify(merged, null, 2), 'utf8');
    } catch (err) {
      console.error('[analysis persist]', err);
    }

    res.json({ segments, consolidated });
  } catch (err) {
    console.error('[video analyze]', err);
    res.status(500).json({ error: String(err) });
  }
});

// ─── API: Generate timestamped live commentary segments + per-segment TTS ───

app.post('/api/challenges/:id/video/commentary', async (req, res) => {
  const dir = resolve(VIDEOS_DIR, req.params.id);
  const analysisPath = resolve(dir, 'analysis.json');
  if (!existsSync(analysisPath)) {
    res.status(404).json({ error: 'No analysis found — run analyze first' });
    return;
  }

  let consolidated: { timestamp: string; description: string }[];
  try {
    consolidated = JSON.parse(readFileSync(analysisPath, 'utf8'));
  } catch {
    res.status(500).json({ error: 'Failed to read analysis' });
    return;
  }
  if (!Array.isArray(consolidated) || consolidated.length === 0) {
    res.status(400).json({ error: 'Analysis is empty' });
    return;
  }

  let openai: OpenAI;
  try { openai = getOpenAI(); }
  catch (err) { res.status(503).json({ error: String(err) }); return; }

  // Limit to first 5 minutes (60 segments) during testing — remove cap for full runs
  const TEST_CAP = 60;
  const allIndexed = consolidated.map((s, i) => ({
    idx: i,
    time: parseTimestamp(s.timestamp),
    timestamp: s.timestamp,
    description: s.description,
  }));
  const indexed = allIndexed.slice(0, TEST_CAP);
  const N = indexed.length;

  // ── Generate commentary in batches of 20, carrying context between batches ──
  // Each batch is anchored to its frames (content sync) but receives the last
  // line of the previous batch so commentary flows continuously.
  const COMMENTARY_BATCH = 20;
  const COMMENTARY_SYSTEM = `You are an OVER-THE-TOP live sports commentator narrating a GTA V stunt track challenge at MAXIMUM intensity.

You will receive numbered timestamped frame descriptions and (optionally) the last line spoken before this batch. Write exactly one commentary line per frame, separated by [BREAK].

Rules:
- Exactly one line per frame — each line describes what is happening in THAT frame. Stay strictly synced to the action.
- Each line: 10–14 words. Short, punchy, breathless. Fits inside a 4-second window.
- The first line of each batch MUST flow naturally from the "previous line" context if provided.
- Use ALL CAPS for peaks. Use: "OH MY GOD!", "NO WAY!", "DISASTER!", "WHAT A SAVE!", "GONE!", "NAILS IT!".
- Describe the VEHICLE and ACTION — never say "the player".
- No timestamps, no numbers, no headings — just the raw call with [BREAK] separators.`;

  const allParts: string[] = [];
  let prevLine = '';
  try {
    for (let b = 0; b < indexed.length; b += COMMENTARY_BATCH) {
      const chunk = indexed.slice(b, b + COMMENTARY_BATCH);
      const M = chunk.length;
      const snapshotList = chunk
        .map((s, i) => `${i + 1}. [${s.timestamp}] ${s.description}`)
        .join('\n');
      const contextNote = prevLine
        ? `Last line spoken (continue from this):\n"${prevLine}"\n\n`
        : '';
      const response = await openai.chat.completions.create({
        model: 'gpt-4o',
        max_tokens: M * 50,
        messages: [
          { role: 'system', content: COMMENTARY_SYSTEM },
          { role: 'user',   content: `${contextNote}Frames:\n\n${snapshotList}\n\nWrite ${M} commentary lines, one per frame, separated by [BREAK].` },
        ],
      });
      const raw = response.choices[0]?.message?.content?.trim() ?? '';
      const parts = raw.split(/\s*\[BREAK\]\s*/i).map(p => p.trim()).filter(Boolean);
      for (let i = 0; i < M; i++) {
        allParts.push(parts[i] ?? chunk[i].description);
      }
      prevLine = allParts[allParts.length - 1] ?? '';
    }
  } catch (err) {
    console.error('[commentary]', err);
    res.status(500).json({ error: String(err) });
    return;
  }

  const segments: { idx: number; time: number; timestamp: string; text: string }[] =
    indexed.map((s, i) => ({
      idx: s.idx,
      time: s.time,
      timestamp: s.timestamp,
      text: allParts[i] ?? s.description,
    }));

  // Persist segments + consolidated text (for the modal)
  try {
    mkdirSync(dir, { recursive: true });
    writeFileSync(
      resolve(dir, 'commentary.json'),
      JSON.stringify(segments.map(({ idx: _idx, ...rest }) => rest), null, 2),
      'utf8',
    );
    const joined = segments.map(s => `[${s.timestamp}] ${s.text}`).join('\n');
    writeFileSync(resolve(dir, 'commentary.txt'), joined, 'utf8');
  } catch (err) {
    console.error('[commentary persist]', err);
  }

  // TTS each segment in batches of 4 (avoids rate-limit on parallel requests)
  const audioDir = resolve(dir, 'audio');
  mkdirSync(audioDir, { recursive: true });

  const TTS_INSTRUCTIONS = `Voice: MAXIMUM-ALERT live sports commentator at peak intensity — on fire, hyped to the limit, like calling the final lap of a championship race.

Energy: 11/10. Pure adrenaline. Sound shocked, thrilled, or devastated as the moment demands. NEVER calm, NEVER measured.

Pacing: FAST. Rapid-fire. Almost breathless. No careful enunciation — words tumble out.

Intonation: HUGE dynamic range. SHOUT the peaks ("OH MY GOODNESS", "NO WAY", "HE'S GOT IT", "DISASTER"). Punch action verbs (ROCKETS, SLAMS, FLIES, NAILS IT, GONE!).

Style: Formula 1 commentator + WWE announcer + NBA Finals buzzer-beater — loud, raw, urgent, in-the-moment.`;

  const BATCH_SIZE = 4;
  const ttsOk: boolean[] = new Array(segments.length).fill(false);
  for (let b = 0; b < segments.length; b += BATCH_SIZE) {
    const batch = segments.slice(b, b + BATCH_SIZE);
    const settled = await Promise.allSettled(batch.map(async (seg, bi) => {
      const realIdx = b + bi;
      const speech = await openai.audio.speech.create({
        model: 'gpt-4o-mini-tts',
        voice: 'ash',
        input: seg.text,
        instructions: TTS_INSTRUCTIONS,
        speed: 1.0,
      });
      const buf = Buffer.from(await speech.arrayBuffer());
      writeFileSync(resolve(audioDir, `seg-${realIdx}.mp3`), buf);
    }));
    settled.forEach((r, bi) => {
      ttsOk[b + bi] = r.status === 'fulfilled';
      if (r.status === 'rejected') console.error(`[tts seg-${b + bi}]`, (r as PromiseRejectedResult).reason);
    });
  }

  const segOut = segments.map((s, i) => ({
    time: s.time,
    timestamp: s.timestamp,
    text: s.text,
    hasAudio: ttsOk[i],
  }));

  res.json({
    commentary: segOut.map(s => `[${s.timestamp}] ${s.text}`).join('\n'),
    segments: segOut,
  });
});

// ─── API: Serve a single commentary segment's audio ─────────────────────────

app.get('/api/challenges/:id/video/audio/:idx', (req, res) => {
  const idx = parseInt(req.params.idx, 10);
  if (!Number.isFinite(idx) || idx < 0) { res.status(400).json({ error: 'bad idx' }); return; }
  const audioPath = resolve(VIDEOS_DIR, req.params.id, 'audio', `seg-${idx}.mp3`);
  if (!existsSync(audioPath)) { res.status(404).json({ error: 'No audio' }); return; }
  res.setHeader('Content-Type', 'audio/mpeg');
  res.sendFile(audioPath);
});

// ─── Start ────────────────────────────────────────────────────────────────────

app.listen(PORT, () => {
  console.log(`\n  gta-builder UI  →  http://localhost:${PORT}\n`);
});
