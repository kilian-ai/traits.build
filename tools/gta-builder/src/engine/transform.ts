/**
 * Transform math utilities for GTA V world-space operations.
 *
 * GTA V coordinate system:
 *   - Right-handed, Y-forward
 *   - X = East (+) / West (-)
 *   - Y = North (+) / South (-)
 *   - Z = Up (+) / Down (-)
 *   - 1 unit ≈ 1 metre
 *   - Headings: 0° = North, 90° = East, 180° = South, 270° = West
 *
 * Rotations in Menyoo XML are stored as quaternions (X, Y, Z, W).
 */

import type { Vec3, Quat } from '../schema/scene.js';

// ─── Constants ────────────────────────────────────────────────────────────────

export const DEG2RAD = Math.PI / 180;
export const RAD2DEG = 180 / Math.PI;

// ─── Vec3 Operations ──────────────────────────────────────────────────────────

export function v3add(a: Vec3, b: Vec3): Vec3 {
  return { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z };
}

export function v3sub(a: Vec3, b: Vec3): Vec3 {
  return { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z };
}

export function v3scale(v: Vec3, s: number): Vec3 {
  return { x: v.x * s, y: v.y * s, z: v.z * s };
}

export function v3length(v: Vec3): number {
  return Math.sqrt(v.x * v.x + v.y * v.y + v.z * v.z);
}

export function v3normalize(v: Vec3): Vec3 {
  const len = v3length(v);
  if (len < 1e-9) return { x: 0, y: 1, z: 0 };
  return v3scale(v, 1 / len);
}

export function v3dot(a: Vec3, b: Vec3): number {
  return a.x * b.x + a.y * b.y + a.z * b.z;
}

export function v3cross(a: Vec3, b: Vec3): Vec3 {
  return {
    x: a.y * b.z - a.z * b.y,
    y: a.z * b.x - a.x * b.z,
    z: a.x * b.y - a.y * b.x,
  };
}

export function v3lerp(a: Vec3, b: Vec3, t: number): Vec3 {
  return {
    x: a.x + (b.x - a.x) * t,
    y: a.y + (b.y - a.y) * t,
    z: a.z + (b.z - a.z) * t,
  };
}

export function v3distance(a: Vec3, b: Vec3): number {
  return v3length(v3sub(b, a));
}

// ─── Heading / Direction ──────────────────────────────────────────────────────

/**
 * Convert a heading (degrees, GTA V convention: 0=North, 90=East) to
 * a normalised 2D forward vector (x=east, y=north).
 */
export function headingToForward(headingDeg: number): Vec3 {
  const rad = headingDeg * DEG2RAD;
  return { x: Math.sin(rad), y: Math.cos(rad), z: 0 };
}

/**
 * Convert a heading to a right-perpendicular vector (90° clockwise).
 */
export function headingToRight(headingDeg: number): Vec3 {
  return headingToForward(headingDeg + 90);
}

/**
 * Move a point along a heading by a given distance.
 */
export function moveAlongHeading(origin: Vec3, headingDeg: number, distance: number): Vec3 {
  const fwd = headingToForward(headingDeg);
  return v3add(origin, v3scale(fwd, distance));
}

// ─── Quaternion Operations ────────────────────────────────────────────────────

export const QUAT_IDENTITY: Quat = { x: 0, y: 0, z: 0, w: 1 };

/**
 * Build a quaternion from an axis (normalised) and angle (radians).
 */
export function quatFromAxisAngle(axis: Vec3, angleRad: number): Quat {
  const half = angleRad / 2;
  const s = Math.sin(half);
  return {
    x: axis.x * s,
    y: axis.y * s,
    z: axis.z * s,
    w: Math.cos(half),
  };
}

/**
 * Build a quaternion from Euler angles (degrees) in ZYX order (yaw-pitch-roll).
 * This matches GTA V's rotation convention used in Menyoo.
 *
 * @param yawDeg   Rotation around Z (heading change)
 * @param pitchDeg Rotation around Y
 * @param rollDeg  Rotation around X
 */
