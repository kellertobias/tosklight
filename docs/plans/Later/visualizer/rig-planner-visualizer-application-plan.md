# Rig Planner and Lighting Visualizer — Application Plan

Status: Proposed architecture and phased delivery plan  
Repository: `/Users/keller/repos/light`  
Target platforms: Windows, macOS, Linux  
Primary UI: Tauri, React, TypeScript  
Planning viewport: WebGL2, managed by the React application  
High-quality visualizer: Godot 4 Forward+, running as a separate process  
Canonical persistence: SQLite-based show document  
Primary interchange: MVR, GDTF, GLB/glTF  

## 1. Product definition

The application is a cross-platform production-planning and lighting-previsualization system. It uses the existing ToskLight fixture library as its fixture and scenery source of truth. It has three product surfaces over one authoritative show model, delivered in a deliberate priority order:

1. **Fixture placement, patching and spatial planning**
   - Browse the shared library as Manufacturer → Fixture → Mode.
   - Either add an unpatched fixture or add and patch it in one operation.
   - Build or import a venue.
   - Place trusses, stages, fixtures, PA, motors, chains, hanging points, distribution and other equipment.
   - Record exact attachment and mounting relationships.

2. **Lighting previsualization**
   - Render the same planned scene with realistic fixtures, materials, shadows, haze and beams.
   - Resolve the shared fixture definition, selected mode, 3D geometry and DMX behavior.
   - Support gobos, zoom, focus, shutters, prisms, color systems, gels and fixture movement.
   - Later support media surfaces, CITP/MSEX previews, lasers, fire and persistence-of-vision effects.

3. **Optional production logistics and paperwork**
   - Plan cable routes and choose cables from available inventory.
   - Model connectors, networking gear, distribution, dimmer racks and composite racks.
   - Calculate preliminary weights, support reactions and utilization.
   - Produce plans, schedules, bills of materials and interchange files.

The planning model is authoritative. Neither the WebGL viewport nor Godot owns show data; both consume projections of the canonical model.

## 2. Product principles

1. **One semantic scene, multiple representations.** A fixture, truss or motor has one identity and transform, but may have separate plan symbols, lightweight editor meshes, realistic render geometry, snapping geometry and physical metadata.
2. **Attachments are explicit.** A fixture is not merely positioned near a truss. It is mounted to a particular truss member at a defined local position and orientation with specified hardware.
3. **Planning calculations are explainable.** Every reported cable length or load must show its inputs, assumptions and calculation path.
4. **Safety boundaries are visible.** Preliminary structural estimates must not be presented as certified engineering.
5. **Interchange is non-destructive.** Unsupported MVR/GDTF data is retained for round-trip export where possible.
6. **The renderer is replaceable.** Godot is selected for the initial and likely production renderer, but communication uses an engine-neutral protocol.
7. **Cross-platform behavior is tested as a product requirement.** Windows, macOS and Linux are first-class platforms from the first technical slice.
8. **The UI is deliberately designed.** React and CSS own the application’s visual language; default browser controls are not the product design.
9. **Open development and dependency compatibility are designed early.** The repository license, asset licenses, fixture-data licenses and third-party engine licenses are tracked separately.
10. **The ToskLight fixture library is shared, not copied.** Manufacturer, fixture, mode, type, channel behavior, model assets and imported GDTF data must have one canonical definition across both applications.
11. **Placement and patching are independent.** A fixture may exist spatially without a DMX address, universe or power assignment and may be patched later without being recreated.
12. **Logistics are opt-in.** A project can stop at placement, patch and visualization. Cable, stock, rack, power and load workflows add information without becoming prerequisites for the core editor.
13. **This is one ToskLight workspace.** The planner, visualizer integration and existing light-control applications live in `/Users/keller/repos/light` and share Rust crates, fixture definitions, protocols and frontend packages instead of maintaining parallel implementations.
14. **The existing UI library defines the visual language.** Reusable controls come from `packages/ui`; the planner adds product-specific composition and adapters, not look-alike copies of common components.
15. **Code structure is a product requirement.** Modules, functions, methods, components, classes and traits remain small, focused and clearly owned. Names are concise and descriptive, dependencies point in one direction, and hierarchies remain shallow enough to understand without tracing many layers.

## 3. Scope boundaries

### 3.1 Version 1 scope

- Parametric rectangular room and basic stage creation.
- Venue import from GLB/glTF, OBJ and 3MF.
- PDF or image plan underlays with scale calibration.
- Shared ToskLight Manufacturer → Fixture → Mode browser.
- Unpatched fixture placement and add-and-patch placement.
- Built-in and user-authored fixtures, scenery and model variants.
- Straight truss systems assembled from inventory-defined parts.
- Hanging points, motors, chains and direct suspensions.
- Fixture, PA and generic equipment placement.
- Explicit top, bottom and side mounting to named truss members.
- Per-instance mounting orientation, hardware pan/tilt inversion and software compensation.
- Per-instance gels and arbitrary custom filter colors for conventional fixtures.
- Equipment weights, center-of-gravity data and attachment points.
- Cable connection chains, route editing, length calculation and stock selection.
- Built-in and custom connector types.
- Networking, DMX, conversion, distribution and composite rack equipment.
- Preliminary static load and support-reaction calculations for supported configurations.
- MVR and GDTF import/export.
- Synchronized quad planning view: top, side, front/back and isometric.
- Separate synchronized Godot 3D visualizer.
- Wireframe, hidden-line and flat-shaded WebGL editor rendering.
- Godot visualizer with PBR materials, fixture movement, dimmer, color, zoom, focus, gobos, shadows, haze, bloom and basic temporal effects.
- PDF plans and CSV schedules.
- Windows, macOS and Linux installers/packages.

### 3.2 Deferred scope

- Full DWG editing.
- General-purpose solid modeling, NURBS or parametric architectural CAD.
- Certified structural verification.
- Complex finite-element analysis.
- Automated bridle design.
- Wind-load engineering.
- Collaboration server or cloud document storage.
- Mobile and tablet applications.
- Photorealistic path tracing.
- Full media-server replacement.
- Pyrotechnic control.

### 3.3 Later product increments

- Curved and polygonal truss systems.
- Ground-supported roofs and towers.
- Advanced truss load-table checking.
- Bridles and multi-leg suspension geometry.
- Power distribution, phase balancing and voltage-drop validation.
- Network topology and bandwidth validation.
- CITP/MSEX, NDI, Spout, Syphon and capture inputs.
- LED walls and projection mapping.
- Simple laser visualization.
- Fire, smoke and particle effects.
- Renderer recording and offline high-quality output.
- IFC and selected CAD interchange.
- Plugin API and manufacturer data packages.

## 4. High-level architecture

```text
Tauri desktop application
├── React application shell
├── synchronized WebGL2 planning viewports
├── shared ToskLight fixture browser and patch workflow
├── Editor tools and worksheets
├── Rust application core
│   ├── canonical scene model
│   ├── commands and undo
│   ├── SQLite document
│   ├── shared fixture/scenery library adapter
│   ├── connector and equipment library
│   ├── MVR/GDTF interchange
│   ├── cable solver
│   ├── preliminary rigging solver
│   └── renderer supervisor/protocol
└── Godot renderer process
    ├── native renderer window
    ├── mirrored render scene
    ├── GDTF runtime behavior
    ├── lighting and volumetrics
    ├── media textures and effects
    └── picking, screenshots and metrics
```

