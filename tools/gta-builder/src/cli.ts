#!/usr/bin/env node
/**
 * gta-builder CLI
 *
 * Usage:
 *   gta-builder generate "Create a straight highway over the ocean with gaps"
 *   gta-builder generate --seed 12345 --type stunt-track "..."
 *   gta-builder validate output.xml
 *   gta-builder assets --category ramp
 *   gta-builder test
 */

import { writeFileSync, readFileSync, mkdirSync } from 'fs';
import { resolve, basename, extname } from 'path';

import { interpretPrompt } from './generators/prompt.js';
import { generateStuntTrack, getStuntTrackStats } from './generators/stunt-track.js';
import { exportMenyooXML, exportSceneJSON, getExportStats } from './export/menyoo.js';
import { validateScene, formatDiagnostics } from './validate/index.js';
import { PROPS } from './assets/props.js';
import { VEHICLES } from './assets/vehicles.js';
import type { GeneratorType } from './schema/scene.js';

// ─── Argument Parsing ─────────────────────────────────────────────────────────

interface CliArgs {
  command: string;
  prompt: string;
  seed?: number;
  type?: GeneratorType;
  output?: string;
  format: 'xml' | 'json' | 'both';
  category?: string;
  noValidate: boolean;
  quiet: boolean;
}

function parseArgs(argv: string[]): CliArgs {
  const args = argv.slice(2);
  const command = args[0] ?? 'help';
  let prompt = '';
  let seed: number | undefined;
  let type: GeneratorType | undefined;
  let output: string | undefined;
  let format: 'xml' | 'json' | 'both' = 'xml';
  let category: string | undefined;
  let noValidate = false;
  let quiet = false;

  for (let i = 1; i < args.length; i++) {
    switch (args[i]) {
      case '--seed':    seed = parseInt(args[++i], 10); break;
      case '--type':    type = args[++i] as GeneratorType; break;
      case '--output':
      case '-o':        output = args[++i]; break;
      case '--format':  format = args[++i] as 'xml' | 'json' | 'both'; break;
      case '--category': category = args[++i]; break;
      case '--no-validate': noValidate = true; break;
      case '--quiet':
      case '-q':        quiet = true; break;
      default:
        if (!args[i].startsWith('--')) {
          prompt = (prompt ? prompt + ' ' : '') + args[i];
        }
    }
  }

  return { command, prompt, seed, type, output, format, category, noValidate, quiet };
}

// ─── Commands ─────────────────────────────────────────────────────────────────

function cmdGenerate(cli: CliArgs): void {
  if (!cli.prompt) {
    console.error('Error: provide a prompt string after "generate"');
    process.exit(1);
  }

  if (!cli.quiet) {
    console.log(`\nPrompt: "${cli.prompt}"`);
    console.log('Interpreting prompt...');
  }

  const interpreted = interpretPrompt(cli.prompt, cli.seed);

  if (!cli.quiet) {
    console.log(`  → Type:       ${interpreted.generatorType}`);
    console.log(`  → Biome:      ${interpreted.params.biome}`);
    console.log(`  → Difficulty: ${interpreted.params.difficulty}`);
    console.log(`  → Layout:     ${interpreted.params.layout}`);
    console.log(`  → Length:     ${interpreted.params.length}m`);
    console.log(`  → Seed:       ${interpreted.params.seed}`);
    console.log('');
  }

  // Route to correct generator
  if (!cli.quiet) console.log('Generating scene...');

  const scene = generateStuntTrack(interpreted.params);
  if (cli.prompt) scene.metadata.prompt = cli.prompt;

  // Validate
  if (!cli.noValidate) {
    const validation = validateScene(scene);
    if (!cli.quiet || !validation.valid) {
      console.log('\nValidation:');
      console.log(formatDiagnostics(validation));
    }
    if (!validation.valid) {
      console.error('Scene has errors — fix before loading in GTA V.');
    }
  }

  // Stats
  if (!cli.quiet) {
    const stats = getStuntTrackStats(scene);
    console.log(`\nScene stats:`);
    console.log(`  Objects:  ${stats.objectCount}`);
    console.log(`  Groups:   ${stats.groupCount}`);
    console.log(`  Ramps:    ${stats.ramps}`);
    console.log(`  Vehicles: ${stats.vehicles}`);
    console.log(`  Sections: ${stats.jumpSections}`);
  }

  // Export
  const slugTitle = interpreted.title.toLowerCase().replace(/[^a-z0-9]+/g, '-').slice(0, 40);
  const baseName  = cli.output ? basename(cli.output, extname(cli.output)) : slugTitle;
  const outDir    = cli.output ? resolve(cli.output, '..') : resolve('output');

  mkdirSync(outDir, { recursive: true });

  if (cli.format === 'xml' || cli.format === 'both') {
    const xml  = exportMenyooXML(scene);
    const path = resolve(outDir, `${baseName}.xml`);
    writeFileSync(path, xml, 'utf8');
    if (!cli.quiet) {
      const xs = getExportStats(scene, xml);
      console.log(`\nExported: ${path}`);
      console.log(`  XML size: ${(xs.xmlBytes / 1024).toFixed(1)} KB`);
    }
  }

  if (cli.format === 'json' || cli.format === 'both') {
    const json = exportSceneJSON(scene);
    const path = resolve(outDir, `${baseName}.scene.json`);
    writeFileSync(path, json, 'utf8');
    if (!cli.quiet) console.log(`  JSON: ${path}`);
  }
}

