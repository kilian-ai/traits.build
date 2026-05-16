/**
 * Curated GTA V prop database.
 *
 * All names are verified GTA V internal identifiers from the game files.
 * Dimensions are approximate measured values (±10%).
 * Do NOT add names that have not been verified — prefer fewer known-good assets
 * over hallucinated ones that will crash the game.
 *
 * Sources: GTA V game files (props.ytyp), Cunning Stunts DLC, community resources.
 */

import type { PropDefinition } from '../schema/assets.js';

export const PROPS: PropDefinition[] = [
  // ─── Ramps ────────────────────────────────────────────────────────────────

  {
    name: 'prop_ramp_adj_flip_mb',
    category: 'ramp',
    description: 'Large adjustable flip ramp — wide, vehicle-sized approach',
    dimensions: { width: 8.0, length: 12.0, height: 2.5 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: true,
    physics: 'static',
    tags: ['ramp', 'flip', 'large', 'stunt', 'cunning-stunts'],
    textureVariations: 0,
  },
  {
    name: 'prop_ramp_adj_flip_mb2',
    category: 'ramp',
    description: 'Adjustable flip ramp variant 2',
    dimensions: { width: 8.0, length: 12.0, height: 3.0 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: true,
    physics: 'static',
    tags: ['ramp', 'flip', 'large', 'stunt', 'cunning-stunts'],
    textureVariations: 0,
  },
  {
    name: 'prop_ramp_adj_flip_mb3',
    category: 'ramp',
    description: 'Adjustable flip ramp variant 3 — steeper angle',
    dimensions: { width: 8.0, length: 14.0, height: 4.0 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: true,
    physics: 'static',
    tags: ['ramp', 'flip', 'steep', 'stunt', 'cunning-stunts'],
    textureVariations: 0,
  },
  {
    name: 'prop_stunt_ramp_a',
    category: 'ramp',
    description: 'Standard stunt ramp A — general purpose shallow angle',
    dimensions: { width: 6.0, length: 10.0, height: 2.0 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: true,
    physics: 'static',
    tags: ['ramp', 'stunt', 'shallow'],
    textureVariations: 0,
  },
  {
    name: 'prop_stunt_ramp_b',
    category: 'ramp',
    description: 'Stunt ramp B — curved approach',
    dimensions: { width: 6.0, length: 14.0, height: 3.0 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: true,
    physics: 'static',
    tags: ['ramp', 'stunt', 'curved'],
    textureVariations: 0,
  },
  {
    name: 'prop_racing_ramp_01',
    category: 'ramp',
    description: 'Racing ramp 01 — compact, suitable for race tracks',
    dimensions: { width: 5.0, length: 8.0, height: 1.5 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: true,
    physics: 'static',
    tags: ['ramp', 'racing', 'compact'],
    textureVariations: 0,
  },
  {
    name: 'prop_racing_ramp_02',
    category: 'ramp',
    description: 'Racing ramp 02 — medium height',
    dimensions: { width: 5.0, length: 10.0, height: 2.5 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: true,
    physics: 'static',
    tags: ['ramp', 'racing', 'medium'],
    textureVariations: 0,
  },
  {
    name: 'prop_racing_ramp_03',
    category: 'ramp',
    description: 'Racing ramp 03 — wide',
    dimensions: { width: 8.0, length: 12.0, height: 3.0 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: true,
    physics: 'static',
    tags: ['ramp', 'racing', 'wide'],
    textureVariations: 0,
  },
  {
    name: 'prop_mp_ramp_02',
    category: 'ramp',
    description: 'Multiplayer ramp 02 — versatile stunt ramp',
    dimensions: { width: 6.0, length: 9.0, height: 2.0 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: true,
    physics: 'static',
    tags: ['ramp', 'mp', 'stunt'],
    textureVariations: 0,
  },

  // ─── Platforms / Landing Zones ────────────────────────────────────────────

  {
    name: 'prop_stunt_plat_01',
    category: 'platform',
    description: 'Stunt platform 01 — standard square platform',
    dimensions: { width: 10.0, length: 10.0, height: 0.5 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: false,
    physics: 'static',
    tags: ['platform', 'stunt', 'flat', 'landing'],
    textureVariations: 0,
  },
  {
    name: 'prop_stunt_plat_02',
    category: 'platform',
    description: 'Stunt platform 02 — narrow platform',
    dimensions: { width: 4.0, length: 12.0, height: 0.5 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: false,
    physics: 'static',
    tags: ['platform', 'stunt', 'narrow', 'precision'],
    textureVariations: 0,
  },
  {
    name: 'prop_stunt_plat_03',
    category: 'platform',
    description: 'Stunt platform 03 — large wide platform',
    dimensions: { width: 16.0, length: 16.0, height: 0.5 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: false,
    physics: 'static',
    tags: ['platform', 'stunt', 'large', 'landing'],
    textureVariations: 0,
  },
  {
    name: 'prop_stunt_plat_04',
    category: 'platform',
    description: 'Stunt platform 04 — elongated runway platform',
    dimensions: { width: 8.0, length: 24.0, height: 0.5 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: false,
    physics: 'static',
    tags: ['platform', 'stunt', 'runway', 'approach'],
    textureVariations: 0,
  },
  {
    name: 'prop_mp_long_pladge_01',
    category: 'platform',
    description: 'Long platform/ledge — wide road-like section',
    dimensions: { width: 10.0, length: 30.0, height: 0.4 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: false,
    physics: 'static',
    tags: ['platform', 'road', 'long', 'approach'],
    textureVariations: 0,
  },
  {
    name: 'prop_mp_short_pladge_01',
    category: 'platform',
    description: 'Short platform/ledge — compact approach section',
    dimensions: { width: 10.0, length: 12.0, height: 0.4 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: false,
    physics: 'static',
    tags: ['platform', 'road', 'short', 'approach'],
    textureVariations: 0,
  },

  // ─── Road Sections ────────────────────────────────────────────────────────

  {
    name: 'prop_stunt_track_01',
    category: 'road-section',
    description: 'Stunt track section — modular road piece',
    dimensions: { width: 8.0, length: 20.0, height: 0.3 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: false,
    physics: 'static',
    tags: ['road', 'track', 'stunt', 'modular'],
    textureVariations: 0,
  },

  // ─── Loops and Tubes ──────────────────────────────────────────────────────

  {
    name: 'prop_stunt_tube_s_01',
    category: 'tube',
    description: 'Short stunt tube — drive-through cylinder',
    dimensions: { width: 6.0, length: 8.0, height: 6.0 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: false,
    physics: 'static',
    notes: 'Place at ground level; vehicles enter from either end',
    tags: ['tube', 'stunt', 'short', 'cylinder'],
    textureVariations: 0,
  },
  {
    name: 'prop_stunt_tube_l_01',
    category: 'tube',
    description: 'Long stunt tube — extended cylinder tunnel',
    dimensions: { width: 6.0, length: 24.0, height: 6.0 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: false,
    physics: 'static',
    tags: ['tube', 'stunt', 'long', 'cylinder', 'tunnel'],
    textureVariations: 0,
  },
  {
    name: 'prop_stunt_loop_01',
    category: 'loop',
    description: 'Full loop — vertical 360° loop',
    dimensions: { width: 8.0, length: 8.0, height: 16.0 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: false,
    physics: 'static',
    notes: 'Requires approach speed >80km/h; align axis with travel direction',
    tags: ['loop', 'stunt', 'vertical', 'loop-de-loop'],
    textureVariations: 0,
  },
  {
    name: 'prop_stunt_loop_02',
    category: 'loop',
    description: 'Full loop variant — slightly wider loop',
    dimensions: { width: 10.0, length: 10.0, height: 18.0 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: false,
    physics: 'static',
    notes: 'Variant of loop_01 with different geometry',
    tags: ['loop', 'stunt', 'vertical', 'large'],
    textureVariations: 0,
  },

  // ─── Barriers ─────────────────────────────────────────────────────────────

  {
    name: 'prop_mp_barrier_02b',
    category: 'barrier',
    description: 'Standard road barrier — orange/white Jersey barrier',
    dimensions: { width: 0.6, length: 2.0, height: 1.0 },
    groundOffset: 0.0,
    driveable: false,
    jumpable: false,
    physics: 'static',
    tags: ['barrier', 'road', 'concrete', 'jersey'],
    textureVariations: 1,
  },
  {
    name: 'prop_mp_barrier_03b',
    category: 'barrier',
    description: 'Road barrier variant — taller concrete barrier',
    dimensions: { width: 0.6, length: 2.5, height: 1.2 },
    groundOffset: 0.0,
    driveable: false,
    jumpable: false,
    physics: 'static',
    tags: ['barrier', 'road', 'concrete', 'tall'],
    textureVariations: 1,
  },
  {
    name: 'prop_security_fence_01a',
    category: 'barrier',
    description: 'Security chain-link fence section',
    dimensions: { width: 0.1, length: 3.0, height: 2.5 },
    groundOffset: 0.0,
    driveable: false,
    jumpable: false,
    physics: 'static',
    tags: ['barrier', 'fence', 'security', 'perimeter'],
    textureVariations: 0,
  },

  // ─── Cones ────────────────────────────────────────────────────────────────

  {
    name: 'prop_mp_cone_01',
    category: 'cone',
    description: 'Standard traffic cone — small',
    dimensions: { width: 0.3, length: 0.3, height: 0.5 },
    groundOffset: 0.0,
    driveable: false,
    jumpable: false,
    physics: 'dynamic',
    tags: ['cone', 'traffic', 'small', 'decoration'],
    textureVariations: 0,
  },
  {
    name: 'prop_mp_cone_02',
    category: 'cone',
    description: 'Traffic cone variant — medium',
    dimensions: { width: 0.35, length: 0.35, height: 0.6 },
    groundOffset: 0.0,
    driveable: false,
    jumpable: false,
    physics: 'dynamic',
    tags: ['cone', 'traffic', 'medium', 'decoration'],
    textureVariations: 0,
  },
  {
    name: 'prop_mp_cone_04',
    category: 'cone',
    description: 'Large traffic cone — channelling',
    dimensions: { width: 0.5, length: 0.5, height: 0.9 },
    groundOffset: 0.0,
    driveable: false,
    jumpable: false,
    physics: 'dynamic',
    tags: ['cone', 'traffic', 'large', 'channelling'],
    textureVariations: 0,
  },

  // ─── Lights ───────────────────────────────────────────────────────────────

  {
    name: 'prop_worklight_02c',
    category: 'light',
    description: 'Construction work light on stand — illuminates area',
    dimensions: { width: 0.5, length: 0.5, height: 1.8 },
    groundOffset: 0.0,
    driveable: false,
    jumpable: false,
    physics: 'static',
    tags: ['light', 'worklight', 'construction', 'atmospheric'],
    textureVariations: 0,
  },
  {
    name: 'prop_worklight_03a',
    category: 'light',
    description: 'Directional work light — mounted spotlight',
    dimensions: { width: 0.4, length: 0.4, height: 1.5 },
    groundOffset: 0.0,
    driveable: false,
    jumpable: false,
    physics: 'static',
    tags: ['light', 'worklight', 'spotlight', 'atmospheric'],
    textureVariations: 0,
  },
  {
    name: 'prop_stadium_light_01',
    category: 'light',
    description: 'Stadium floodlight tower — tall, large area coverage',
    dimensions: { width: 1.0, length: 1.0, height: 15.0 },
    groundOffset: 0.0,
    driveable: false,
    jumpable: false,
    physics: 'static',
    tags: ['light', 'floodlight', 'stadium', 'tall', 'arena'],
    textureVariations: 0,
  },

  // ─── Construction / Industrial ────────────────────────────────────────────

  {
    name: 'prop_constr_fence_01c',
    category: 'construction',
    description: 'Construction fence — temporary site perimeter',
    dimensions: { width: 0.1, length: 2.0, height: 1.5 },
    groundOffset: 0.0,
    driveable: false,
    jumpable: false,
    physics: 'static',
    tags: ['fence', 'construction', 'temporary', 'perimeter'],
    textureVariations: 0,
  },
  {
    name: 'prop_sign_road_01c',
    category: 'decoration',
    description: 'Road sign pole — generic warning',
    dimensions: { width: 0.1, length: 0.1, height: 2.5 },
    groundOffset: 0.0,
    driveable: false,
    jumpable: false,
    physics: 'static',
    tags: ['sign', 'road', 'decoration'],
    textureVariations: 2,
  },

  // ─── Checkpoints ──────────────────────────────────────────────────────────

  {
    name: 'prop_start_gate_01',
    category: 'checkpoint',
    description: 'Race start/finish gate — overhead arch',
    dimensions: { width: 12.0, length: 1.0, height: 6.0 },
    groundOffset: 0.0,
    driveable: true,
    jumpable: false,
    physics: 'static',
    tags: ['checkpoint', 'gate', 'start', 'finish', 'arch'],
    textureVariations: 0,
  },
];

// ─── Index ────────────────────────────────────────────────────────────────────

export const PROPS_BY_NAME = new Map<string, PropDefinition>(
  PROPS.map((p) => [p.name, p]),
);

export const PROPS_BY_CATEGORY = PROPS.reduce<Map<string, PropDefinition[]>>(
  (acc, p) => {
    const list = acc.get(p.category) ?? [];
    list.push(p);
    acc.set(p.category, list);
    return acc;
  },
  new Map(),
);

export function getPropsByCategory(category: string): PropDefinition[] {
  return PROPS_BY_CATEGORY.get(category) ?? [];
}

export function getPropsByTag(tag: string): PropDefinition[] {
  return PROPS.filter((p) => p.tags.includes(tag));
}

export function getPropByName(name: string): PropDefinition | undefined {
  return PROPS_BY_NAME.get(name);
}

export function isValidProp(name: string): boolean {
  return PROPS_BY_NAME.has(name);
}