### 4.1 Process ownership

**Tauri process**

- Owns the current document and all persistence.
- Owns undo/redo and command validation.
- Owns file import/export.
- Owns equipment libraries and inventory.
- Owns cable and load calculations.
- Launches, monitors and restarts the renderer.

**Godot process**

- Owns its native window and render loop.
- Owns GPU resources, render meshes, materials and textures.
- Mirrors only the render-relevant portion of the show.
- Interpolates live values locally.
- Handles renderer-window camera movement and object picking.
- Never writes the SQLite show file.

### 4.2 Failure behavior

- If Godot crashes, the document remains open and editable.
- The UI reports that visualization stopped and offers restart.
- Restart sends a fresh full snapshot followed by current live values.
- Renderer version or protocol incompatibility fails with a diagnostic rather than a blank window.
- Imported assets that fail in Godot are replaced with bounds or placeholders and reported in an asset-errors panel.

## 5. Proposed repository structure

This is a firm workspace decision: the application is part of `/Users/keller/repos/light`. Before implementation, inspect the live workspace and fit the exact crate and package names to what already exists, but do not create a separate repository or a parallel foundation.

```text
/Users/keller/repos/light/
├── apps/
│   ├── rig-planner/              Tauri + React planner application
│   ├── visualizer-godot/         Godot renderer project and packaging
│   └── ...                       existing ToskLight applications
├── crates/
│   ├── scene-model/              canonical semantic scene and units
│   ├── scene-commands/           validated mutations and undo records
│   ├── show-document/            SQLite persistence and migrations
│   ├── asset-library/            models, symbols, truss and equipment assets
│   ├── fixture/                  shared ToskLight fixture definitions and runtime mapping
│   ├── connector-library/        built-in and user-defined connector types
│   ├── equipment-topology/       ports, modules, racks and exposed capabilities
│   ├── mvr/                      MVR import, retained data and export
│   ├── cable-planning/           route graph and stock optimizer
│   ├── rigging-planning/         preliminary loads and support reactions
│   ├── renderer-protocol/        wire messages, snapshots and compatibility
│   ├── renderer-supervisor/      process lifecycle and recovery
│   └── reporting/                PDF/CSV schedules and calculation reports
├── packages/
│   ├── ui/                       shared reusable React component library
│   ├── editor-core/              TypeScript tools, selection and viewport commands
│   ├── editor-renderer/          WebGL2 retained-mode drawing engine
│   └── scene-types/              generated TypeScript scene/protocol types
├── schemas/
│   ├── show/                     SQL migrations and schema documentation
│   ├── protocol/                 renderer protocol schema
│   └── equipment/                asset and manufacturer-data schemas
└── fixtures/
    └── benchmark-scenes/         canonical test and performance scenes
```

The names of proposed new crates may be consolidated into existing crates after inspection. The ownership boundary matters more than the exact directory count: reusable domain behavior belongs in workspace crates, reusable presentation belongs in `packages/ui`, application workflows belong in `apps/rig-planner`, and Godot-specific rendering belongs in `apps/visualizer-godot`.

Reuse the existing fixture, MVR, source-GDTF retention, resolved-output and CITP seams after live inspection. Do not create a second fixture-definition system or copy fixture records into an independently evolving planner schema.

### 5.1 Code organization and dependency rules

The codebase must remain approachable as the product grows:

- Use concise, descriptive names. Short names are preferred when their meaning is obvious, but abbreviations or cryptic names must not replace clarity.
- Keep functions and methods short and single-purpose. Separate orchestration, domain calculation, persistence, protocol conversion and rendering rather than combining them in one method.
- Keep React components and hooks focused. Large screens compose smaller views; they do not become stateful “god components.”
- Keep class and trait hierarchies shallow and explicit. Prefer composition and small interfaces over deep inheritance. Introduce a Rust trait or TypeScript abstraction only at a real substitution, test or integration boundary.
- Give each module one clear concept and a small public API. Validate invariants at its boundary.
- Avoid cyclic dependencies, cross-layer shortcuts and mutable global state.
- Use the same domain terms in Rust, TypeScript, SQLite, the renderer protocol and Godot. Do not translate one concept into several competing names.
- Do not create speculative framework layers. Extract shared behavior when there is concrete reuse or a deliberate protocol/domain boundary.
- Public APIs and non-obvious invariants require concise documentation; ordinary code should remain readable without narrative comments.

Required dependency direction:

```text
domain crates
    ↑
commands and domain services
    ↑
persistence, import/export and renderer adapters
    ↑
Tauri application

packages/ui ← typed view models and callbacks ← app-specific React controllers

renderer-protocol ← Tauri supervisor and Godot renderer
```

Domain crates must not depend on React, Tauri, Godot or platform UI details. The React application and Godot renderer may consume shared schemas and generated types, but neither may redefine the canonical scene.

### 5.2 Reusable UI component contract

The planner UI uses the existing `packages/ui` library and its design tokens as the default source for controls, tables, dialogs, window chrome and input surfaces.

- Search for and reuse an existing component before adding an app-local equivalent.
- If a missing control is broadly reusable, implement it in `packages/ui`, demonstrate it with deterministic Storybook stories, and test it there before integrating it into the planner.
- Keep `packages/ui` presentational. It must not depend on application contexts, server APIs, Tauri/native integration, document persistence, renderer state or workspace layout state.
- Shared components accept typed view models, values and callbacks. Application-owned adapters in `apps/rig-planner` connect them to commands, services and state.
- Product-specific panels and workflows remain in `apps/rig-planner`, but still use shared primitives, tokens, typography, focus behavior and spacing.
- Preserve established operator terminology, labels, geometry and keyboard behavior. Do not fork nearly identical controls for the planner.
- Reusable components require Storybook coverage, interaction tests, keyboard/accessibility checks and stable visual examples.

Likely reusable seams include the existing common controls, window kit, modal input controls, fader/encoder surfaces, data tables and button grids. Their exact current APIs must be inspected rather than guessed.

### 5.3 Shared fixture-library contract

The fixture browser and definition hierarchy are:

```text
Manufacturer
└── Fixture
    └── Mode
```

The current ToskLight schema and asset format must be inspected before implementation and reused literally. The existing 3D asset appears to be GLB-compatible in the current stage path, but the plan must not invent a replacement format without verifying the live library.

**Manufacturer**

- Stable identifier and display name.
- Built-in or user-created origin.
- Optional branding and metadata.
- `Venue` is a built-in manufacturer containing scenery and stage elements.

**Fixture**

- Stable identifier and revision.
- Manufacturer relationship.
- Display name and equipment type.
- Physical metadata.
- Default icon, editor representation and 3D model.
- Ports and connector capabilities.
- GDTF source data where applicable.

**Mode**

- Stable identifier and name.
- For controllable fixtures, normally a DMX mode with channel footprint and functions.
- For scenery, an addressless model/physical variant.
- May override the fixture’s default model, dimensions, weight, ports or physical metadata.
- Examples under `Venue → Stage Element` include 10 cm and 20 cm stage-height variants.

Mode is therefore a library variant boundary, not an assertion that every entry consumes DMX.

### 5.4 Library authoring

Support:

1. Built-in definitions shipped with ToskLight.
2. Manual fixture/scenery definition.
3. Manual icon and model assignment.
4. GDTF import with retained source bytes.
5. User-defined manufacturers, fixtures and modes.
6. Revision-aware updates so existing shows do not silently change.

