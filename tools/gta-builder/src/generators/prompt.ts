/**
 * Prompt interpretation layer.
 *
 * Converts natural-language strings into deterministic GenerationParams.
 * Does NOT call any LLM — uses keyword/pattern matching so output is
 * always stable and inspectable.
 *
 * Extend the keyword tables to improve coverage.
 */

import type {
  GenerationParams,
  InterpretedPrompt,
  FeatureFlags,
  ProgressionParams,
  StyleParams,
  Biome,
  Difficulty,
  Density,
  TrackLayout,
  GeneratorType,
  Vec3,
} from '../schema/scene.js';
import { deriveSeed } from '../engine/prng.js';

// ─── Biome Detection ──────────────────────────────────────────────────────────

function detectBiome(prompt: string): Biome {
  const p = prompt.toLowerCase();
  if (/ocean|sea|water|offshore|floating|island/.test(p))    return 'ocean';
  if (/desert|sand|dune|dry|arid/.test(p))                  return 'desert';
  if (/mountain|hill|cliff|slope|elevation/.test(p))        return 'mountain';
  if (/forest|jungle|trees|woodland/.test(p))               return 'forest';
  if (/airport|runway|terminal|hangar/.test(p))             return 'airport';
  if (/military|base|army|combat|barracks|checkpoint/.test(p)) return 'military';
  if (/industrial|warehouse|factory|dock|harbour/.test(p))  return 'industrial';
  return 'urban';
}

// ─── Layout Detection ─────────────────────────────────────────────────────────

function detectLayout(prompt: string): TrackLayout {
  const p = prompt.toLowerCase();
  if (/loop.*de.*loop|loop-de-loop|loop the loop/.test(p))  return 'loop-de-loop';
  if (/figure.?8|figure eight/.test(p))                    return 'figure8';
  if (/spiral|helix/.test(p))                              return 'spiral';
  if (/arena|circle|round|enclosed/.test(p))               return 'arena';
  if (/s.?curve|snake|serpentine|zigzag|slalom/.test(p))   return 's-curve';
  if (/curve|arc|bend|turn/.test(p))                       return 'curved';
  return 'straight';
}

// ─── Difficulty Detection ─────────────────────────────────────────────────────

function detectDifficulty(prompt: string): Difficulty {
  const p = prompt.toLowerCase();
  if (/extreme|impossible|insane|brutal|expert/.test(p))   return 'extreme';
  if (/hard|difficult|challenge|expert/.test(p))           return 'hard';
  if (/easy|beginner|simple|forgiving|gentle/.test(p))     return 'easy';
  return 'medium';
}

// ─── Density Detection ────────────────────────────────────────────────────────

function detectDensity(prompt: string): Density {
  const p = prompt.toLowerCase();
  if (/dense|packed|crowded|lots of|many/.test(p))         return 'dense';
  if (/sparse|few|minimal|open/.test(p))                   return 'sparse';
  return 'medium';
}

// ─── Generator Type Detection ─────────────────────────────────────────────────

function detectGeneratorType(prompt: string): GeneratorType {
  const p = prompt.toLowerCase();
  if (/military|base|checkpoint|barricade|guard/.test(p))  return 'military-base';
  if (/parkour|rooftop|roof|jump between buildings/.test(p)) return 'rooftop-parkour';
  if (/floating|sky|above|aerial|cyberpunk city/.test(p))  return 'floating-city';
  if (/survival|zombie|arena|wave|defend/.test(p))         return 'arena';
  if (/race|racing|track|circuit/.test(p))                 return 'race-track';
  if (/obstacle|course|challenge/.test(p))                 return 'obstacle-course';
  if (/cinematic|scene|movie|film/.test(p))                return 'cinematic-scene';
  return 'stunt-track';
}

// ─── Feature Detection ────────────────────────────────────────────────────────

function detectFeatures(prompt: string): FeatureFlags {
  const p = prompt.toLowerCase();
  return {
    ramps:       /ramp|jump|launch|boost/.test(p),
    pits:        /pit|gap|hole|drop|void/.test(p),
    barriers:    /barrier|wall|fence|block/.test(p),
    vehicles:    /vehicle|car|truck|lineup|fleet/.test(p),
    lights:      /light|neon|glow|lit|illuminat/.test(p),
    checkpoints: /checkpoint|gate|start|finish|waypoint/.test(p),
    loops:       /loop|tube|cylinder/.test(p),
    boostPads:   /boost|nitro|speed pad/.test(p),
  };
}

// ─── Style Detection ──────────────────────────────────────────────────────────

function detectStyle(prompt: string): StyleParams {
  const p = prompt.toLowerCase();
  return {
    arcade:    /arcade|forgiving|easy|casual/.test(p),
    symmetric: /symmetric|mirror|equal|both sides/.test(p),
    neon:      /neon|cyberpunk|futuristic|glow/.test(p),
    military:  /military|tactical|combat|army/.test(p),
    chaos:     /chaos|random|wild|crazy|mayhem/.test(p) ? 0.7 : 0.15,
  };
}

