/**
 * Asset type definitions for the GTA V prop and vehicle database.
 * All names are verified GTA V internal identifiers.
 */

export type AssetCategory =
  | 'ramp'
  | 'platform'
  | 'road-section'
  | 'loop'
  | 'tube'
  | 'barrier'
  | 'cone'
  | 'light'
  | 'container'
  | 'construction'
  | 'building'
  | 'nature'
  | 'vehicle'
  | 'decoration'
  | 'checkpoint';

export type PhysicsType = 'static' | 'dynamic' | 'kinematic' | 'ragdoll';

export interface PropDimensions {
  /** Width in meters (X axis when unrotated) */
  width: number;
  /** Length in meters (Y axis when unrotated) */
  length: number;
  /** Height in meters (Z axis) */
  height: number;
}

export interface PropDefinition {
  /** GTA V internal name (e.g. prop_ramp_adj_flip_mb) */
  name: string;
  category: AssetCategory;
  /** Human-readable description */
  description: string;
  /** Approximate dimensions */
  dimensions?: PropDimensions;
  /** Default Z offset from ground (some props need to sit partially underground) */
  groundOffset: number;
  /** Whether the prop has drive-over collision */
  driveable: boolean;
  /** Whether vehicles can jump off this */
  jumpable: boolean;
  /** Physics behavior */
  physics: PhysicsType;
  /** Free-text notes (collision quirks, flip bugs, etc.) */
  notes?: string;
  /** Semantic tags for search */
  tags: string[];
  /** Texture variation count (0 = only one variation) */
  textureVariations: number;
}

export interface VehicleDefinition {
  /** GTA V internal model name */
  name: string;
  /** Human display name */
  displayName: string;
  /** Vehicle class */
  vehicleClass: VehicleClass;
  /** Approximate dimensions */
  dimensions?: PropDimensions;
  /** Notes */
  notes?: string;
  tags: string[];
}

export type VehicleClass =
  | 'super'
  | 'sports'
  | 'muscle'
  | 'suv'
  | 'offroad'
  | 'van'
  | 'truck'
  | 'emergency'
  | 'military'
  | 'motorcycle'
  | 'boat'
  | 'helicopter'
  | 'plane'
  | 'industrial';

// ─── Asset Query API ──────────────────────────────────────────────────────────

export interface AssetQuery {
  category?: AssetCategory | AssetCategory[];
  tags?: string[];
  driveable?: boolean;
  jumpable?: boolean;
  minWidth?: number;
  maxWidth?: number;
}
