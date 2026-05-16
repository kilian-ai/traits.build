/**
 * Scene validation layer.
 *
 * Checks a SceneDescription for issues BEFORE export.
 * Returns a list of diagnostics — callers decide what to do with warnings vs errors.
 */

import type { SceneDescription, SceneObject } from '../schema/scene.js';
import { isValidProp } from '../assets/props.js';
import { isValidVehicle } from '../assets/vehicles.js';
import { v3distance, minSpeedForGap, estimateJumpTrajectory } from '../engine/transform.js';

// ─── Diagnostic Types ─────────────────────────────────────────────────────────

export type DiagnosticSeverity = 'error' | 'warning' | 'info';

export interface Diagnostic {
  severity: DiagnosticSeverity;
  code: string;
  message: string;
  objectId?: string;
}

// ─── Individual Checks ────────────────────────────────────────────────────────

/** Ensure all prop model names are in the known-safe asset database. */
function checkUnknownAssets(scene: SceneDescription): Diagnostic[] {
  const diags: Diagnostic[] = [];
  for (const obj of scene.objects) {
    if (obj.type === 'prop' && !isValidProp(obj.model)) {
      diags.push({
        severity: 'error',
        code: 'UNKNOWN_PROP',
        message: `Unknown prop model "${obj.model}" — not in asset database. May crash game.`,
        objectId: obj.id,
      });
    }
    if (obj.type === 'vehicle' && !isValidVehicle(obj.model)) {
      diags.push({
        severity: 'warning',
        code: 'UNKNOWN_VEHICLE',
        message: `Vehicle "${obj.model}" not in verified database — verify before loading.`,
        objectId: obj.id,
      });
    }
  }
  return diags;
}

/** Check for objects placed suspiciously far below sea level. */
function checkSunkenObjects(scene: SceneDescription): Diagnostic[] {
  const SEA_LEVEL = 0;
  const WARNING_DEPTH = -5;
  const diags: Diagnostic[] = [];
  for (const obj of scene.objects) {
    if (obj.position.z < WARNING_DEPTH) {
      diags.push({
        severity: 'warning',
        code: 'SUNKEN_OBJECT',
        message: `Object "${obj.model}" at Z=${obj.position.z.toFixed(1)} — below sea level. May be invisible or cause issues.`,
        objectId: obj.id,
      });
    }
  }
  return diags;
}

/** Warn about excessive object count — can cause streaming issues. */
function checkObjectDensity(scene: SceneDescription): Diagnostic[] {
  const WARN_COUNT  = 300;
  const ERROR_COUNT = 750;
  const diags: Diagnostic[] = [];
  if (scene.objects.length > ERROR_COUNT) {
    diags.push({
      severity: 'error',
      code: 'TOO_MANY_OBJECTS',
      message: `Scene has ${scene.objects.length} objects (limit ~${ERROR_COUNT}). GTA V may crash or freeze.`,
    });
  } else if (scene.objects.length > WARN_COUNT) {
    diags.push({
      severity: 'warning',
      code: 'HIGH_OBJECT_COUNT',
      message: `Scene has ${scene.objects.length} objects — may impact game performance.`,
    });
  }
  return diags;
}

/** Detect objects at identical or very close positions (likely duplicates). */
function checkDuplicatePositions(scene: SceneDescription): Diagnostic[] {
  const DUPLICATE_THRESHOLD = 0.1; // metres
  const diags: Diagnostic[] = [];
  for (let i = 0; i < scene.objects.length; i++) {
    for (let j = i + 1; j < scene.objects.length; j++) {
      const a = scene.objects[i];
      const b = scene.objects[j];
      if (
        a.model === b.model &&
        v3distance(a.position, b.position) < DUPLICATE_THRESHOLD
      ) {
        diags.push({
          severity: 'warning',
          code: 'DUPLICATE_POSITION',
          message: `Objects "${a.id}" and "${b.id}" share near-identical position — possible duplicate.`,
          objectId: a.id,
        });
      }
    }
    // Only check first 200 objects to keep O(n²) manageable
    if (i > 200) break;
  }
  return diags;
}

