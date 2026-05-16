/**
 * GTA V vehicle database — verified internal model names.
 * Used for vehicle lineup generation on tracks.
 */

import type { VehicleDefinition } from '../schema/assets.js';

export const VEHICLES: VehicleDefinition[] = [
  // ─── Super ────────────────────────────────────────────────────────────────
  { name: 'adder',      displayName: 'Adder',      vehicleClass: 'super',  dimensions: { width: 2.0, length: 4.3, height: 1.1 }, tags: ['super', 'fast'] },
  { name: 'zentorno',   displayName: 'Zentorno',   vehicleClass: 'super',  dimensions: { width: 2.0, length: 4.5, height: 1.2 }, tags: ['super', 'fast'] },
  { name: 't20',        displayName: 'T20',        vehicleClass: 'super',  dimensions: { width: 1.9, length: 4.4, height: 1.1 }, tags: ['super'] },
  { name: 'vagner',     displayName: 'Vagner',     vehicleClass: 'super',  dimensions: { width: 2.0, length: 4.5, height: 1.1 }, tags: ['super', 'modern'] },
  { name: 'emerus',     displayName: 'Emerus',     vehicleClass: 'super',  dimensions: { width: 2.0, length: 4.4, height: 1.1 }, tags: ['super', 'modern'] },
  { name: 'krieger',    displayName: 'Krieger',    vehicleClass: 'super',  dimensions: { width: 2.1, length: 4.5, height: 1.2 }, tags: ['super', 'modern'] },

  // ─── Sports ───────────────────────────────────────────────────────────────
  { name: 'sultan',     displayName: 'Sultan',     vehicleClass: 'sports', dimensions: { width: 1.8, length: 4.4, height: 1.3 }, tags: ['sports', 'awd'] },
  { name: 'banshee',    displayName: 'Banshee',    vehicleClass: 'sports', dimensions: { width: 1.9, length: 4.3, height: 1.2 }, tags: ['sports'] },
  { name: 'carbonizzare', displayName: 'Carbonizzare', vehicleClass: 'sports', dimensions: { width: 1.9, length: 4.4, height: 1.2 }, tags: ['sports', 'convertible'] },
  { name: 'elegy2',     displayName: 'Elegy RH8',  vehicleClass: 'sports', dimensions: { width: 1.8, length: 4.5, height: 1.3 }, tags: ['sports', 'awd'] },
  { name: 'jester',     displayName: 'Jester',     vehicleClass: 'sports', dimensions: { width: 1.9, length: 4.3, height: 1.2 }, tags: ['sports'] },
  { name: 'comet2',     displayName: 'Comet',      vehicleClass: 'sports', dimensions: { width: 1.8, length: 4.2, height: 1.2 }, tags: ['sports', 'convertible'] },

  // ─── Muscle ───────────────────────────────────────────────────────────────
  { name: 'dominator',  displayName: 'Dominator',  vehicleClass: 'muscle', dimensions: { width: 1.9, length: 4.8, height: 1.3 }, tags: ['muscle', 'american'] },
  { name: 'gauntlet',   displayName: 'Gauntlet',   vehicleClass: 'muscle', dimensions: { width: 1.9, length: 4.9, height: 1.4 }, tags: ['muscle', 'american'] },
  { name: 'tampa',      displayName: 'Tampa',      vehicleClass: 'muscle', dimensions: { width: 1.9, length: 4.8, height: 1.3 }, tags: ['muscle', 'american'] },
  { name: 'buccaneer',  displayName: 'Buccaneer',  vehicleClass: 'muscle', dimensions: { width: 2.0, length: 5.0, height: 1.3 }, tags: ['muscle', 'classic'] },

  // ─── SUV ──────────────────────────────────────────────────────────────────
  { name: 'baller',     displayName: 'Baller',     vehicleClass: 'suv',    dimensions: { width: 2.0, length: 4.9, height: 1.8 }, tags: ['suv', 'large'] },
  { name: 'cavalcade',  displayName: 'Cavalcade',  vehicleClass: 'suv',    dimensions: { width: 2.0, length: 5.0, height: 1.8 }, tags: ['suv', 'large'] },
  { name: 'granger',    displayName: 'Granger',    vehicleClass: 'suv',    dimensions: { width: 2.1, length: 5.2, height: 1.9 }, tags: ['suv', 'large', 'police'] },

  // ─── Off-Road ─────────────────────────────────────────────────────────────
  { name: 'brawler',    displayName: 'Brawler',    vehicleClass: 'offroad', dimensions: { width: 2.2, length: 4.5, height: 2.0 }, tags: ['offroad', 'buggy'] },
  { name: 'insurgent',  displayName: 'Insurgent',  vehicleClass: 'offroad', dimensions: { width: 2.3, length: 5.8, height: 2.2 }, tags: ['offroad', 'military', 'armored'] },
  { name: 'nightshark', displayName: 'Nightshark', vehicleClass: 'offroad', dimensions: { width: 2.2, length: 5.5, height: 2.0 }, tags: ['offroad', 'armored', 'military'] },

  // ─── Trucks ───────────────────────────────────────────────────────────────
  { name: 'phantom',    displayName: 'Phantom',    vehicleClass: 'truck',  dimensions: { width: 2.5, length: 8.5, height: 2.8 }, tags: ['truck', 'large', 'semi'] },
  { name: 'hauler',     displayName: 'Hauler',     vehicleClass: 'truck',  dimensions: { width: 2.5, length: 8.0, height: 2.8 }, tags: ['truck', 'large'] },
  { name: 'dump',       displayName: 'Dump Truck', vehicleClass: 'truck',  dimensions: { width: 4.0, length: 8.0, height: 4.5 }, tags: ['truck', 'construction', 'huge'] },
  { name: 'flatbed',    displayName: 'Flatbed',    vehicleClass: 'truck',  dimensions: { width: 2.5, length: 10.0, height: 2.5 }, tags: ['truck', 'industrial', 'platform'] },

  // ─── Military ─────────────────────────────────────────────────────────────
  { name: 'rhino',      displayName: 'Rhino Tank', vehicleClass: 'military', dimensions: { width: 3.5, length: 9.0, height: 2.8 }, tags: ['military', 'tank', 'huge'] },
  { name: 'barracks',   displayName: 'Barracks',   vehicleClass: 'military', dimensions: { width: 2.5, length: 6.5, height: 3.0 }, tags: ['military', 'truck'] },
  { name: 'apc',        displayName: 'APC',        vehicleClass: 'military', dimensions: { width: 2.8, length: 6.0, height: 2.5 }, tags: ['military', 'armored', 'apc'] },

  // ─── Motorcycles ──────────────────────────────────────────────────────────
  { name: 'bati',       displayName: 'Bati 801',   vehicleClass: 'motorcycle', dimensions: { width: 0.8, length: 2.1, height: 1.2 }, tags: ['motorcycle', 'fast', 'sport'] },
  { name: 'akuma',      displayName: 'Akuma',      vehicleClass: 'motorcycle', dimensions: { width: 0.8, length: 2.0, height: 1.1 }, tags: ['motorcycle', 'sport'] },
  { name: 'fcr',        displayName: 'FCR 1000',   vehicleClass: 'motorcycle', dimensions: { width: 0.9, length: 2.2, height: 1.1 }, tags: ['motorcycle', 'fast'] },
  { name: 'shotaro',    displayName: 'Shotaro',    vehicleClass: 'motorcycle', dimensions: { width: 0.9, length: 2.3, height: 1.2 }, tags: ['motorcycle', 'futuristic', 'neon'] },
];

// ─── Index ────────────────────────────────────────────────────────────────────

export const VEHICLES_BY_NAME = new Map<string, VehicleDefinition>(
  VEHICLES.map((v) => [v.name, v]),
);

export const VEHICLES_BY_CLASS = VEHICLES.reduce<Map<string, VehicleDefinition[]>>(
  (acc, v) => {
    const list = acc.get(v.vehicleClass) ?? [];
    list.push(v);
    acc.set(v.vehicleClass, list);
    return acc;
  },
  new Map(),
);

/** Sorted by vehicle size (length) ascending */
export const VEHICLES_BY_SIZE = [...VEHICLES].sort(
  (a, b) => (a.dimensions?.length ?? 0) - (b.dimensions?.length ?? 0),
);

export function getVehiclesByClass(cls: string): VehicleDefinition[] {
  return VEHICLES_BY_CLASS.get(cls) ?? [];
}

export function getVehiclesByTag(tag: string): VehicleDefinition[] {
  return VEHICLES.filter((v) => v.tags.includes(tag));
}

export function isValidVehicle(name: string): boolean {
  return VEHICLES_BY_NAME.has(name);
}