The planner reuses the same authoring and validation core as ToskLight. A desktop-oriented browser may use more mouse, keyboard, table and batch-edit affordances than the touch-oriented control surface, while preserving the same data and terminology.

### 5.5 Add versus patch

The fixture browser exposes two primary operations:

- **Add fixture:** creates one or more spatial instances with no required universe, address, circuit or output assignment.
- **Add and patch:** creates the instances and immediately enters the existing patch workflow.

An existing unpatched fixture can later be patched. Removing or changing a patch never deletes or moves the spatial fixture. Scenery and other addressless definitions only expose **Add**.

Patch state is optional instance data:

```text
Patch
├── protocol/domain
├── universe
├── address
├── mode
├── fixture identifier
├── power circuit
└── optional network/node assignment
```

## 6. Canonical scene model

### 6.1 Coordinate and unit contract

- Canonical world: right-handed, Z-up.
- Canonical distance: millimeters.
- Mass: kilograms.
- Force: newtons.
- Electrical values: volts, amperes, watts and volt-amperes as applicable.
- Angles: radians internally, degrees at operator surfaces.
- CPU calculations: `f64`.
- Renderer projections: validated conversion to `f32`.
- Godot adapter: one centrally tested Z-up-to-Y-up transform conversion.
- Every imported format records its source units and conversion.

### 6.2 Common entity data

Every scene entity has:

- Stable UUID.
- Entity kind.
- Name and display label.
- Parent or layer.
- Local transform.
- Visibility and class/layer membership.
- Tags and user data.
- Source provenance.
- Revision metadata.
- Optional asset-definition reference.

### 6.3 Representation layers

An equipment definition may provide:

- `render_geometry`: realistic GLB and material resources.
- `editor_geometry`: reduced mesh or parametric primitive.
- `plan_symbol`: 2D vector symbol.
- `elevation_symbol`: 2D vector symbol.
- `selection_bounds`: editor picking bounds.
- `snap_geometry`: points, axes, lines and planes.
- `physical_model`: mass, center of gravity and dimensions.
- `mount_points`: clamps, chords, yokes and connection nodes.
- `ports`: power, DMX, network, audio and media connections.

Missing data must degrade explicitly. Geometry must never silently invent a safe working load or equipment mass.

### 6.4 Principal entity types

- Venue.
- Room volume.
- Architectural mesh.
- Stage platform.
- Truss component.
- Truss system.
- Support and tower.
- Hanging point.
- Hoist or motor.
- Chain.
- Bridle leg.
- Fixture.
- PA enclosure or array.
- Video wall and panel.
- Projector.
- Distribution device.
- Network device.
- Generic equipment.
- Cable.
- Cable route.
- Focus point.
- Measuring annotation.
- Drawing sheet, viewport and title block.

## 7. Attachment and mounting model

Spatial proximity is not an attachment. Use explicit relationships:

```text
Attachment
├── child entity
├── parent entity
├── parent attachment node
├── child attachment node
├── local transform
├── mounting orientation
├── hardware definition
├── safety attachment
└── calculated load transfer
```

Truss definitions expose named members and attachment rails:

- top-left chord;
- top-right chord;
- bottom-left chord;
- bottom-right chord;
- center rail where applicable;
- end plates and connection nodes;
- manufacturer-approved attachment zones.

The editor offers semantic operations such as “mount below bottom-left chord,” not only free transforms. Free placement remains available but is visibly marked as unattached.

Moving an attached parent updates children. Splitting, replacing or rotating a truss preserves attachments where geometrically valid and presents a repair workflow otherwise.

### 7.1 Fixture installation and pan/tilt compensation

Fixture-definition behavior and installed-instance behavior remain separate.

Each moving-light instance records:

- Physical mounting orientation.
- Whether the fixture is top-mounted, bottom-hung, side-mounted or freely oriented.
- Hardware pan-reverse setting.
- Hardware tilt-reverse setting.
- Software pan-reverse compensation.
- Software tilt-reverse compensation.
- User override and the reason/source of each automatic value.

The application derives suggested software compensation from the mounting transform and known hardware settings so identically selected fixtures move in the same operator-facing direction. For example, a top-stacked fixture will normally receive the appropriate default tilt compensation. Pan compensation must be derived and tested from the complete orientation rather than guessed solely from a “top-mounted” flag.

The operator can accept or override the suggestion. Effective visualization and output use the composed mapping:

```text
requested parameter
→ software compensation
→ hardware reverse configuration
→ fixture physical axis
```

Changing hardware reverse settings proposes corresponding software changes but does not silently overwrite a deliberate user override. These settings are per installed fixture, not global mutations of the shared library definition.

### 7.2 Conventional fixtures and gels

An installed conventional/dimming fixture may reference:

- A built-in gel/filter definition.
- A user-created gel definition.
- An arbitrary color chosen with a color picker.
- No filter/open white.

The filter assignment affects the WebGL representation, Godot beam/color rendering, fixture schedule and gel paperwork. User-defined filters may later add manufacturer/code, spectral data and transmission; the prototype only requires a named color and optional note.

## 8. Venue and drawing workflow

### 8.1 New venue

The room tool creates:

- width, length and height;
- wall, floor and ceiling surfaces;
- optional stage and proscenium;
- origin and north/front direction;
- named rigging grid.

### 8.2 Imported venue

Initial imports:

- GLB/glTF.
- OBJ.
- 3MF.
- MVR geometry.
- PDF/image underlay.

Import flow:

1. Inspect file safely.
2. Show detected units and bounds.
3. Ask for units when ambiguous.
4. Select origin, up axis and scale.
5. Preview before committing.
6. Generate lightweight editor geometry.
7. Cache a renderer-ready artifact.
8. Retain import provenance and original asset.

3MF is supported as mesh interchange, not treated as a complete architectural or CAD model.

### 8.3 Editor views

The first planning workspace is a synchronized quad view:

1. Top/plan.
2. Side, switchable between left and right.
3. Elevation, switchable between front and back.
4. Isometric/orthographic 3D.

A separate Godot view provides high-quality 3D visualization. All five views share:

- One selection.
- One canonical set of object transforms.
- One active tool.
- Snapping and attachment results.
- Visibility/layer settings where applicable.
- Immediate propagation of committed mutations.

An object can be selected and moved from any planning pane. Axis constraints derive from the active view. The Godot view participates in selection and picking, but document mutations still pass through the Rust command core.

Display modes:

- Wireframe.
- Hidden line.
- Flat shaded.
- Material color.
- X-ray/attachment inspection.
- Cable routes.
- Load visualization.

React owns toolbars, properties, hierarchy, worksheets and status. The imperative viewport controller owns camera motion, pointer tracking, drag previews, hit testing and GPU resources.

## 9. Truss and rigging planning

### 9.1 Truss library

Each truss definition contains:

- Manufacturer, family, model and revision.
- Straight or curved geometry.
- Length and connection type.
- Cross-section and chord locations.
- Mass and center of gravity.
- Compatible corners, hinges and adapters.
- Connection rules.
- Manufacturer load tables where licensed and available.
- Approved attachment members/zones.
- Editor and render geometry.

Truss assemblies remain composed of identifiable inventory parts rather than being flattened into one mesh.

### 9.2 Hanging system

Support:

- Venue hanging points with identifiers and allowable loads.
- Motors/hoists with capacity, self-weight and chain properties.
- Chain length and hook positions.
- Direct drops.
- Motor-up and motor-down configurations.
- Multiple supports on a truss.
- Unresolved or unverified hanging points.

