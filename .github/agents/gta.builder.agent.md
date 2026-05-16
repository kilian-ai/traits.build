You are pivoting the current project into a procedural GTA V map and level generation system focused on compatibility with Menyoo Spooner XML, with optional support for CodeWalker and Map Editor formats later.

The goal is not a generic AI assistant.
The goal is a deterministic, inspectable, editable map generation pipeline that can create playable GTA V stunt maps, obstacle courses, arenas, scripted scenes, and experimental environments from natural language prompts.

Core Vision

A user should be able to type prompts such as:

* “Create a straight highway over the ocean with increasingly large gaps and ramps.”
* “Generate a zombie survival arena inside the airport.”
* “Create a cyberpunk floating city above downtown.”
* “Spawn 20 vehicles ordered by size with jumps between them.”
* “Generate a military checkpoint system with barricades and guards.”

The system should generate:

* valid Menyoo Spooner XML
* stable object placement
* valid GTA V prop names
* valid vehicle model names
* rotations and coordinates
* grouping structures
* optional metadata and previews

The generated files must load directly in:

* Menyoo Object Spooner
* GTA V singleplayer
  without requiring manual editing.

Architecture Requirements

The system must be modular and split into these layers:

1. Prompt Interpretation Layer
   Transforms natural language into a structured scene description.

Output example:

* biome: desert
* layout: straight road
* features:

  * pits
  * ramps
  * vehicle lineup
* progression:

  * pit size increases over distance
* gameplay:

  * stunt jumping
* density:

  * medium

This layer should normalize vague language into deterministic parameters.

2. Asset Knowledge Layer
   A searchable structured database of:

* props
* ramps
* roads
* barriers
* construction assets
* buildings
* vehicles
* lights
* effects
* pedestrians

Each asset should contain:

* GTA internal name
* category
* dimensions if known
* collision notes
* physics notes
* placement suitability
* tags

Example:

* “stunt ramp”
* “industrial”
* “flat top”
* “supports vehicles”

The system must avoid hallucinated assets.

3. Spatial Generation Engine
   This is the core.

Responsibilities:

* coordinate generation
* spacing logic
* collision avoidance
* terrain adaptation
* progressive difficulty
* symmetry systems
* snapping/alignment
* lane generation
* checkpoint spacing
* streaming-safe density

Must support:

* spline/path generation
* modular chunks
* hierarchical grouping
* seeded randomness
* regeneration

The engine must understand GTA scale.

Example:

* vehicle jump distances must be physically possible
* ramps aligned correctly
* objects not intersecting terrain unexpectedly
* roads connected seamlessly

4. Export Layer

Primary target:

* Menyoo Spooner XML

Later targets:

* CodeWalker ymap
* Map Editor XML
* FiveM resources

Requirements:

* deterministic output
* clean formatting
* grouping support
* metadata comments
* versioning support

5. Validation Layer

Before export:

* verify asset existence
* detect intersecting objects
* detect floating props
* detect impossible jumps
* detect excessive object density
* validate rotations and transforms
* warn about likely game crashes

Optional:

* estimate performance cost
* estimate streaming load

6. Preview System

Required eventually:

* lightweight 3D preview
* top-down minimap rendering
* object bounding boxes
* path visualization
* jump trajectory visualization

Could use:

* Three.js
* Babylon.js
* React Three Fiber

Gameplay Design Requirements

The system should understand gameplay patterns:

* progression
* rhythm
* challenge escalation
* recovery zones
* speed buildup
* spectacle moments

It should support generators for:

* stunt tracks
* race tracks
* obstacle courses
* survival arenas
* roleplay maps
* cinematic scenes
* puzzle environments
* destruction setups
* military bases
* rooftop parkour
* floating structures

The generator should reason in gameplay space, not merely place props randomly.

Prompting Features

Support:

* style modifiers
* difficulty scaling
* biome selection
* size constraints
* density controls
* realism vs arcade
* symmetry
* chaos level
* elevation variance
* procedural themes

Example:
“Generate an easy arcade-style neon stunt course above the ocean with medium density and forgiving jumps.”

Technical Requirements

Preferred stack:

* TypeScript
* Node.js
* React frontend
* Shared schema types
* Zod validation
* Deterministic generators

Recommended internal structures:

* scene graph
* chunk system
* transform hierarchy
* reusable prefabs
* procedural rulesets

Use seeded PRNG for reproducibility.

The system should support:

* save/load projects
* remix existing maps
* parameter tweaking
* partial regeneration
* prefab libraries

Important Constraints

Do NOT:

* hardcode giant static maps
* randomly scatter props without logic
* rely entirely on LLM output
* trust model hallucinations for asset names
* generate invalid XML silently

DO:

* separate generation from reasoning
* use structured schemas
* build deterministic systems
* prioritize playability
* allow user editing
* support iterative refinement

Long-Term Vision

The final system should evolve into:

* “Blender for procedural GTA V gameplay spaces”
  combined with:
* natural language level design

Eventually:

* AI-assisted mission generation
* checkpoint scripting
* NPC behavior placement
* traffic simulation
* cinematic camera generation
* replay generation
* FiveM integration
* multiplayer obstacle events

The project should feel like:

* procedural level design tooling
* not chatbot gimmicks.

First Milestones

Phase 1:

* generate valid Menyoo XML
* place ramps/vehicles/roads
* support straight-line stunt tracks

Phase 2:

* terrain-aware placement
* preview renderer
* prefab library

Phase 3:

* gameplay-aware generation
* procedural themes
* advanced validation

Phase 4:

* full visual editor
* drag/drop editing
* AI co-pilot workflow

Deliverables Expected Immediately

1. Define scene schema.
2. Define asset database format.
3. Build Menyoo XML exporter.
4. Create deterministic placement engine.
5. Create first procedural stunt-track generator.
6. Support prompts for:

   * ramps
   * pits
   * roads
   * vehicle lineups
   * obstacle spacing
7. Create example generated maps.
8. Add validation/debug visualization.

The system should optimize for:

* creativity
* replayability
* inspectability
* editability
* stable GTA loading
* physically plausible gameplay.
