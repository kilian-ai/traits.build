/**
 * Spatial placement utilities.
 *
 * Provides helpers for:
 *  - Building object instances from prop definitions
 *  - Aligning objects along a track path
 *  - Side-by-side (lane) placement
 *  - Simple AABB intersection checking
 */

import { nanoid } from './nanoid.js';
import type { SceneObject, Vec3, Quat, EntityType } from '../schema/scene.js';
import type { PropDefinition } from '../schema/assets.js';
import { MENYOO_TYPE, QUAT_IDENTITY } from '../schema/scene.js';
import {
  moveAlongHeading,
  headingToRight,
  v3add,
  v3scale,
  quatFromHeading,
  quatFromEuler,
  joaat,
} from './transform.js';

// ─── ID Generation ────────────────────────────────────────────────────────────

let _objectCounter = 0;

export function resetCounter(): void {
  _objectCounter = 0;
}

export function nextId(prefix = 'obj'): string {
  return `${prefix}_${String(++_objectCounter).padStart(4, '0')}`;
}

// ─── Object Factory ───────────────────────────────────────────────────────────

export interface PlacementOptions {
  position: Vec3;
  /** GTA V heading (0=North, 90=East) */
  headingDeg?: number;
  /** Extra pitch (degrees, around local Y) */
  pitchDeg?: number;
  /** Extra roll (degrees, around local X) */
  rollDeg?: number;
  frozen?: boolean;
  dynamic?: boolean;
  textureVariation?: number;
  lodDistance?: number;
  health?: number;
  tags?: string[];
  groupId?: string;
  type?: EntityType;
}

export function makePropObject(
  prop: PropDefinition | string,
  opts: PlacementOptions,
): SceneObject {
  const model = typeof prop === 'string' ? prop : prop.name;
  const heading = opts.headingDeg ?? 0;
  const pitch   = opts.pitchDeg   ?? 0;
  const roll    = opts.rollDeg    ?? 0;

  let rotation: Quat;
  if (pitch === 0 && roll === 0) {
    rotation = quatFromHeading(heading);
  } else {
    rotation = quatFromEuler(heading, pitch, roll);
  }

  const tags = [...(opts.tags ?? [])];
  if (typeof prop !== 'string') {
    tags.push(...prop.tags.filter((t) => !tags.includes(t)));
  }

  return {
    id: nextId(),
    type: opts.type ?? 'prop',
    model,
    position: { ...opts.position },
    rotation,
    frozen: opts.frozen ?? true,
    dynamic: opts.dynamic ?? false,
    textureVariation: opts.textureVariation ?? 0,
    lodDistance: opts.lodDistance ?? 100,
    health: opts.health ?? -1,
    tags,
    groupId: opts.groupId,
  };
}

export function makeVehicleObject(
  modelName: string,
  opts: PlacementOptions,
): SceneObject {
  return makePropObject(modelName, {
    ...opts,
    type: 'vehicle',
    tags: ['vehicle', ...(opts.tags ?? [])],
  });
}

// ─── Track Placement Helpers ──────────────────────────────────────────────────

export interface TrackCursor {
  /** Current world position (start of next element) */
  position: Vec3;
  /** Track heading in degrees */
  heading: number;
  /** Accumulated track distance */
  distance: number;
}

/**
 * Advance a cursor forward by `metres` along the current heading.
 */
export function advanceCursor(cursor: TrackCursor, metres: number): TrackCursor {
  return {
    position: moveAlongHeading(cursor.position, cursor.heading, metres),
    heading: cursor.heading,
    distance: cursor.distance + metres,
  };
}

/**
 * Turn cursor heading by `deltaDeg`.
 */
export function turnCursor(cursor: TrackCursor, deltaDeg: number): TrackCursor {
  return {
    ...cursor,
    heading: (cursor.heading + deltaDeg + 360) % 360,
  };
}

/**
 * Place an object centred at the cursor position, oriented along heading.
 * Returns [object, advanced cursor] — cursor is moved forward by the prop length.
 */
export function placeAtCursor(
  prop: PropDefinition | string,
  cursor: TrackCursor,
  opts: Partial<PlacementOptions> = {},
): [SceneObject, TrackCursor] {
  const propLength = typeof prop === 'string' ? 0 : (prop.dimensions?.length ?? 0);

  const obj = makePropObject(prop, {
    position: cursor.position,
    headingDeg: cursor.heading,
    ...opts,
  });

  return [obj, advanceCursor(cursor, propLength)];
}

/**
 * Place objects evenly along both sides of the track at a position.
 * Useful for barriers, lights, cones.
 */
export function placeSideMarkers(
  prop: PropDefinition | string,
  cursor: TrackCursor,
  trackWidth: number,
  count: number,
  spacingMetres: number,
  opts: Partial<PlacementOptions> = {},
): SceneObject[] {
  const objects: SceneObject[] = [];
  const right = headingToRight(cursor.heading);
  const halfWidth = trackWidth / 2;

  for (let i = 0; i < count; i++) {
    const fwd = moveAlongHeading(cursor.position, cursor.heading, i * spacingMetres);
    const leftPos  = v3add(fwd, v3scale(right, -halfWidth));
    const rightPos = v3add(fwd, v3scale(right, halfWidth));

    objects.push(makePropObject(prop, { position: leftPos,  headingDeg: cursor.heading, ...opts }));
    objects.push(makePropObject(prop, { position: rightPos, headingDeg: cursor.heading, ...opts }));
  }
  return objects;
}

// ─── AABB Intersection ────────────────────────────────────────────────────────

interface AABB {
  minX: number; maxX: number;
  minY: number; maxY: number;
  minZ: number; maxZ: number;
}

function propAABB(obj: SceneObject, prop?: PropDefinition): AABB {
  const hw = (prop?.dimensions?.width  ?? 2) / 2;
  const hl = (prop?.dimensions?.length ?? 2) / 2;
  const hh = (prop?.dimensions?.height ?? 1) / 2;
  return {
    minX: obj.position.x - Math.max(hw, hl),
    maxX: obj.position.x + Math.max(hw, hl),
    minY: obj.position.y - Math.max(hw, hl),
    maxY: obj.position.y + Math.max(hw, hl),
    minZ: obj.position.z - hh,
    maxZ: obj.position.z + hh,
  };
}

function aabbOverlaps(a: AABB, b: AABB): boolean {
  return (
    a.minX < b.maxX && a.maxX > b.minX &&
    a.minY < b.maxY && a.maxY > b.minY &&
    a.minZ < b.maxZ && a.maxZ > b.minZ
  );
}

/**
 * Check if adding `candidate` to `existing` causes an AABB intersection.
 * Fast but approximate (treats all objects as axis-aligned boxes regardless of rotation).
 */
export function wouldIntersect(
  candidate: SceneObject,
  existing: SceneObject[],
): boolean {
  const ca = propAABB(candidate);
  for (const e of existing) {
    if (aabbOverlaps(ca, propAABB(e))) return true;
  }
  return false;
}