export function quatFromEuler(yawDeg: number, pitchDeg: number, rollDeg: number): Quat {
  const cy = Math.cos(yawDeg   * DEG2RAD / 2);
  const sy = Math.sin(yawDeg   * DEG2RAD / 2);
  const cp = Math.cos(pitchDeg * DEG2RAD / 2);
  const sp = Math.sin(pitchDeg * DEG2RAD / 2);
  const cr = Math.cos(rollDeg  * DEG2RAD / 2);
  const sr = Math.sin(rollDeg  * DEG2RAD / 2);

  return {
    x: sr * cp * cy - cr * sp * sy,
    y: cr * sp * cy + sr * cp * sy,
    z: cr * cp * sy - sr * sp * cy,
    w: cr * cp * cy + sr * sp * sy,
  };
}

/**
 * Quaternion for a heading rotation (yaw only, around Z axis).
 * headingDeg: 0=North, 90=East (GTA V convention).
 */
export function quatFromHeading(headingDeg: number): Quat {
  return quatFromEuler(headingDeg, 0, 0);
}

/**
 * Multiply two quaternions (q1 * q2 = combined rotation).
 */
export function quatMul(a: Quat, b: Quat): Quat {
  return {
    x:  a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
    y:  a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
    z:  a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
    w:  a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
  };
}

/**
 * Normalise a quaternion.
 */
export function quatNormalize(q: Quat): Quat {
  const len = Math.sqrt(q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w);
  if (len < 1e-9) return QUAT_IDENTITY;
  return { x: q.x / len, y: q.y / len, z: q.z / len, w: q.w / len };
}

// ─── Jenkins One-At-A-Time Hash ───────────────────────────────────────────────

/**
 * GTA V model hash (Jenkins one-at-a-time).
 * Returns a signed 32-bit integer as used in Menyoo XML <Value> fields.
 */
export function joaat(str: string): number {
  let hash = 0;
  const s = str.toLowerCase();
  for (let i = 0; i < s.length; i++) {
    hash = (hash + s.charCodeAt(i)) >>> 0;
    hash = (hash + (hash << 10)) >>> 0;
    hash = (hash ^ (hash >>> 6)) >>> 0;
  }
  hash = (hash + (hash << 3)) >>> 0;
  hash = (hash ^ (hash >>> 11)) >>> 0;
  hash = (hash + (hash << 15)) >>> 0;
  // Convert to signed 32-bit int (Menyoo uses signed)
  return hash | 0;
}

// ─── Trajectory Math (for jump validation) ───────────────────────────────────

export interface JumpTrajectory {
  /** Distance covered horizontally (metres) */
  horizontalDistance: number;
  /** Peak height above ramp exit (metres) */
  peakHeight: number;
  /** Total airtime (seconds) */
  airtime: number;
}

/**
 * Estimate jump trajectory given launch speed and ramp angle.
 * Uses GTA V gravity (~9.8 m/s²) and ignores aerodynamic drag.
 *
 * @param speedKph   Vehicle speed at ramp exit (km/h)
 * @param rampAngle  Ramp angle in degrees (positive = upward)
 */
export function estimateJumpTrajectory(speedKph: number, rampAngle: number): JumpTrajectory {
  const GRAVITY = 9.8; // m/s²
  const speedMs = speedKph / 3.6;
  const angleRad = rampAngle * DEG2RAD;
  const vx = speedMs * Math.cos(angleRad);
  const vz = speedMs * Math.sin(angleRad);
  // Time to apex: vz / g
  const timeToApex = vz / GRAVITY;
  const airtime = timeToApex * 2;
  const horizontalDistance = vx * airtime;
  const peakHeight = vz * timeToApex - 0.5 * GRAVITY * timeToApex * timeToApex;
  return { horizontalDistance, peakHeight, airtime };
}

/**
 * Recommend a minimum approach speed (km/h) for clearing a given gap.
 *
 * @param gapMetres  Distance to clear
 * @param rampAngle  Ramp angle in degrees
 * @returns Minimum speed in km/h
 */
export function minSpeedForGap(gapMetres: number, rampAngle: number): number {
  const GRAVITY = 9.8;
  const angleRad = rampAngle * DEG2RAD;
  // Derive: gap = v_x * t_air = v*cos(a) * (2*v*sin(a)/g)
  //   gap = v² * sin(2a) / g
  //   v = sqrt(gap * g / sin(2a))
  const sin2a = Math.sin(2 * angleRad);
  if (Math.abs(sin2a) < 1e-6) return Infinity;
  const speedMs = Math.sqrt((gapMetres * GRAVITY) / sin2a);
  return speedMs * 3.6;
}