### 9.3 Preliminary calculation levels

**Level 1 — inventory and dead-weight summary**

- Sum known equipment and truss weights.
- Report missing masses.
- Compute assembly center of gravity.

**Level 2 — supported static reactions**

- Straight truss or beam.
- Defined support locations.
- Point and distributed loads.
- Self-weight.
- Static support reactions and utilization.

**Level 3 — manufacturer-table validation**

- Compare supported configurations against licensed manufacturer data.
- Reject interpolation where manufacturer rules do not allow it.

**Later — advanced solver**

- Multi-span systems.
- Frames and corners.
- Torsion.
- Bridles.
- Dynamic and environmental load cases.
- Structural-analysis engine validated with qualified engineers.

### 9.4 Safety behavior

- Unsupported configurations return “not calculated,” never zero or green.
- Missing weights and capacities are first-class blocking warnings.
- Reports list assumptions, solver version, inputs and source data.
- Preliminary reports carry a persistent non-certification notice.
- Safety factors and regulatory profiles are versioned, named and never silently changed.
- Certified workflows require a separate product and validation effort with qualified rigging/structural professionals.

## 10. Optional connectivity, racks, inventory and cable planning

All features in this section are optional per project. Fixtures and scenery can be placed, patched and visualized without defining ports, racks, stock or cable routes. Enabling logistics progressively adds warnings only for the systems the project elects to plan.

### 10.1 Connector library

Connector types are data, not a closed source-code enum. Ship a built-in catalog and allow users to add custom connector definitions.

A connector definition contains:

- Stable UUID and revision.
- Manufacturer/common name.
- Physical family and variant.
- Gender or genderless behavior.
- Pin/contact count and optional pin mapping.
- Keying/coding.
- Supported signal or power domains.
- Voltage, current, frequency, bandwidth and other applicable limits.
- Mating compatibility.
- Optional adapters.
- Symbol and optional model.

The initial built-in catalog should cover at least:

- XLR 3-pin and XLR 5-pin for DMX.
- RJ45 used for DMX transport and Ethernet.
- Common fiber connector families and media-converter endpoints.
- IEC appliance connectors.
- Schuko/CEE 7 mains connectors.
- CEE single- and three-phase variants with explicit current/voltage/keying.
- powerCON variants.
- powerCON TRUE1 variants.
- Common low-voltage DC connectors.
- Common audio connectors.

Product names and trademarks must be represented accurately and reviewed before shipping. Connector compatibility depends on more than visual similarity; CEE rating/keying and powerCON generations, for example, remain distinct definitions.

The catalog separates physical connector from carried service. An RJ45 port is not intrinsically “Art-Net”: it is an Ethernet-capable physical port whose device/service configuration may carry Art-Net, sACN, ordinary IP traffic or another protocol.

### 10.2 Equipment definitions and ports

Ports define:

- Domain: power, DMX, Ethernet, audio, video, fiber or custom.
- Connector-definition reference and gender.
- Input/output/thru behavior.
- Electrical or signal capacity.
- Maximum permitted downstream rules.
- Physical location on the equipment geometry.
- Logical name such as `DMX A`, `Universe B`, `Network Primary` or `Dimmer 1 Channel 6`.
- Optional internal module/port relationship.

Initial equipment families include:

- Fixtures and scenery.
- DMX nodes.
- Ethernet switches.
- Fiber/media converters.
- Power distributors.
- Dimmers.
- Network gateways.
- Audio and video devices.
- Generic user-defined equipment.

### 10.3 Composite racks and modules

A rack is a composite equipment definition. It contains modules and internal connections but exposes a simplified external contract to normal planning workflows.

Example:

```text
Rack
├── external three-phase inlet
├── power-distribution module
│   ├── external high-current outlets
│   └── internal feeds
├── dimmer module A
│   └── dimmed multicore/channel outputs
├── dimmer module B
│   └── dimmed multicore/channel outputs
└── DMX/network node
    ├── Ethernet/Art-Net input
    ├── DMX universe A on configured connector
    └── DMX universe B on configured connector
```

The exact ratings, modules and connector types are user-configurable; the example does not hard-code one national or company rack design.

Normal users see:

- External rack ports.
- Aggregate capabilities.
- Patchable dimmer channels.
- Network/DMX universe mapping.
- Weight, power demand and physical model.

An advanced editor can expand:

- Contained modules.
- Internal port-to-port connections.
- Internal power allocation.
- Module slot/position.
- Panel connector mapping.

Adding or removing a module recompiles the rack’s exposed capabilities. Rack instances reference a versioned rack definition so changing a warehouse template does not silently alter an existing show.

### 10.4 Service and power classes

Routes and paperwork are grouped by configurable service class. Initial built-ins:

- High-current power trunks.
- Fixture/low-current mains.
- Dimmed power.
- Low-voltage power.
- DMX.
- Ethernet/network.
- Audio.
- Fiber.
- Video/media.
- Custom.

The words “high current” and “low current” are planning categories, not substitutes for electrical ratings. Every power port and cable retains explicit voltage, phase, current, connector and capacity data.

### 10.5 Logical connections

The simplest connectivity workflow does not require a routed cable or warehouse:

1. Select a service class.
2. Click compatible source and destination ports/devices.
3. Optionally continue through fixtures in order.
4. Save the logical connection chain.

The application can therefore document that fixture A feeds fixture B without requiring exact cable lengths. Incompatible or ambiguous ports produce a choice or warning, not a guessed connection.

### 10.6 Route graph

When physical cable planning is enabled, routing uses a separate spatial graph:

- Truss route segments.
- Vertical drops.
- Floor paths.
- Cable bridges and trays.
- Distribution positions.
- Riser/down points.
- Restricted zones.
- User-defined waypoints.

A logical connection and physical route are separate. The logical chain says which ports connect; the physical route says where the cable travels.

### 10.7 Routed-cable workflow

1. Choose a cable or system type.
2. Click source equipment/port.
3. Click fixtures or devices in order.
4. End the chain.
5. Review automatically chosen ports.
6. Accept or edit the proposed route.
7. Apply slack, tails and service-loop policies.
8. Calculate required stocked cables.
9. Resolve shortages or incompatible connections.

### 10.8 Length calculation

```text
route polyline
+ port-to-route tails
+ vertical drops
+ service loops
+ connector allowance
+ configured slack
= required length
```

Every contribution is inspectable.

### 10.9 Warehouse stock and optimizer

Inputs:

- User-defined cable types, connector ends and capabilities.
- Available lengths.
- Quantity owned, rented or reserved.
- Connector compatibility.
- Maximum joins.
- Preferred spare percentage.
- Preference for fewer joins or lower excess.

Outputs:

- Cable assignment per run.
- Installed versus required length.
- Excess and join positions.
- Shortages.
- Shopping/rental list.
- Unused inventory.

Electrical validation, such as voltage drop and circuit capacity, is a separate rule layer and must not be implied merely by a successful geometric route.

Warehouse records are optional and independent of library definitions. A cable type can exist without stock counts; a show can request cables that are not currently owned; shortages become explicit rental/purchase requirements.

The warehouse model can later count fixtures, truss parts, racks, modules and other equipment through the same versioned-definition identity. Cable types and cable quantities are the first required stock workflow.

## 11. MVR, GDTF and export

### 11.1 MVR workflows

