#!/usr/bin/env tsx
/**
 * Generates the hand-crafted "Shredder Gauntlet" 7-jump progressive level.
 *
 * Run with:  npx tsx scripts/make-level.ts
 *
 * What it does:
 *  1. Builds a scene.json with 7 ramp+gap+landing sections
 *  2. Clears all existing .scene.json and .xml output files
 *  3. Writes the new scene.json and updates _index.json
 *
 * Track runs heading=0 (North/+Y) from GTA position (2200, 3500, 33).
 * Progressive gaps: 30 → 42 → 55 → 70 → 85 → 105 → 130 m
 * Progressive ramps: ramp_02 x2, ramp_adj_flip_mb x2, ramp_adj_flip_mb3 x3
 */

import { writeFileSync, readdirSync, unlinkSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

import { resetCounter, makePropObject, placeAtCursor, advanceCursor } from '../src/engine/placement.js';
import type { TrackCursor } from '../src/engine/placement.js';
import type { SceneObject, SceneGroup } from '../src/schema/scene.js';
import { PROPS_BY_NAME } from '../src/assets/props.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const OUT = resolve(__dirname, '../output');

// ── Helpers ───────────────────────────────────────────────────────────────────

let _grpCounter = 0;
function grpId(): string {
  return `grp_${String(++_grpCounter).padStart(4, '0')}`;
}

const START_X = 2200;
const START_Z = 33;

function mkCursor(y: number): TrackCursor {
  return { position: { x: START_X, y, z: START_Z }, heading: 0, distance: 0 };
}

const objects: SceneObject[] = [];
const groups: (SceneGroup & { name: string })[] = [];

function addGroup(id: string, name: string) {
  groups.push({ id, name, objectIds: [], tags: [] });
}

function place(model: string, cur: TrackCursor, gid: string, tags: string[]): TrackCursor {
  const prop = PROPS_BY_NAME.get(model);
  if (!prop) throw new Error(`Unknown prop: ${model}`);
  const [obj, next] = placeAtCursor(prop, cur, { groupId: gid, tags });
  objects.push(obj);
  return next;
}

// ── Build Track ───────────────────────────────────────────────────────────────

resetCounter();
let cur = mkCursor(3500);

// ── START ─────────────────────────────────────────────────────────────────────
{
  const gid = grpId();
  addGroup(gid, 'Start');
  cur = place('prop_start_gate_01', cur, gid, ['start', 'gate', 'checkpoint']);
  for (let i = 0; i < 4; i++) cur = place('prop_mp_long_pladge_01', cur, gid, ['road', 'approach']);
}

// ── JUMP SECTIONS ─────────────────────────────────────────────────────────────
interface JumpDef {
  name: string;
  approach: number;   // number of short pladges before the ramp
  ramp: string;       // model name
  gap: number;        // metres from ramp tip to landing pad back edge
  landing: number;    // number of short pladges after the gap
}

const JUMPS: JumpDef[] = [
  { name: 'Jump 1 — Warm-Up',       approach: 3, ramp: 'prop_racing_ramp_02',    gap:  30, landing: 4 },
  { name: 'Jump 2 — Getting Real',  approach: 3, ramp: 'prop_racing_ramp_02',    gap:  42, landing: 4 },
  { name: 'Jump 3 — Medium',        approach: 3, ramp: 'prop_ramp_adj_flip_mb',  gap:  55, landing: 4 },
  { name: 'Jump 4 — Hard',          approach: 4, ramp: 'prop_ramp_adj_flip_mb',  gap:  70, landing: 4 },
  { name: 'Jump 5 — Very Hard',     approach: 4, ramp: 'prop_ramp_adj_flip_mb3', gap:  85, landing: 5 },
  { name: 'Jump 6 — Extreme',       approach: 4, ramp: 'prop_ramp_adj_flip_mb3', gap: 105, landing: 5 },
  { name: 'Jump 7 — Legendary',     approach: 5, ramp: 'prop_ramp_adj_flip_mb3', gap: 130, landing: 6 },
];

for (const j of JUMPS) {
  const gid = grpId();
  addGroup(gid, j.name);

  // Approach road
  for (let i = 0; i < j.approach; i++)
    cur = place('prop_mp_short_pladge_01', cur, gid, ['road', 'approach']);

  // Ramp — placeAtCursor places at cursor (= center), advances by ramp.length
  const rampProp = PROPS_BY_NAME.get(j.ramp)!;
  const rampLen  = rampProp.dimensions!.length;  // e.g. 10, 12, or 14
  cur = place(j.ramp, cur, gid, ['ramp', 'jump', 'stunt']);

  // Skip the gap:
  //   After placeAtCursor(ramp), cursor = ramp_center + rampLen
  //   We want landing[0] center at: ramp_tip + gap + landingPad_halfLen
  //                               = (ramp_center + rampLen/2) + gap + 6
  //   skip = landing[0]_center - cursor
  //        = (ramp_center + rampLen/2 + gap + 6) - (ramp_center + rampLen)
  //        = gap + 6 - rampLen/2
  const skip = j.gap + 6 - rampLen / 2;
  cur = advanceCursor(cur, skip);

  // Landing pads
  for (let i = 0; i < j.landing; i++)
    cur = place('prop_mp_short_pladge_01', cur, gid, ['road', 'landing']);
}

// ── FINISH ────────────────────────────────────────────────────────────────────
{
  const gid = grpId();
  addGroup(gid, 'Finish');
  for (let i = 0; i < 3; i++) cur = place('prop_mp_long_pladge_01', cur, gid, ['road']);
  cur = place('prop_start_gate_01', cur, gid, ['finish', 'gate', 'checkpoint']);
}

// ── Build Scene JSON ──────────────────────────────────────────────────────────
const trackLength = cur.position.y - 3500;
const ID          = 'shredder-gauntlet-v1';
const NOW         = new Date().toISOString();

const scene = {
  metadata: {
    title:            'Shredder Gauntlet — 7 Progressive Jumps',
    description:      `7 hand-crafted jump sections with industrial shredders in each gap. `
                    + `Gaps grow from 30 m to 130 m; ramps escalate from racing to flip. `
                    + `Track length: ${trackLength} m.`,
    seed:             77777777,
    generatorType:    'stunt-track',
    generatedAt:      NOW,
    generatorVersion: '0.2.0',
    prompt:           'Handcrafted 7-jump shredder gauntlet, progressive difficulty',
  },
  params: {
    seed:          77777777,
    biome:         'urban',
    layout:        'straight',
    difficulty:    'extreme',
    density:       'medium',
    length:        trackLength,
    startPosition: { x: START_X, y: 3500, z: START_Z },
    heading:       0,
    features: {
      ramps: true, pits: false, barriers: false, vehicles: false,
      lights: false, checkpoints: true, loops: false, boostPads: false,
    },
    progression: {
      gapGrowth: 1.0, heightGrowth: 1.0, recoveryZones: false, speedBuildup: true,
    },
    style: {
      arcade: true, symmetric: false, neon: false, military: false, chaos: 0,
    },
  },
  objects,
  groups,
};

// ── Clear Old Output ──────────────────────────────────────────────────────────
const existing = readdirSync(OUT);
let deleted = 0;
for (const f of existing) {
  if (f.endsWith('.scene.json') || f.endsWith('.xml')) {
    unlinkSync(join(OUT, f));
    deleted++;
  }
}
if (deleted) console.log(`  Deleted ${deleted} old output file(s).`);

// ── Write Scene + Index ───────────────────────────────────────────────────────
const sceneFile = `${ID}.scene.json`;
writeFileSync(join(OUT, sceneFile), JSON.stringify(scene, null, 2));

const rampCount = JUMPS.length;
const index = [
  {
    id:          ID,
    title:       scene.metadata.title,
    prompt:      scene.metadata.prompt,
    seed:        scene.metadata.seed,
    difficulty:  'extreme',
    biome:       'urban',
    layout:      'straight',
    objectCount: objects.length,
    groups:      groups.length,
    ramps:       rampCount,
    vehicles:    0,
    sections:    rampCount,
    xmlFile:     '',
    jsonFile:    sceneFile,
    generatedAt: NOW,
    valid:       true,
    errorCount:  0,
    warningCount: 0,
  },
];
writeFileSync(join(OUT, '_index.json'), JSON.stringify(index, null, 2));

console.log(`✓ Shredder Gauntlet generated`);
console.log(`  Objects : ${objects.length}`);
console.log(`  Groups  : ${groups.length}`);
console.log(`  Length  : ${trackLength} m (y ${3500} → ${cur.position.y})`);
console.log(`  Written : output/${sceneFile}`);
