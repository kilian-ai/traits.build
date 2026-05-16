/**
 * Procedural stunt track generator.
 *
 * Generates a linear (or curved) stunt track consisting of:
 *   1. Speed-buildup approach
 *   2. Repeating jump sections (ramp → gap → landing)
 *   3. Optional loops, tubes, barriers, vehicle lineups
 *   4. Finish platform
 *
 * All placement is deterministic from the seed in GenerationParams.
 * Output is a SceneDescription ready for Menyoo XML export.
 */

import type {
  SceneDescription,
  SceneObject,
  SceneGroup,
  GenerationParams,
  SceneMetadata,
  Vec3,
} from '../schema/scene.js';
import {
  getPropsByCategory,
  getPropsByTag,
} from '../assets/props.js';
import { VEHICLES, VEHICLES_BY_SIZE } from '../assets/vehicles.js';
import { createPRNG, deriveSeed } from '../engine/prng.js';
import {
  moveAlongHeading,
  headingToRight,
  v3add,
  v3scale,
  estimateJumpTrajectory,
  minSpeedForGap,
} from '../engine/transform.js';
import type { PRNG } from '../engine/prng.js';
import {
  resetCounter,
  nextId,
  makePropObject,
  makeVehicleObject,
  advanceCursor,
  placeAtCursor,
  placeSideMarkers,
  type TrackCursor,
} from '../engine/placement.js';

// ─── Difficulty Params ────────────────────────────────────────────────────────

interface DifficultyConfig {
  /** Initial gap between ramps (metres) */
  initialGap: number;
  /** Maximum gap at track end */
  maxGap: number;
  /** Ramp angle for jump sections */
  rampAngleDeg: number;
  /** Platform (landing + approach) length */
  platformLength: number;
  /** Side barrier width */
  trackWidth: number;
}

const DIFFICULTY_CONFIG: Record<string, DifficultyConfig> = {
  easy:    { initialGap: 15, maxGap: 30,  rampAngleDeg: 20, platformLength: 24, trackWidth: 10 },
  medium:  { initialGap: 25, maxGap: 55,  rampAngleDeg: 30, platformLength: 20, trackWidth: 8  },
  hard:    { initialGap: 40, maxGap: 80,  rampAngleDeg: 35, platformLength: 16, trackWidth: 8  },
  extreme: { initialGap: 60, maxGap: 120, rampAngleDeg: 40, platformLength: 12, trackWidth: 6  },
};

// ─── Section Types ────────────────────────────────────────────────────────────

interface JumpSection {
  approachLength: number;
  gapLength: number;
  rampAngle: number;
  landingLength: number;
  index: number;
}

function planSections(params: GenerationParams, cfg: DifficultyConfig): JumpSection[] {
  const trackLength = params.length;
  const sectionLength = cfg.platformLength * 2 + 15; // rough section size
  const count = Math.max(3, Math.floor(trackLength / sectionLength));
  const sections: JumpSection[] = [];

  for (let i = 0; i < count; i++) {
    const t = count > 1 ? i / (count - 1) : 0;
    const gap = cfg.initialGap + (cfg.maxGap - cfg.initialGap) * t * params.progression.gapGrowth;
    sections.push({
      approachLength: cfg.platformLength,
      gapLength: gap,
      rampAngle: cfg.rampAngleDeg + t * 8 * params.progression.heightGrowth,
      landingLength: cfg.platformLength,
      index: i,
    });
  }
  return sections;
}

// ─── Element Builders ─────────────────────────────────────────────────────────

function buildApproach(
  cursor: TrackCursor,
  length: number,
  groupId: string,
  params: GenerationParams,
): { objects: SceneObject[]; cursor: TrackCursor } {
  const objects: SceneObject[] = [];
  const platformProps = getPropsByCategory('platform');
  const longPlat = platformProps.find((p) => p.name === 'prop_mp_long_pladge_01')
    ?? platformProps[0];
  const shortPlat = platformProps.find((p) => p.name === 'prop_mp_short_pladge_01')
    ?? platformProps[0];

  let cur = cursor;
  let remaining = length;

  while (remaining > 0) {
    const prop = remaining >= 28 ? longPlat : shortPlat;
    const [obj, next] = placeAtCursor(prop, cur, { groupId });
    objects.push(obj);
    cur = next;
    remaining -= prop.dimensions?.length ?? 12;
  }
  return { objects, cursor: cur };
}