function cmdAssets(cli: CliArgs): void {
  if (cli.category) {
    const filtered = PROPS.filter((p) => p.category === cli.category);
    if (filtered.length === 0) {
      console.log(`No props found in category "${cli.category}"`);
      return;
    }
    console.log(`\nProps in category "${cli.category}" (${filtered.length}):\n`);
    for (const p of filtered) {
      const dims = p.dimensions
        ? `  ${p.dimensions.width}w × ${p.dimensions.length}l × ${p.dimensions.height}h`
        : '';
      console.log(`  ${p.name.padEnd(40)} ${p.description}`);
      if (dims) console.log(`    dimensions:${dims}`);
    }
  } else {
    const categories = [...new Set(PROPS.map((p) => p.category))];
    console.log(`\nAsset database: ${PROPS.length} props, ${VEHICLES.length} vehicles\n`);
    console.log('Prop categories:');
    for (const cat of categories) {
      const count = PROPS.filter((p) => p.category === cat).length;
      console.log(`  ${cat.padEnd(20)} ${count} props`);
    }
    console.log('\nVehicle classes:');
    const vClasses = [...new Set(VEHICLES.map((v) => v.vehicleClass))];
    for (const cls of vClasses) {
      const count = VEHICLES.filter((v) => v.vehicleClass === cls).length;
      console.log(`  ${cls.padEnd(20)} ${count} vehicles`);
    }
  }
}

function cmdTest(): void {
  console.log('\nRunning built-in generation tests...\n');

  const testCases = [
    { prompt: 'Create a straight highway over the ocean with increasingly large gaps and ramps.',  label: 'Ocean highway (easy)' },
    { prompt: 'Generate an extreme stunt track with loops and pits.',                              label: 'Extreme with loops' },
    { prompt: 'Create a military checkpoint with barricades and vehicles.',                        label: 'Military base' },
    { prompt: 'Generate a short easy neon stunt course above the ocean with lights.',              label: 'Neon easy ocean' },
  ];

  let passed = 0;
  let failed = 0;

  for (const tc of testCases) {
    try {
      const interpreted = interpretPrompt(tc.prompt);
      const scene = generateStuntTrack(interpreted.params);
      const validation = validateScene(scene);
      const xml = exportMenyooXML(scene);

      const hasErrors = validation.errorCount > 0;
      const xmlValid  = xml.includes('<SpoonerPlacements>') && xml.includes('</SpoonerPlacements>');
      const hasObjects = scene.objects.length > 0;

      if (!hasErrors && xmlValid && hasObjects) {
        console.log(`  ✓ ${tc.label} — ${scene.objects.length} objects, ${validation.warningCount} warnings`);
        passed++;
      } else {
        console.log(`  ✗ ${tc.label} — FAILED: errors=${validation.errorCount}, xmlValid=${xmlValid}, objects=${scene.objects.length}`);
        if (hasErrors) {
          for (const d of validation.diagnostics.filter((d) => d.severity === 'error')) {
            console.log(`      ✗ ${d.message}`);
          }
        }
        failed++;
      }
    } catch (err) {
      console.log(`  ✗ ${tc.label} — EXCEPTION: ${err}`);
      failed++;
    }
  }

  console.log(`\nResults: ${passed} passed, ${failed} failed.\n`);
  if (failed > 0) process.exit(1);
}

function cmdHelp(): void {
  console.log(`
gta-builder — Procedural GTA V map generation pipeline

USAGE
  gta-builder generate "<prompt>" [options]
  gta-builder assets [--category <cat>]
  gta-builder test
  gta-builder help

COMMANDS
  generate    Generate a GTA V map from a natural language prompt
  assets      Browse the asset database
  test        Run built-in generation tests

GENERATE OPTIONS
  --seed <n>         Set random seed (deterministic output)
  --type <t>         Generator type (stunt-track, arena, etc.)
  --output <path>    Output file path (default: ./output/<title>.xml)
  --format <f>       Output format: xml | json | both (default: xml)
  --no-validate      Skip pre-export validation
  --quiet, -q        Suppress informational output

EXAMPLES
  gta-builder generate "Create a straight highway over the ocean with gaps"
  gta-builder generate --seed 42 "Extreme loop-de-loop course"
  gta-builder generate --format both "Military checkpoint with vehicles"
  gta-builder assets --category ramp
`);
}

// ─── Entry Point ──────────────────────────────────────────────────────────────

const cli = parseArgs(process.argv);

switch (cli.command) {
  case 'generate': cmdGenerate(cli); break;
  case 'assets':   cmdAssets(cli);   break;
  case 'test':     cmdTest();        break;
  case 'help':
  default:         cmdHelp();        break;
}
