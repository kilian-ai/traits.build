/**
 * Seeded pseudo-random number generator (mulberry32 algorithm).
 *
 * Using mulberry32 for speed and simplicity — produces uniform distribution
 * in [0, 1) and is fully deterministic given the same seed.
 *
 * Usage:
 *   const rng = createPRNG(12345);
 *   rng.float()         // 0.0 – 1.0
 *   rng.range(0, 10)    // integer [0, 10)
 *   rng.floatRange(a,b) // float [a, b)
 *   rng.pick(array)     // random element
 *   rng.shuffle(array)  // Fisher-Yates shuffle (in-place)
 */

export interface PRNG {
  /** Float in [0, 1) */
  float(): number;
  /** Integer in [min, max) */
  range(min: number, max: number): number;
  /** Float in [min, max) */
  floatRange(min: number, max: number): number;
  /** Pick a random element from a non-empty array */
  pick<T>(arr: T[]): T;
  /** Shuffle array in-place using Fisher-Yates; returns same array */
  shuffle<T>(arr: T[]): T[];
  /** Gaussian (normal) random using Box-Muller transform */
  gaussian(mean: number, stddev: number): number;
  /** Boolean with given probability (0–1) */
  chance(probability: number): boolean;
  /** Clone current state — allows branching without affecting main stream */
  clone(): PRNG;
  /** Current internal state (for reproducibility debugging) */
  state(): number;
}

export function createPRNG(seed: number): PRNG {
  // mulberry32 state — kept as a mutable object so clone() works correctly
  let s = seed >>> 0;

  function next(): number {
    s = (s + 0x6D2B79F5) >>> 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }

  let hasSpare = false;
  let spare = 0;

  return {
    float: next,

    range(min, max) {
      return Math.floor(next() * (max - min)) + min;
    },

    floatRange(min, max) {
      return next() * (max - min) + min;
    },

    pick<T>(arr: T[]): T {
      if (arr.length === 0) throw new Error('PRNG.pick: empty array');
      return arr[Math.floor(next() * arr.length)];
    },

    shuffle<T>(arr: T[]): T[] {
      for (let i = arr.length - 1; i > 0; i--) {
        const j = Math.floor(next() * (i + 1));
        [arr[i], arr[j]] = [arr[j], arr[i]];
      }
      return arr;
    },

    gaussian(mean, stddev) {
      // Box-Muller transform
      if (hasSpare) {
        hasSpare = false;
        return mean + stddev * spare;
      }
      let u: number, v: number, r: number;
      do {
        u = next() * 2 - 1;
        v = next() * 2 - 1;
        r = u * u + v * v;
      } while (r >= 1 || r === 0);
      const mul = Math.sqrt(-2.0 * Math.log(r) / r);
      spare = v * mul;
      hasSpare = true;
      return mean + stddev * u * mul;
    },

    chance(probability) {
      return next() < probability;
    },

    clone() {
      return createPRNG(s);
    },

    state() {
      return s;
    },
  };
}

/**
 * Derive a child seed from a parent seed and a string key.
 * Useful for partitioned generation (terrain seed, objects seed, etc.)
 */
export function deriveSeed(parentSeed: number, key: string): number {
  let h = parentSeed;
  for (let i = 0; i < key.length; i++) {
    h = (Math.imul(h, 31) + key.charCodeAt(i)) >>> 0;
  }
  return h;
}