function buildRamp(
  cursor: TrackCursor,
  rampAngle: number,
  groupId: string,
): { objects: SceneObject[]; cursor: TrackCursor; rampExitZ: number } {
  const objects: SceneObject[] = [];
  const rampProps = getPropsByCategory('ramp');

  // Pick ramp based on angle — steeper angles use flip ramps
  const rampProp = rampAngle >= 35
    ? (rampProps.find((p) => p.name === 'prop_ramp_adj_flip_mb3') ?? rampProps[0])
    : rampAngle >= 28
    ? (rampProps.find((p) => p.name === 'prop_ramp_adj_flip_mb') ?? rampProps[0])
    : (rampProps.find((p) => p.name === 'prop_racing_ramp_02') ?? rampProps[0]);

  const [obj, next] = placeAtCursor(rampProp, cursor, {
    groupId,
    tags: ['ramp', 'jump'],
  });
  objects.push(obj);

  // Approximate exit height of ramp
  const rampHeight = rampProp.dimensions?.height ?? 2.5;
  const rampExitZ  = cursor.position.z + rampHeight;

  return { objects, cursor: next, rampExitZ };
}

function buildLanding(
  cursor: TrackCursor,
  length: number,
  groupId: string,
): { objects: SceneObject[]; cursor: TrackCursor } {
  // Landing platform at same Z as approach (ramp brings vehicle back down)
  return buildApproach(cursor, length, groupId, {} as GenerationParams);
}

/**
 * Place 10 vehicles in a 2-wide racing grid BEHIND the start position.
 * Row 0 = 7 m behind start gate, row 4 = 35 m behind.
 * Vehicles are frozen (parked props), facing the track heading.
 */
function buildRacingGrid(
  startPosition: Vec3,
  heading: number,
  cfg: DifficultyConfig,
  rng: PRNG,
  groupId: string,
  vehicleCount: number = 10,
  rowSpacingM: number = 7,
): SceneObject[] {
  const objects: SceneObject[] = [];
  const rightDir = headingToRight(heading);
  const rowCount = Math.ceil(vehicleCount / 2);
  const rowSpacing = rowSpacingM;
  const sideOffset = Math.max(2.5, cfg.trackWidth * 0.25);

  // Prefer sports/super/muscle cars for the racing grid
  const raceCars = VEHICLES.filter((v) =>
    ['super', 'sports', 'muscle'].includes(v.vehicleClass),
  );
  // Shuffle for variety; fall back to non-trucks if we need more
  const shuffled = rng.shuffle([...raceCars]);
  const selected =
    shuffled.length >= rowCount * 2
      ? shuffled.slice(0, rowCount * 2)
      : [
          ...shuffled,
          ...rng
            .shuffle(VEHICLES.filter((v) => !['truck', 'military'].includes(v.vehicleClass)))
            .slice(0, rowCount * 2 - shuffled.length),
        ];

  for (let row = 0; row < rowCount; row++) {
    // Negative distance = behind the start gate
    const rowCenter = moveAlongHeading(startPosition, heading, -(row + 1) * rowSpacing);
    const leftPos  = v3add(rowCenter, v3scale(rightDir, -sideOffset));
    const rightPos = v3add(rowCenter, v3scale(rightDir,  sideOffset));

    const leftVeh  = selected[row * 2];
    const rightVeh = selected[row * 2 + 1];

    if (leftVeh) {
      objects.push(makeVehicleObject(leftVeh.name, {
        position: leftPos,
        headingDeg: heading,
        frozen: true,
        groupId,
        tags: ['vehicle', 'grid', 'racer', `row-${row + 1}`],
      }));
    }
    if (rightVeh) {
      objects.push(makeVehicleObject(rightVeh.name, {
        position: rightPos,
        headingDeg: heading,
        frozen: true,
        groupId,
        tags: ['vehicle', 'grid', 'racer', `row-${row + 1}`],
      }));
    }
  }
  return objects;
}

function buildBarriers(
  cursor: TrackCursor,
  sectionLength: number,
  trackWidth: number,
  groupId: string,
): SceneObject[] {
  const barrierProp = getPropsByCategory('barrier')
    .find((p) => p.name === 'prop_mp_barrier_02b')
    ?? getPropsByCategory('barrier')[0];

  if (!barrierProp) return [];

  const spacingM = barrierProp.dimensions?.length ?? 2;
  const count = Math.ceil(sectionLength / spacingM);
  return placeSideMarkers(barrierProp, cursor, trackWidth, count, spacingM, { groupId });
}