- Create a new project from MVR.
- Import an MVR into an existing project through preview-and-apply.
- Export any project as standards-compliant MVR.
- Preserve source GDTF bytes and unsupported MVR content.
- Keep unresolved fixtures as visible placeholders and warnings.
- Validate the staged document before replacing the active document.

### 11.2 Application-specific data

MVR will not represent every cable, inventory or calculation concept. Store complete data in SQLite. For MVR:

- Export standardized data in standardized nodes.
- Use a namespaced extension only where allowed and documented.
- Never make application extensions necessary for another program to read the standard scene.
- Preserve imported provider data even when not interpreted.

### 11.3 Other exports

- GLB scene with baked placement transforms.
- 3MF geometry export where useful.
- DXF plan/elevation.
- PDF drawing sheets.
- CSV equipment, fixture, cable and motor schedules.
- JSON diagnostic export for support and testing.
- PNG/EXR renderer screenshots.

## 12. Godot visualizer

### 12.1 Selected baseline

- Godot 4 Forward+.
- Native Vulkan, Direct3D 12 or Metal driver as appropriate.
- Separate packaged executable/process.
- MIT-compatible engine dependency.
- GDScript for scene orchestration where adequate.
- Custom shaders for fixture optics, beams and post-processing.
- C++ or Rust GDExtension only when profiling proves it necessary.

Pin a supported Godot minor version. Engine upgrades are deliberate projects with benchmark and screenshot comparison.

### 12.2 Renderer scene

The renderer maintains:

- Instanced venue and stage meshes.
- Instanced truss and equipment meshes.
- Fixture kinematic hierarchies from the shared ToskLight definition and selected mode.
- Per-instance mounting orientation and effective pan/tilt compensation.
- Light-emitter components.
- Shadow allocation state.
- Volumetric-light participation.
- Gobo, prism and shutter textures/parameters.
- Conventional-fixture gel/filter state.
- Media-surface textures.
- Camera and post-processing state.

It does not contain inventory, worksheets, undo history or canonical attachment rules.

### 12.3 Fixture rendering increments

**R1 — base scene**

- Loading the exact model-asset format already used by the ToskLight fixture library, with GLB as the expected format to verify.
- Mode-specific model overrides for fixtures and `Venue` scenery.
- PBR materials.
- Camera navigation.
- Object picking.
- Flat environmental lighting.

**R2 — fixture fundamentals**

- GDTF geometry hierarchy.
- Pan and tilt.
- Hardware reverse and software-compensation composition.
- Correct movement for top-mounted, bottom-hung and arbitrarily rotated instances.
- Intensity and strobe.
- Color emitters and basic filters.
- Gel/custom-filter color for conventional fixtures.
- Beam angle/zoom.
- Shadowed spotlights.

**R3 — theatrical optics**

- Gobos, indexed and rotating.
- Continuous gobo-wheel position and physical transitions between adjacent slots.
- Partial-slot occlusion and blackout while the wheel traverses opaque regions, instead of instantaneous texture replacement.
- Iris.
- Focus and edge softness.
- Frost.
- Shutters.
- Animation wheels.
- Prism approximation.

**R4 — atmosphere and perception**

- Volumetric haze.
- Shadowed beams.
- Bloom and glare.
- Exposure adaptation.
- Configurable retinal-persistence buffer.
- Quality tiers and temporal stabilization.

**R5 — media and effects**

- Video/LED-wall textures.
- CITP/MSEX preview ingestion.
- Pluggable NDI/Spout/Syphon or decoded-file sources.
- Simple laser visualization.
- Fire and smoke particles.

### 12.4 Shadow and fog budgets

Not every active fixture receives a full-resolution shadow map. The renderer uses:

- Shadow-light prioritization.
- Distance and screen-contribution scores.
- Fixed atlas budgets.
- Per-fixture importance overrides.
- Hysteresis to prevent shadow flicker.
- Lower-cost unshadowed volumetric contribution for secondary lights.
- Quality profiles based on detected GPU limits.

### 12.5 Provisional performance targets

Targets must be validated by the benchmark phase, but the initial acceptance envelope is:

- 60 fps at 2560×1440 on representative recommended hardware.
- 500 placed fixtures.
- 100 simultaneously active moving beams.
- 16–32 prioritized shadow-casting fixtures, depending on quality tier.
- Stable camera interaction while live values update at 44–60 Hz.
- No full-scene rebuild for live value changes.
- Renderer restart and scene resynchronization in seconds, not minutes.

Publish both minimum and recommended hardware after measurements rather than guessing them in marketing material.

## 13. Renderer protocol

### 13.1 Transport

The initial implementation uses framed messages over loopback TCP for identical behavior on all platforms. Move large dynamic media to shared memory only after profiling.

### 13.2 Message families

- `Hello` / `Capabilities` / `ProtocolMismatch`.
- `LoadSnapshot`.
- `CreateEntity`, `UpdateEntity`, `DeleteEntity`.
- `UpdateFixtureLibraryReference`, `UpdateFixtureMode`, `UpdatePatch`.
- `UpdateMountOrientation`, `UpdateAxisCompensation`, `UpdateGel`.
- `UpdateAttachmentProjection`.
- `LoadAsset` / `AssetReady` / `AssetFailed`.
- `UpdateFixtureValuesBatch`.
- `SetCamera` / `CameraChanged`.
- `PickRequest` / `PickResult`.
- `SetQuality`.
- `CaptureFrame`.
- `Metrics`.
- `Heartbeat`.
- `RequestFullSnapshot`.
- `Shutdown`.

### 13.3 Reliability

- Protocol version and feature flags.
- Monotonic scene revision.
- Sequence numbers for live batches.
- Idempotent entity updates.
- Full-snapshot recovery.
- Maximum message and asset sizes.
- Timeouts and process supervision.
- Structured renderer logs collected by the application.

### 13.4 Live values

Send resolved fixture parameters, not raw UI state. Batch them by frame:

```text
fixture UUID
parameter identifier
normalized or physical value
timestamp
```

Godot interpolates pan, tilt and media timing locally. React is never in the live renderer frame loop.

## 14. Document and asset persistence

### 14.1 SQLite document

Store:

- Scene entities and transforms.
- Attachments.
- Shared ToskLight fixture/scenery instance references and definition revisions.
- Selected mode and mode-specific asset override.
- Optional patch and live-control references.
- Per-instance mount orientation and pan/tilt compensation.
- Per-instance gel/filter assignment.
- Connector, equipment-topology and composite-rack definition references.
- Cable networks and routes.
- Inventory assignments.
- Rigging inputs and calculation results.
- Sheets, viewports and annotations.
- User-defined fields.
- Import provenance.
- Application version and schema version.

### 14.2 Asset strategy

Use a content-addressed asset store:

- SHA-256 or equivalent content identity.
- Shared ToskLight model/icon identity and revision.
- Original imported file.
- Normalized metadata.
- Editor-ready reduced mesh.
- Godot-ready render artifact.
- Thumbnail and preview.
- License and source metadata.

The document references immutable library and asset revisions. Replacing a shared ToskLight library definition creates an explicit upgrade operation rather than silently changing old projects. The planner may cache derived editor/Godot assets but must not fork the semantic fixture definition.

### 14.3 Portable show package

Decide during Phase 1 between:

1. One SQLite file with embedded small assets and external cache references.
2. A ZIP-based package containing `show.sqlite` and referenced assets.