/** Validate quaternion rotations are normalised (|q| ≈ 1). */
function checkRotations(scene: SceneDescription): Diagnostic[] {
  const diags: Diagnostic[] = [];
  for (const obj of scene.objects) {
    const q = obj.rotation;
    const len = Math.sqrt(q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w);
    if (Math.abs(len - 1) > 0.01) {
      diags.push({
        severity: 'error',
        code: 'BAD_ROTATION',
        message: `Object "${obj.model}" has non-unit quaternion (|q|=${len.toFixed(4)}). Normalise before export.`,
        objectId: obj.id,
      });
    }
  }
  return diags;
}

/** Check that ramp objects exist (no sections with just gaps). */
function checkRampPresence(scene: SceneDescription): Diagnostic[] {
  const diags: Diagnostic[] = [];
  const hasRamps = scene.objects.some((o) => o.tags.includes('ramp'));
  const hasSection = scene.groups.some((g) => g.tags.includes('section'));
  if (hasSection && !hasRamps) {
    diags.push({
      severity: 'warning',
      code: 'NO_RAMPS',
      message: 'Scene has jump sections but no ramp objects — vehicle may not be able to jump.',
    });
  }
  return diags;
}

/** Estimate whether jump sections have physically-possible gaps. */
function checkJumpPhysics(scene: SceneDescription): Diagnostic[] {
  const diags: Diagnostic[] = [];
  const jumpGroups = scene.groups.filter((g) => g.tags.includes('section'));

  for (const group of jumpGroups) {
    const gapTag = group.tags.find((t) => t.startsWith('gap-'));
    if (!gapTag) continue;
    const gapM = parseFloat(gapTag.replace('gap-', ''));
    if (isNaN(gapM)) continue;

    // Assume 30° ramp angle; check minimum speed needed
    const minSpeed = minSpeedForGap(gapM, 30);
    if (minSpeed > 180) {
      diags.push({
        severity: 'error',
        code: 'IMPOSSIBLE_JUMP',
        message: `Jump section "${group.name}" requires ${minSpeed.toFixed(0)} km/h — physically impossible in GTA V. Reduce gap (${gapM}m).`,
      });
    } else if (minSpeed > 140) {
      diags.push({
        severity: 'warning',
        code: 'DIFFICULT_JUMP',
        message: `Jump section "${group.name}" requires ${minSpeed.toFixed(0)} km/h at 30° ramp — very difficult. Gap: ${gapM}m.`,
      });
    }
  }
  return diags;
}

// ─── Main Validation Function ─────────────────────────────────────────────────

export interface ValidationResult {
  valid: boolean;          // false if any 'error' severity diagnostics
  diagnostics: Diagnostic[];
  errorCount: number;
  warningCount: number;
  infoCount: number;
}

export function validateScene(scene: SceneDescription): ValidationResult {
  const diagnostics: Diagnostic[] = [
    ...checkUnknownAssets(scene),
    ...checkSunkenObjects(scene),
    ...checkObjectDensity(scene),
    ...checkDuplicatePositions(scene),
    ...checkRotations(scene),
    ...checkRampPresence(scene),
    ...checkJumpPhysics(scene),
  ];

  const errorCount   = diagnostics.filter((d) => d.severity === 'error').length;
  const warningCount = diagnostics.filter((d) => d.severity === 'warning').length;
  const infoCount    = diagnostics.filter((d) => d.severity === 'info').length;

  return {
    valid: errorCount === 0,
    diagnostics,
    errorCount,
    warningCount,
    infoCount,
  };
}

export function formatDiagnostics(result: ValidationResult): string {
  if (result.diagnostics.length === 0) {
    return '✓ Scene validated — no issues found.';
  }
  const lines: string[] = [];
  for (const d of result.diagnostics) {
    const icon = d.severity === 'error' ? '✗' : d.severity === 'warning' ? '⚠' : 'ℹ';
    const obj  = d.objectId ? ` [${d.objectId}]` : '';
    lines.push(`${icon} [${d.code}]${obj} ${d.message}`);
  }
  lines.push('');
  lines.push(
    `Summary: ${result.errorCount} error(s), ${result.warningCount} warning(s), ${result.infoCount} info(s).`,
  );
  return lines.join('\n');
}