function buildLoopSection(
  cursor: TrackCursor,
  groupId: string,
): { objects: SceneObject[]; cursor: TrackCursor } {
  const objects: SceneObject[] = [];
  const loopProps = getPropsByCategory('loop');
  const loop = loopProps.find((p) => p.name === 'prop_stunt_loop_01') ?? loopProps[0];
  if (!loop) return { objects, cursor };

  // Add short approach tube then loop
  const tubeProp = getPropsByCategory('tube').find((p) => p.name === 'prop_stunt_tube_s_01');
  let cur = cursor;
  if (tubeProp) {
    const [tubeObj, afterTube] = placeAtCursor(tubeProp, cur, { groupId });
    objects.push(tubeObj);
    cur = afterTube;
  }

  const [loopObj, afterLoop] = placeAtCursor(loop, cur, { groupId });
  objects.push(loopObj);

  return { objects, cursor: afterLoop };
}

// ─── Public Options ─────────────────────────────────────────────────────────

export interface TrackGridOptions {
  /** Total vehicles in the racing grid (placed 2-wide). Default: 10 */
  vehicleCount?: number;
  /** Gap between grid rows in metres. Default: 7 */
  rowSpacing?: number;
}

export interface TrackOptions {
  grid?: TrackGridOptions;
  /** Acceleration runway before first jump (metres). Default: 80 */
  approachLength?: number;
  /** Runway after finish gate (metres). Default: 40 */
  finishRunwayLength?: number;
}

// ─── Main Generator ───────────────────────────────────────────────────────────

