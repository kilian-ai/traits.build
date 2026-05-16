/**
 * Core scene graph schema for the GTA V map generation pipeline.
 * All coordinates use GTA V world space (meters, Y=north, Z=up).
 */

// ─── Primitives ────────────────────────────────────────────────────────────────

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

export interface Quat {
  x: number;
  y: number;
  z: number;
  w: number;
}

export interface BoundingBox {
  min: Vec3;
  max: Vec3;
}

// ─── Entity Types ──────────────────────────────────────────────────────────────

export type EntityType = 'prop' | 'vehicle' | 'ped';

/** Menyoo XML entity type codes */
export const MENYOO_TYPE: Record<EntityType, number> = {
  prop: 3,
  vehicle: 2,
  ped: 1,
};

// ─── Scene Enums ──────────────────────────────────────────────────────────────

export type Biome =
  | 'urban'
  | 'desert'
  | 'ocean'
  | 'mountain'
  | 'forest'
  | 'airport'
  | 'industrial'
  | 'military';

export type Difficulty = 'easy' | 'medium' | 'hard' | 'extreme';
export type Density = 'sparse' | 'medium' | 'dense';

export type TrackLayout =
  | 'straight'
  | 'curved'
  | 's-curve'
  | 'spiral'
  | 'figure8'
  | 'arena'
  | 'loop-de-loop';

export type GeneratorType =
  | 'stunt-track'
  | 'race-track'
  | 'arena'
  | 'obstacle-course'
  | 'military-base'
  | 'rooftop-parkour'
  | 'floating-city'
  | 'cinematic-scene';

// ─── Scene Object ─────────────────────────────────────────────────────────────

export interface SceneObject {
  /** Unique ID within scene */
  id: string;
  type: EntityType;
  /** GTA V internal model/hash name (e.g. prop_ramp_adj_flip_mb) */
  model: string;
  position: Vec3;
  /** Quaternion rotation */
  rotation: Quat;
  frozen: boolean;
  dynamic: boolean;
  textureVariation: number;
  lodDistance: number;
  /** -1 = max health */
  health: number;
  /** Semantic tags for grouping/filtering */
  tags: string[];
  /** Group ID this object belongs to */
  groupId?: string;
  /** Approximate bounding box (optional, used for validation) */
  bounds?: BoundingBox;
}

// ─── Scene Group ──────────────────────────────────────────────────────────────

export interface SceneGroup {
  id: string;
  name: string;
  /** Ordered list of object IDs */
  objectIds: string[];
  tags: string[];
}

// ─── Generation Parameters ────────────────────────────────────────────────────

export interface GenerationParams {
  /** Seed for deterministic generation */
  seed: number;
  biome: Biome;
  layout: TrackLayout;
  difficulty: Difficulty;
  density: Density;
  /** Track/scene length in meters */
  length: number;
  /** World-space starting position */
  startPosition: Vec3;
  /** Heading in degrees (0=north/+Y, 90=east/+X) */
  heading: number;
  features: FeatureFlags;
  progression: ProgressionParams;
  style: StyleParams;
}

export interface FeatureFlags {
  ramps: boolean;
  pits: boolean;
  barriers: boolean;
  vehicles: boolean;
  lights: boolean;
  checkpoints: boolean;
  loops: boolean;
  boostPads: boolean;
}

export interface ProgressionParams {
  /** 0–1: how much gap size grows along the track */
  gapGrowth: number;
  /** 0–1: how much ramp height increases */
  heightGrowth: number;
  /** Whether to include easy recovery zones */
  recoveryZones: boolean;
  /** Whether to build up speed before the first jump */
  speedBuildup: boolean;
}

export interface StyleParams {
  /** true=forgiving/arcade, false=sim-style */
  arcade: boolean;
  /** Mirror the track symmetrically */
  symmetric: boolean;
  /** Add neon/light elements */
  neon: boolean;
  /** Military aesthetic */
  military: boolean;
  /** Chaos factor 0–1 */
  chaos: number;
}

// ─── Prompt Interpretation Result ─────────────────────────────────────────────

export interface InterpretedPrompt {
  raw: string;
  params: GenerationParams;
  generatorType: GeneratorType;
  title: string;
  description: string;
  confidence: number;
}

// ─── Scene Description (top-level document) ───────────────────────────────────

export interface SceneMetadata {
  title: string;
  description: string;
  seed: number;
  generatorType: GeneratorType;
  generatedAt: string;
  generatorVersion: string;
  prompt?: string;
}

export interface SceneDescription {
  metadata: SceneMetadata;
  params: GenerationParams;
  objects: SceneObject[];
  groups: SceneGroup[];
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

export const VEC3_ZERO: Vec3 = { x: 0, y: 0, z: 0 };
export const QUAT_IDENTITY: Quat = { x: 0, y: 0, z: 0, w: 1 };

export function vec3(x: number, y: number, z: number): Vec3 {
  return { x, y, z };
}

export function quat(x: number, y: number, z: number, w: number): Quat {
  return { x, y, z, w };
}