Recommended default: a packaged show archive for portability, while allowing an unpacked development form. Large media remains linked unless the user explicitly collects it into the package.

### 14.4 Migration safety

- Backup before migration.
- Transactional migration.
- Never open a newer unsupported schema read-write.
- Provide read-only inspection where possible.
- Golden migration tests from every released schema.

## 15. React application

The React application lives in `apps/rig-planner` and composes the existing reusable controls from `packages/ui`. React owns product UI and coordinates editor state; it does not own frame-by-frame viewport state or duplicate Rust domain rules.

### 15.1 Main workspaces

- Fixtures.
- Patch.
- Plan/quad view.
- Visualizer.
- Venue.
- Rig.
- Libraries.

Optional planning workspaces:

- Cables.
- Racks and topology.
- Warehouse.
- Loads.
- Sheets and reports.

### 15.2 Common layout

- Custom title bar and application menu.
- Left tool palette.
- Central drawing viewport.
- Right properties/inspector.
- Bottom status, coordinates and warnings.
- Dockable hierarchy, worksheets and issue panels.
- Saved workspace layouts.

### 15.3 State boundary

**React-visible state**

- Current document snapshot/revision.
- Selection.
- Active tool.
- Inspector values.
- Workspace layout.
- Issues and calculation summaries.

**Viewport-local transient state**

- Pointer and hover.
- Camera matrices during movement.
- Drag ghosts.
- Temporary snaps.
- GPU resource handles.
- Animation frame state.

Drag operations update viewport previews directly and commit one semantic command on completion.

### 15.4 Undo

Undo records mutations, not navigation:

- Insert/remove entity.
- Move/attach/detach equipment.
- Change definition or properties.
- Create/edit cable chain.
- Change route.
- Assign inventory.
- Modify room or truss.

Camera movement, selection changes and switching workspaces are not document undo steps.

## 16. Reporting and sheets

Version 1 reports:

1. **Load paperwork**
   - Truss loads.
   - Motor/support reactions.
   - Hanging-point loads and utilization.
   - Missing or unverified physical data.

2. **Placement paperwork**
   - Fixture and equipment locations.
   - Truss and mounting relationship.
   - Top/bottom/side mounting.
   - Fixture identifiers and orientation.

3. **Fixture schedule**
   - All fixtures, including unpatched fixtures.
   - Fixture type and selected mode.
   - Patch, DMX universe/address and node assignment.
   - Power or dimmed circuit.
   - Attached truss/position.
   - Gel/filter.

4. **Cable-run paperwork**
   - Separate schedules or filters for high-current power, fixture mains, dimmed power and low-voltage power.
   - DMX.
   - Ethernet/network.
   - Audio.
   - Fiber.
   - Video/media and custom services.
   - Logical connections may be reported even when exact physical routes or stock assignments are absent.

5. **Inventory paperwork**
   - Required cables and equipment.
   - Assigned warehouse stock.
   - Shortages and rental/purchase list.

Reporting and paperwork are not prerequisites for the first prototype. Their schemas should be preserved in the model, but polished report generation follows fixture placement, synchronized views and visualizer work.

Drawing sheets support:

- Page size and scale.
- Title blocks.
- Plan, elevation and section viewports.
- Dimensions and annotations.
- Layer/class visibility.
- Vector output where possible.
- Rasterized venue meshes only where necessary.

PDF output must be deterministic and independently tested; it must not merely screenshot the current viewport.

## 17. Security and robustness

Treat show files, MVR archives, GDTF archives and model files as untrusted:

- Prevent ZIP path traversal.
- Limit archive expansion ratio and total size.
- Limit mesh vertices, indices, textures and dimensions.
- Reject executable/script content from imported assets.
- Normalize external paths.
- Never let imported Godot resources execute scripts.
- Run conversion in bounded worker processes where practical.
- Validate XML and SQLite schema before applying.
- Keep the renderer asset cache read-only from imported script logic.

## 18. Testing strategy

### 18.1 Unit and property tests

- Unit conversions.
- Transform hierarchy and Godot coordinate conversion.
- Shared ToskLight manufacturer/fixture/mode identity and revision mapping.
- Mode-specific and fixture-default model selection.
- Unpatched-to-patched transition without instance replacement.
- Attachment propagation.
- Mount-orientation and hardware/software pan/tilt compensation.
- Port compatibility.
- Custom connector definition and adapter compatibility.
- Composite-rack exposed-capability compilation.
- Cable length breakdown.
- Stock optimization.
- Static reaction cases.
- Missing-data and unsupported-configuration behavior.
- SQL migrations.

### 18.2 Interchange tests

- MVR import/export round trips.
- Source GDTF retention.
- UUID preservation.
- Unsupported provider-data preservation.
- GLB transforms and units.
- 3MF units and materials.
- Malformed and malicious archives.

### 18.3 UI tests

- Shared controls in `packages/ui` through deterministic Storybook stories and interaction tests.
- Planner adapters against typed view models and callbacks, without Tauri or server dependencies in the shared components.
- Manufacturer → Fixture → Mode browser.
- Add fixture versus add-and-patch.
- Addressless `Venue` scenery/model variants.
- Synchronized selection and movement across top, side, front/back and isometric panes.
- Placement, snapping and attachment workflows.
- Click-in-order cable creation.
- Route editing.
- Calculation warning flows.
- Undo/redo.
- Cross-surface selection between plan, hierarchy and visualizer.
- Deterministic screenshots on all three platforms.

### 18.4 Renderer tests

- Fixed-camera golden images with tolerance.
- Pan/tilt and kinematic fixtures.
- Top-mounted/bottom-hung inversion and user overrides.
- Conventional dimmer fixtures with built-in and arbitrary gel colors.
- Gobo, zoom, focus and color fixtures.
- Animated gobo-wheel traversal with partial occlusion/blackout between slots.
- Shadow allocation stability.
- Volumetric beam occlusion.
- Restart/resynchronization.
- Asset failure fallback.
- GPU metrics and memory-growth tests.

### 18.5 Performance scenes

- Small theatre.
- Medium touring rig.
- Large arena.
- Dense truss and cable plan.
- Hundreds of moving fixtures.
- Media-heavy stage.
- Worst-case imported venue mesh.

Record CPU time, GPU time, memory, startup, snapshot load, live-update latency and renderer restart time on Windows, Apple Silicon macOS and representative Linux AMD/NVIDIA systems.

### 18.6 Safety-calculation validation

- Hand-calculated canonical cases.
- Independent reference calculations.
- Boundary and invalid configurations.
- Versioned expected reports.
- Domain-expert review before any public load-calculation release.

### 18.7 Code-quality and architecture gates

Every pull request must pass the relevant workspace checks:

- Rust formatting, linting, unit tests and workspace build.
- TypeScript formatting, linting, type checking, unit/interaction tests and production build.
- Storybook build and tests for changed reusable UI components.
- Generated TypeScript/protocol types checked for drift from their authoritative schemas.
- Dependency-boundary checks preventing domain crates from importing application/platform layers and preventing `packages/ui` from importing Tauri, server or product-state modules.
- Duplicate-component review when a new app-local control resembles an existing shared component.
- Focused review of large methods, components, classes or services that mix multiple responsibilities.
- Cross-platform CI for the shared Rust crates and the `apps/rig-planner` build.

There is no arbitrary requirement that every method have the fewest possible lines. The enforceable standard is that names remain concise and clear, responsibilities remain singular, and a unit can be understood and tested without reconstructing unrelated application state.