// ─── Track Length Extraction ──────────────────────────────────────────────────

function detectLength(prompt: string): number {
  // Look for explicit distance mentions
  const match = prompt.match(/(\d+)\s*(m|metre|meter|km|kilometre|kilometer)/i);
  if (match) {
    const val = parseFloat(match[1]);
    const unit = match[2].toLowerCase();
    if (unit.startsWith('k')) return val * 1000;
    return val;
  }
  // Infer from descriptor words
  const p = prompt.toLowerCase();
  if (/long|extended|massive|huge|epic/.test(p))  return 800;
  if (/short|quick|small|mini/.test(p))           return 200;
  return 400; // default
}

// ─── Start Position Selection ─────────────────────────────────────────────────

function detectStartPosition(biome: Biome): Vec3 {
  // Pre-chosen world positions that work well for each biome.
  // All Z values place the track above terrain/water.
  const positions: Record<Biome, Vec3> = {
    ocean:      { x: -3100, y:  -800, z: 50 },   // West ocean, above water
    desert:     { x:  2200, y:  3500, z: 33 },   // Grand Senora desert
    mountain:   { x:  -100, y:  5000, z: 500 },  // Chiliad area
    forest:     { x: -1200, y:  4000, z: 60 },   // Paleto Forest
    airport:    { x: -1000, y: -3000, z: 14 },   // LSIA area
    industrial: { x:  -800, y: -1500, z: 30 },   // LS docks
    military:   { x:  3000, y:  3500, z: 30 },   // Fort Zancudo region
    urban:      { x:   400, y:  -800, z: 65 },   // Downtown LS rooftop zone
  };
  return positions[biome];
}

// ─── Heading Detection ────────────────────────────────────────────────────────

function detectHeading(prompt: string): number {
  const p = prompt.toLowerCase();
  if (/north/.test(p)) return 0;
  if (/east/.test(p))  return 90;
  if (/south/.test(p)) return 180;
  if (/west/.test(p))  return 270;
  return 0; // default: northward
}

// ─── Title Derivation ─────────────────────────────────────────────────────────

function deriveTitle(prompt: string, generatorType: GeneratorType, biome: Biome): string {
  const cleaned = prompt.trim().replace(/[.!?]+$/, '');
  if (cleaned.length <= 60) return cleaned;
  // Fallback: derive from type and biome
  const typeLabel: Record<GeneratorType, string> = {
    'stunt-track':      'Stunt Track',
    'race-track':       'Race Circuit',
    'arena':            'Battle Arena',
    'obstacle-course':  'Obstacle Course',
    'military-base':    'Military Installation',
    'rooftop-parkour':  'Rooftop Parkour',
    'floating-city':    'Floating City',
    'cinematic-scene':  'Cinematic Scene',
  };
  return `${biome.charAt(0).toUpperCase() + biome.slice(1)} ${typeLabel[generatorType]}`;
}

// ─── Progression Params ───────────────────────────────────────────────────────

function deriveProgression(difficulty: Difficulty, prompt: string): ProgressionParams {
  const p = prompt.toLowerCase();
  const gapGrowth: Record<Difficulty, number> = {
    easy: 0.3, medium: 0.5, hard: 0.7, extreme: 0.9,
  };
  return {
    gapGrowth: gapGrowth[difficulty],
    heightGrowth: gapGrowth[difficulty] * 0.6,
    recoveryZones: difficulty === 'easy' || difficulty === 'medium',
    speedBuildup: !/no buildup|skip approach/.test(p),
  };
}

// ─── Main Interpreter ─────────────────────────────────────────────────────────

export function interpretPrompt(prompt: string, explicitSeed?: number): InterpretedPrompt {
  const biome          = detectBiome(prompt);
  const layout         = detectLayout(prompt);
  const difficulty     = detectDifficulty(prompt);
  const density        = detectDensity(prompt);
  const generatorType  = detectGeneratorType(prompt);
  const features       = detectFeatures(prompt);
  const style          = detectStyle(prompt);
  const length         = detectLength(prompt);
  const startPosition  = detectStartPosition(biome);
  const heading        = detectHeading(prompt);
  const title          = deriveTitle(prompt, generatorType, biome);
  const progression    = deriveProgression(difficulty, prompt);

  // Seed derived from prompt content unless overridden (reproducible)
  const seed = explicitSeed ?? deriveSeed(0, prompt);

  const params: GenerationParams = {
    seed,
    biome,
    layout,
    difficulty,
    density,
    length,
    startPosition,
    heading,
    features,
    progression,
    style,
  };

  return {
    raw: prompt,
    params,
    generatorType,
    title,
    description: `${generatorType.replace('-', ' ')} in ${biome} biome — ${difficulty} difficulty`,
    confidence: 0.85,
  };
}