export function generateStuntTrack(params: GenerationParams, options?: TrackOptions): SceneDescription {
  resetCounter();

  const cfg = DIFFICULTY_CONFIG[params.difficulty] ?? DIFFICULTY_CONFIG.medium;
  const rng = createPRNG(deriveSeed(params.seed, 'stunt-track'));

  const allObjects: SceneObject[] = [];
  const groups: SceneGroup[] = [];

  // Resolve options with defaults
  const gridVehicleCount  = Math.max(2, Math.min(20, options?.grid?.vehicleCount  ?? 10));
  const gridRowSpacing    = Math.max(4, Math.min(14, options?.grid?.rowSpacing    ?? 7));
  const approachLen       = Math.max(20, Math.min(200, options?.approachLength    ?? 80));
  const finishRunwayLen   = Math.max(10, Math.min(120, options?.finishRunwayLength ?? 40));

  // ── Racing grid (always — vehicles lined up behind start gate) ────────────

  const gridGroupId = nextId('grp');
  const gridObjects = buildRacingGrid(
    params.startPosition,
    params.heading,
    cfg,
    rng,
    gridGroupId,
    gridVehicleCount,
    gridRowSpacing,
  );
  allObjects.push(...gridObjects);
  groups.push({
    id: gridGroupId,
    name: 'Racing Grid',
    objectIds: gridObjects.map((o) => o.id),
    tags: ['grid', 'vehicles', 'start'],
  });

  // ── Start gate + speed-buildup approach ──────────────────────────────────

  const approachGroupId = nextId('grp');
  const cursor: TrackCursor = {
    position: { ...params.startPosition },
    heading: params.heading,
    distance: 0,
  };

  let cur = cursor;
  const startObjects: SceneObject[] = [];

  // Always place start gate at the start line
  const startGateProp = getPropsByTag('checkpoint')[0];
  if (startGateProp) {
    const [gateObj, next] = placeAtCursor(startGateProp, cur, {
      groupId: approachGroupId,
      tags: ['start', 'gate', 'checkpoint'],
    });
    startObjects.push(gateObj);
    cur = next;
  }

  // 80m acceleration runway before first jump
  const { objects: approachObjs, cursor: afterApproach } = buildApproach(cur, approachLen, approachGroupId, params);
  startObjects.push(...approachObjs);
  cur = afterApproach;

  allObjects.push(...startObjects);
  groups.push({
    id: approachGroupId,
    name: 'Start Approach',
    objectIds: startObjects.map((o) => o.id),
    tags: ['approach', 'start'],
  });

  // ── Jump sections ─────────────────────────────────────────────────────────

  const sections = planSections(params, cfg);

  for (const section of sections) {
    const sectionGroupId = nextId('grp');
    const sectionObjects: SceneObject[] = [];

    // Approach
    const { objects: approachObjs, cursor: afterApproach } =
      buildApproach(cur, section.approachLength, sectionGroupId, params);
    sectionObjects.push(...approachObjs);
    cur = afterApproach;

    // Barriers along approach
    if (params.features.barriers) {
      const barriers = buildBarriers(
        { position: approachObjs[0]?.position ?? cur.position, heading: cur.heading, distance: cur.distance },
        section.approachLength,
        cfg.trackWidth,
        sectionGroupId,
      );
      sectionObjects.push(...barriers);
    }

    // Ramp
    const { objects: rampObjs, cursor: afterRamp, rampExitZ } =
      buildRamp(cur, section.rampAngle, sectionGroupId);
    sectionObjects.push(...rampObjs);
    cur = afterRamp;

    // Gap — advance cursor over void (no objects placed here)
    const trajectory = estimateJumpTrajectory(80, section.rampAngle);
    const gapLen = Math.min(section.gapLength, trajectory.horizontalDistance * 0.9);
    cur = advanceCursor(cur, gapLen);

    // Landing
    const { objects: landObjs, cursor: afterLanding } =
      buildLanding(cur, section.landingLength, sectionGroupId);
    sectionObjects.push(...landObjs);
    cur = afterLanding;

    // Optional: insert loop every 3rd section
    if (params.features.loops && section.index % 3 === 2) {
      const { objects: loopObjs, cursor: afterLoop } = buildLoopSection(cur, sectionGroupId);
      sectionObjects.push(...loopObjs);
      cur = afterLoop;
    }

    // Optional: recovery zone every other section on easy/medium
    if (params.progression.recoveryZones && section.index % 2 === 1) {
      const { objects: recoveryObjs, cursor: afterRecovery } =
        buildApproach(cur, 30, sectionGroupId, params);
      sectionObjects.push(...recoveryObjs);
      cur = afterRecovery;
    }


    allObjects.push(...sectionObjects);
    groups.push({
      id: sectionGroupId,
      name: `Jump Section ${section.index + 1} (gap: ${Math.round(section.gapLength)}m)`,
      objectIds: sectionObjects.map((o) => o.id),
      tags: ['section', 'jump', `gap-${Math.round(section.gapLength)}`],
    });
  }

  // ── Finish ────────────────────────────────────────────────────────────────

  const finishGroupId = nextId('grp');
  const finishObjects: SceneObject[] = [];
  const finishCursorStart = { ...cur };

  // Always place finish gate
  const finGateProp = getPropsByTag('checkpoint')[0];
  if (finGateProp) {
    const [finGateObj, afterFinGate] = placeAtCursor(finGateProp, cur, {
      groupId: finishGroupId,
      tags: ['finish', 'gate', 'checkpoint'],
    });
    finishObjects.push(finGateObj);
    cur = afterFinGate;
  }

  // Finish runway (40 m)
  const { objects: finPlat, cursor: afterFinPlat } = buildApproach(cur, finishRunwayLen, finishGroupId, params);
  finishObjects.push(...finPlat);
  cur = afterFinPlat;

  // Always place lights flanking the finish gate
  const lightProp = getPropsByCategory('light').find((p) => p.name === 'prop_worklight_02c');
  if (lightProp) {
    const lights = placeSideMarkers(lightProp, finishCursorStart, cfg.trackWidth, 6, 8, {
      groupId: finishGroupId,
      tags: ['light', 'finish'],
    });
    finishObjects.push(...lights);
  }

  allObjects.push(...finishObjects);
  groups.push({
    id: finishGroupId,
    name: 'Finish',
    objectIds: finishObjects.map((o) => o.id),
    tags: ['finish'],
  });

  // ── Assemble SceneDescription ──────────────────────────────────────────────

  const metadata: SceneMetadata = {
    title: `Stunt Track — ${params.difficulty} / ${params.layout}`,
    description: `Procedural stunt track with ${sections.length} jump sections, `
      + `initial gap ${cfg.initialGap}m → ${cfg.maxGap}m max gap, `
      + `${params.biome} biome.`,
    seed: params.seed,
    generatorType: 'stunt-track',
    generatedAt: new Date().toISOString(),
    generatorVersion: '0.1.0',
  };

  return {
    metadata,
    params,
    objects: allObjects,
    groups,
  };
}

// ─── Validation Helper ────────────────────────────────────────────────────────

export interface StuntTrackStats {
  objectCount: number;
  groupCount: number;
  trackLengthMetres: number;
  jumpSections: number;
  vehicles: number;
  ramps: number;
  platforms: number;
}

export function getStuntTrackStats(scene: SceneDescription): StuntTrackStats {
  return {
    objectCount: scene.objects.length,
    groupCount: scene.groups.length,
    trackLengthMetres: scene.params.length,
    jumpSections: scene.groups.filter((g) => g.tags.includes('section')).length,
    vehicles: scene.objects.filter((o) => o.type === 'vehicle').length,
    ramps: scene.objects.filter((o) => o.tags.includes('ramp')).length,
    platforms: scene.objects.filter((o) =>
      o.tags.includes('platform') || o.tags.includes('road'),
    ).length,
  };
}