## 19. Cross-platform delivery

### 19.1 Windows

- WebView2/Tauri shell.
- Godot Direct3D 12 or Vulkan capability testing.
- Signed installer.
- Named application identity shared by editor and renderer.
- NVIDIA, AMD and Intel test coverage.

### 19.2 macOS

- Universal application where feasible.
- Apple Silicon primary performance target.
- Signed and notarized app bundle.
- Renderer helper packaged inside the app.
- Metal rendering.
- Correct application/window grouping and shutdown.

### 19.3 Linux

- Define supported distributions and libc baseline.
- Test Wayland and X11.
- Vulkan primary renderer path.
- AMD, Intel and NVIDIA driver coverage.
- Desktop portal file dialogs.
- Select AppImage, Flatpak and/or distribution packages after dependency testing.

## 20. Licensing plan

Godot is compatible with a genuinely open-source application, but code, fixture data, manufacturer models and imported user assets have separate licensing concerns.

Before accepting external contributions, choose one standard license:

- **Apache-2.0 or MIT** for permissive reuse, including commercial use.
- **GPL-3.0** if distributed modified versions should remain open under its conditions.

A “modified MIT” license that forbids selling is not MIT and is not open source under the Open Source Definition. If commercial prohibition is required, describe the project as source-available and use a professionally reviewed license.

Recommended open-source commercial-protection approach:

- Standard OSI-approved code license.
- Protected project name and logo.
- Trademark policy for official builds.
- Signed official binaries.
- Separate notices for manufacturer-supplied data and assets.
- Developer Certificate of Origin or contributor agreement decision before accepting contributions.

Obtain legal review before release; this plan is not legal advice.

## 21. Delivery phases

Phases are ordered by current product value: fixture list and patch first, synchronized planning views second, high-quality visualization third, and optional management/paperwork afterward. They are gated by evidence, not only completion of tasks.

### Phase 0 — live reuse audit and cross-platform spike

Duration target: 4–8 weeks

Deliver:

- Establish `apps/rig-planner` and `apps/visualizer-godot` inside `/Users/keller/repos/light`.
- Map existing workspace crates and packages before proposing any new crate or duplicate model.
- Audit `packages/ui` and its Storybook stories for reusable planner controls.
- Inspect and document the live ToskLight Manufacturer → Fixture → Mode schema.
- Confirm the exact icon/model asset type and existing model-loading path.
- Confirm shared library revision, manual authoring and GDTF-import contracts.
- Confirm existing patch, MVR, resolved-value and CITP seams.
- Minimal Tauri shell on Windows, macOS and Linux.
- Minimal Godot helper launch and heartbeat on all platforms.
- A bounded Godot benchmark proving the intended shadowed-beam/fog direction before deeper renderer work.

Gate:

- The applications build as members of the existing `/Users/keller/repos/light` workspace.
- The prototype uses `packages/ui` controls through app-owned adapters and introduces no copied common controls.
- Shared Rust behavior is consumed through workspace crates rather than duplicated inside the Tauri app.
- The planner reads the real library and shows the same fixture/mode identity as ToskLight.
- One built-in fixture and one manually imported GDTF load through the shared path.
- No second fixture schema or copied library is introduced.
- Godot shows no early cross-platform blocker for the target visual direction.

### Phase 1 — fixture list, placement and patch prototype

Duration target: 6–10 weeks

Deliver:

- Desktop-oriented Manufacturer → Fixture → Mode browser composed from `packages/ui`.
- Storybook stories and interaction tests for any new reusable components before planner integration.
- **Add fixture** and **Add and patch** actions.
- Unpatched fixtures in the canonical fixture list.
- Patch editing through reused ToskLight behavior.
- Addressless `Venue` scenery and mode-based stage/model variants.
- Position, rotation and basic metadata editing.
- Optional gel/custom color on conventional fixtures.
- SQLite persistence, undo, autosave and recovery for this slice.

Gate:

- Shared controls contain no planner state, Tauri integration or server dependency.
- App-specific adapters remain in `apps/rig-planner`, with the workspace dependency rules passing in CI.
- Fixtures can be added unpatched, patched later and unpatched again without losing position or identity.
- Scenery variants never require a fake DMX footprint.
- Save/reopen preserves fixture, mode, patch, transform and gel state.

### Phase 2 — synchronized planning views

Duration target: 10–16 weeks

Deliver:

- Quad WebGL workspace with top, side, front/back and isometric panes.
- Shared selection and tool state.
- Move, rotate, duplicate and delete from every pane.
- Axis constraints and snapping derived from the active pane.
- Lightweight models from the shared fixture/scenery library.
- Basic room, truss and stage placement sufficient for spatial planning.
- Optional imported venue mesh.
- Keyboard/mouse-first editing and batch fixture operations.

Gate:

- Moving an object in any pane updates all other panes in the same interaction.
- A representative fixture plot can be positioned without opening the high-quality renderer.
- Hundreds of fixtures remain interactive without routing frame-time movement through React reconciliation.

### Phase 3 — Godot visualizer prototype and v1

Duration target: 20–32 weeks

Deliver in order:

1. Supervised renderer process, snapshot/delta protocol and synchronized picking.
2. Shared fixture/scenery model loading.
3. Fixture kinematic hierarchy and mount orientation.
4. Hardware reverse plus software pan/tilt compensation.
5. Dimmer, strobe, color and conventional gels.
6. Zoom, focus, iris, frost and shutters.
7. Continuous gobo-wheel traversal, rotation and transition occlusion.
8. PBR scene, shadows, haze and volumetric beams.
9. Bloom, exposure, temporal persistence and quality profiles.

Gate:

- Moving or patching a fixture updates Godot without a full-scene reload.
- Top-mounted and bottom-hung fixtures behave consistently under documented compensation rules.
- Gobo changes visibly traverse the wheel rather than snapping textures.
- Curated comparison scenes meet the agreed grandMA3-level target.
- Renderer crash/restart does not lose document work.

### Phase 4 — complete spatial rig model

Duration target: 12–20 weeks

Deliver:

- Full truss library and assemblies.
- Named truss members/chords and semantic mounting.
- Hanging points, motors and chains.
- PA, generic equipment and distribution placement.
- Venue import calibration for GLB/OBJ/3MF and plan underlays.
- Measurements, annotations, layers/classes and focus points.
- Transactional MVR/GDTF import/export and retained source data.

Gate:

- A representative theatre rig can be built without freehand coordinate entry.
- Parent movement and truss replacement preserve or explicitly repair attachments.
- MVR round trips do not silently lose understood or retained provider data.

### Phase 5 — optional connectivity, racks and logical networks

Duration target: 10–16 weeks

Deliver:

- Built-in connector catalog and custom connector editor.
- Equipment ports and compatibility.
- DMX nodes, switches and media converters.
- Power distribution and dimmer modules.
- Composite rack definition and external capability compilation.
- Logical click-in-order connection chains without required cable routing.
- Service classes for power, dimmed power, low voltage, DMX, network, audio and fiber.

Gate:

- A configurable rack exposes the correct external power, dimmer, network and DMX capabilities.
- Users can document simple device-to-device connections without configuring stock.
- Custom connectors participate in compatibility and reporting like built-ins.

### Phase 6 — optional cable routing and warehouse

Duration target: 10–18 weeks

Deliver:

- Spatial route graph and truss/drop routing.
- Route editing and vertical drop points.
- Explainable cable-length breakdown.
- Cable type and stock quantities.
- Discrete-length optimizer.
- Shortage and rental/purchase results.

Gate:

- Reference jobs match independently measured cable schedules.
- Every computed length is explainable.
- Projects with logistics disabled remain free of irrelevant missing-cable warnings.

### Phase 7 — preliminary loads and paperwork

Duration target: 14–24 weeks with domain-expert involvement

Deliver:

- Mass and center-of-gravity validation.
- Supported static truss/support calculations.
- Motor and hanging-point utilization.
- Load paperwork.
- Placement paperwork.
- Fixture/patch/power/truss schedule.
- Cable schedules separated by service class.
- Inventory and shortage paperwork.
- Deterministic PDF and CSV output.

Gate:

- Solver passes canonical reference cases.
- A qualified reviewer approves boundaries and wording.
- Unsupported calculations never appear successfully verified.
- Paperwork matches the canonical scene rather than maintaining duplicate report-only data.

### Phase 8 — production hardening and advanced visualization

Deliver:

- Signed/notarized cross-platform packages.
- Installer/updater strategy.
- Accessibility and keyboard navigation.
- GPU fallbacks and published hardware matrix.
- Manuals and onboarding show.
- CITP/MSEX and media surfaces.
- NDI/Spout/Syphon adapters where appropriate.
- Lasers, fire/smoke, recording and offline rendering.
- Higher-end fixture optics and photometry.

## 22. Staffing and schedule expectation

A credible first public version is a multi-person, multi-year effort.

Recommended core team:

- Product/domain lead with production and lighting experience.
- React/WebGL editor engineer.
- Rust/document/interchange engineer.
- Godot/graphics engineer.
- Rigging and structural domain consultant.
- QA/automation contribution shared or dedicated.

With four to six experienced contributors, a focused useful MVP could be reached in roughly 12–18 months; a polished v1 with robust planning, interchange and grandMA3-class visualization is more plausibly 18–30 months. A solo implementation should expect several years and should reduce v1 aggressively.

## 23. Principal risks and mitigations

| Risk | Mitigation |
|---|---|
| Semantic model becomes a generic mesh scene | Define attachments, ports, inventory and physical metadata before UI expansion |
| Godot performs poorly with many moving shadowed fixtures | Phase 0 benchmark; explicit shadow/fog budgets; engine-neutral protocol |
| Planner and ToskLight fixture definitions diverge | Consume the shared library/schema directly; no copied fixture database |
| Planner UI drifts from the rest of ToskLight | Reuse `packages/ui`; add missing reusable controls through Storybook before app integration |
| Shared UI becomes coupled to planner or Tauri state | Typed view models and callbacks; app-owned adapters; automated dependency-boundary checks |
| Large components or services become difficult to change | Short single-purpose methods, focused modules, shallow hierarchies and responsibility-focused review gates |
| DMX mode assumptions break scenery variants | Explicit addressless/variant mode semantics and validation |
| Unpatched fixtures are treated as invalid | Make patch an optional relationship independent of spatial existence |
| Imported venue models overwhelm editor/renderer | Import budgets, LOD generation, culling and asset conversion |
| MVR round trips lose provider data | Retain original data and source GDTF; transactional preview/apply |
| Load estimates are mistaken for certification | Hard scope boundaries, qualified review, explicit unsupported states |
| Cross-platform renderer diverges | Golden scenes and GPU test matrix from Phase 0 |
| UI becomes slow despite Tauri | Keep frame-time viewport state outside React reconciliation |
| Renderer protocol becomes chatty | Snapshots plus batched deltas; profile before shared memory |
| Connector catalog confuses physical plug and carried protocol | Separate connector type, port capability and configured service |
| Rack internals leak complexity into every plan | Compile composite rack definitions to stable external ports/capabilities |
| Automatic pan/tilt compensation surprises operators | Show derived settings, preserve overrides and test real mounting orientations |
| Open-source asset licensing is unclear | Per-asset provenance/license metadata and release checks |
| Scope expands into general CAD | Maintain explicit non-goals and phase gates |

## 24. Decisions to settle before Phase 1

The repository decision is settled: the application lives in `/Users/keller/repos/light`, with the planner under `apps/rig-planner`, the Godot project under `apps/visualizer-godot`, shared Rust behavior under workspace crates and reusable React controls under `packages/ui`.

1. Is the license permissive (MIT/Apache-2.0) or reciprocal (GPL-3.0)?
2. What is the portable show-package extension and internal structure?
3. Which Linux distributions and packaging formats are supported initially?
4. Which exact Godot minor version is pinned?
5. Which fixture/venue benchmark scene defines the visual target?
6. Which preliminary rigging cases are in v1 and which are rejected?
7. Which manufacturer data may legally ship in the default library?
8. What constitutes the minimum recommended GPU?
9. Are official builds allowed to use a protected trademark while forks use another name?
10. What is the exact current fixture model-asset format and which mode-level overrides already exist?
11. Which built-in connector catalog is safe and useful to ship initially?
12. Which mount orientations should automatically suggest pan reverse, tilt reverse or both for each fixture geometry?

## 25. First concrete implementation slice

Build the first prototype in this order:

1. Create the planner and Godot app boundaries inside the existing `/Users/keller/repos/light` workspace.
2. Audit and reuse the required `packages/ui` controls; add and test missing reusable pieces in Storybook.
3. Open the real shared ToskLight fixture library through its existing Rust crate.
4. Display Manufacturer → Fixture → Mode in a desktop fixture browser.
5. Add one fixture without patching it.
6. Add and patch a second fixture through the reused patch core.
7. Add one addressless `Venue` stage-element variant.
8. Show all three instances in synchronized top, side, front/back and isometric panes.
9. Select and move each instance from every pane.
10. Save/reopen and prove that library identity, mode, patch and transform survive.
11. Launch Godot as a supervised helper and send the scene snapshot.
12. Load the exact shared fixture/scenery model assets.
13. Move a fixture and prove all planning panes and Godot update without scene reload.
14. Configure mount orientation and hardware/software pan/tilt compensation.
15. Send pan, tilt, dimmer, color and gel values incrementally.
16. Animate one gobo change through the physical wheel transition.
17. Render one shadowed volumetric beam.
18. Crash Godot deliberately and prove automatic restart/resynchronization.
19. Pass architecture, formatting, lint, type-check, test and Storybook gates.
20. Build and run the slice on Windows, macOS and Linux.

Do not implement warehouse stock, routed cables, rack internals, load reports or polished paperwork in this first prototype. Their future data boundaries remain documented, but they must not delay proof of the shared fixture library, quad planning views or visualizer.

## References

- [Godot renderer overview](https://docs.godotengine.org/en/4.6/tutorials/rendering/renderers.html)
- [Godot volumetric fog](https://docs.godotengine.org/en/4.6/tutorials/3d/volumetric_fog.html)
- [MVR specification](https://gdtf-share.com/help/developers/mvr_1_6/index.html)
- [GDTF specification](https://gdtf-share.com/help/developers/gdtf_1_2/index.html)
- [3MF specification](https://3mf.io/spec/)
- [Open Source Definition](https://opensource.org/osd)
- [MIT license](https://opensource.org/license/MIT)
- [ESTA published standards](https://tsp.esta.org/tsp/documents/published_docs.php)
- [DIN EN 17206](https://www.dinmedia.de/de/norm/din-en-17206/347804636)
