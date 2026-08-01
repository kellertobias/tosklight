# Network-Connected Live Visualizer MVP

## Status

**Implemented.** The MVP described here is built and running. See
[`apps/viz-renderer/README.md`](../../../../apps/viz-renderer/README.md) for the operator
documentation and [Implementation evidence](#implementation-evidence) below for what was measured.

The application lives in `apps/viz-renderer` with its reusable behaviour in `crates/viz/scene`,
`crates/viz/dmx`, `crates/viz/project`, `crates/viz/desk`, and `crates/viz/render`. Building or
opening ToskLight does not build it.

This is a focused first delivery slice of the
[Rig Planner and Lighting Visualizer application plan](./rig-planner-visualizer-application-plan.md).
It does not replace that canonical long-term plan or its
[architecture decisions](./architecture-decisions.md). It deliberately proves the live renderer
before the complete planner, paperwork, inventory, and offline document workflows.

The MVP is also separate from the
[efficient visualizer embedded in the ToskLight Stage](../../refactoring/finished/21-efficient-built-in-stage-visualizer.md).
The standalone application has its own build, process, window, connection lifecycle, rendering
quality, and failure budget. The desk must continue to build and run without it.

The canonical application's internal renderer protocol names `UpdateFixtureValuesBatch`. In this
MVP such a batch may still exist inside the process between the UDP decoder and render core, but
its authoritative source is always received Art-Net/sACN. The Viz editor, desk API, and existing
visualization WebSocket must not originate those live batches for this slice.

The renderer window is source-provider independent. The first implemented provider connects to a
ToskLight lighting desk. A later provider connects to the local Viz planning software. Both
providers project into the same renderer-owned semantic scene/value boundary; the render core must
not hardcode desk HTTP routes, Art-Net, sACN, or planner IPC.

## Goal

Build a standalone ToskLight visualizer that:

- starts without the main ToskLight desktop application being part of its process;
- connects to a ToskLight server over the network, using the local server by default;
- obtains fixture identity, fixture mode, patch, model, physical placement, scenery, and
  visualizer-view configuration from the desk API;
- receives live output values only as real Art-Net and/or sACN UDP packets;
- supports a desk and visualizer running on the same computer as well as on different computers;
- exposes a keyboard-accessible **Quick Settings** menu for source, server, port, rendering
  quality, and the haze amount;
- renders every light-producing fixture or fixture head with an explicit generic fallback instead
  of silently omitting unsupported optics;
- renders pixel-only emitters as visible light sources without inventing projected beams;
- renders visible beam atmosphere from its own haze amount; and
- supports named orthographic views plus line, simple, and full 3D rendering modes.

The first launch should require no network configuration when ToskLight is running on the same
computer and has a compatible local output route. The operator can enter a different desk address
when connecting across a lighting network.

## MVP product boundary

### Included

- One independently packaged renderer application with its own native window.
- A minimal connection/startup surface, visible connection state, and actionable diagnostics.
- A default desk API base URL of `http://127.0.0.1:5000`.
- A configurable desk hostname or IP address, port, authentication, and network interface.
- A source-provider boundary that can later select a local planning-software server without
  changing renderer-scene or GPU code.
- A **Quick Settings** menu opened with **Command+,** on macOS and **Ctrl+,** on Windows/Linux,
  with the same action available from the application menu.
- Read-only consumption of the active show and desk-owned visualizer configuration.
- Initial full scene synchronization followed by revisioned scene/configuration changes.
- Art-Net input.
- sACN (ANSI E1.31) input.
- Simultaneous listeners for configured Art-Net and sACN routes.
- Same-host UDP operation through the operating system network stack.
- Cross-host operation on a lighting network.
- Orthographic, line, simple-previsualization, and full-visualization rendering modes.
- Generic support for all light-producing fixture heads described by ToskLight fixture data.
- Renderer-owned participating atmosphere and volumetric beam visibility.
- Explicit stale-data, missing-asset, unsupported-profile, and disconnected states.
- A render-core boundary that can later be hosted inside the main application's Stage viewport.

### Not included

- A fixture, patch, or stage-position editor inside the visualizer.
- A REST or WebSocket server hosted by the visualizer.
- Sending commands, programmer changes, selection, pan/tilt, or other live control back to the desk.
- Reading live fixture values from `/api/v2/output/visualization` or another desk HTTP/WebSocket
  visualization feed.
- Making the main desktop application launch, supervise, or depend on this MVP.
- Replacing the existing Stage pane in this delivery.
- Streaming rendered video back into the main application.
- Full rig planning, paperwork, cable routing, stock, load calculations, or MVR editing.
- Offline show-file editing.
- The production local planning-software provider; this plan defines its boundary and future
  acceptance contract, but the desk provider is the MVP connection implementation.
- Fluid-dynamics simulation of fog.
- Photometric certification or a promise that the rendered image exactly predicts a real venue.
- Perfect simulation of every manufacturer-specific optical artifact in the first release.

## Authoritative data and ownership

Exactly one scene provider is active at a time.

### Lighting-desk provider

The MVP lighting-desk provider has two deliberately separate input planes.

| Input | Authority | Transport | Examples |
| --- | --- | --- | --- |
| Scene and configuration | ToskLight server | Authenticated desk HTTP snapshot plus established WebSocket events | Fixture identity, manufacturer/fixture/mode reference, patch, physical instances, models, stage transforms, scenery, view and input configuration |
| Live output | Lighting network | Art-Net and/or sACN UDP | Intensity, color, pan, tilt, zoom, shutter, strobe, hazer output, and other patched DMX parameters |

The API control plane never supplies live pan, tilt, intensity, color, or effect values to this
MVP. It supplies enough patch and fixture-mode information for the visualizer to decode received
DMX slots. Pan and tilt are therefore visualized, but only when their values arrive in Art-Net or
sACN; stage position and mounting rotation remain API-owned physical configuration.

### Future planning-software provider

The future local Viz planning software provides:

- the same versioned semantic scene snapshot, entity deltas, asset references, view configuration,
  and monotonic revisions as the desk provider;
- planner-owned fixture placement, model, scenery, and document changes;
- local preview values through the canonical engine-neutral renderer protocol, including
  `UpdateFixtureValuesBatch`, rather than requiring the planner to emit Art-Net or sACN; and
- provider capability/version negotiation, heartbeat, full-snapshot recovery, and diagnostics.

The planning software normally connects over a local loopback transport, but its server and port
remain configurable in **Quick Settings**. The renderer never opens the planner's database or
becomes its document authority. The planner provider may optionally exercise real Art-Net/sACN as
an integration mode, but network DMX is not a mandatory dependency of offline planning.

The provider boundary exposes semantic operations such as loading a snapshot, applying entity
deltas, updating fixture values, applying view configuration, resolving an asset, reporting
health, and requesting resynchronization. Provider-specific URLs, sockets, authentication, packet
decoding, and reconnect behavior stay outside the render core.

Switching source provider:

1. leaves the current valid scene visible while the candidate provider connects;
2. negotiates and validates a complete candidate snapshot;
3. atomically replaces the scene only after that snapshot is valid;
4. clears values, deltas, assets, revisions, and diagnostics owned only by the previous provider;
5. starts accepting only the new provider's revisions and live-value source; and
6. never merges a lighting-desk scene with a planning-software scene.

The visualizer is a read-only projection:

- The selected lighting desk or planning document remains authoritative for its scene.
- The fixture library remains authoritative for mode channels, ranges, fine bytes, splits,
  defaults, logical heads, physical metadata, and model assets.
- For the lighting-desk provider, the output route maps a logical show universe to a protocol
  destination universe.
- The visualizer owns only GPU resources, decoded latest input frames, camera interaction that has
  not been persisted by the selected provider, and diagnostic state.
- The visualizer never writes the upstream show/planning database or creates a parallel fixture
  schema.

The current server exposes much of the required initial data through `GET /api/v2/patch`, immutable
profile snapshots, revisioned fixture packages, `show_patch_changed` events, and the stored
`stage_layout` object. Before implementation, the server must reconcile the patch location and
`stage_layout/main.positions3d` into one documented effective transform per physical instance. The
renderer must not guess between two transform authorities.

Unpatched fixtures remain part of the synchronized scene. They retain their API-supplied placement
and model but cannot react to network DMX until patched. Visual-only `Venue` objects remain visible
without a DMX address. Multi-patch physical instances share their logical fixture values while
retaining independent physical transforms. Multi-head fixtures retain head identity and decode
head-specific channels.

## Application and workspace architecture

The MVP belongs in the existing Light workspace:

- the standalone executable belongs under `apps/viz-renderer`;
- reusable renderer, scene projection, fixture decoding, and graphics behavior belong under
  focused `crates/viz` crates as the canonical plan defines;
- shared wire types are generated from `crates/light/contracts/wire`; and
- existing fixture, output-protocol, model-loading, and UI code is reused where its ownership
  permits rather than copied.

The independently runnable application owns:

- source-provider selection, discovery/configuration, and authentication;
- the active desk API or future planner-protocol connection;
- Art-Net and sACN receiver sockets;
- scene synchronization and revision recovery;
- the render window and camera;
- visible status and diagnostics; and
- structured logs under the canonical `.artifacts` runtime paths during development.

The render core accepts a presentation-surface adapter. The MVP supplies a standalone native
window. A future main-application adapter may instantiate the same core in process and present it
inside the Stage viewport. That future host must own its own renderer instance; it must not embed
the standalone renderer process or consume a rendered-video stream.

The visualizer has its own build, test, package, and launch commands. Building the main ToskLight
desktop must not build or package the visualizer unless a release workflow explicitly asks for the
complete suite. Visualizer failures must not affect the server, output engine, or desk UI.

## Startup and operator flow

1. Launch the visualizer directly.
2. It attempts the default desk at `http://127.0.0.1:5000`.
3. It checks readiness, authenticates through the existing desk mechanism, negotiates compatible
   visualization capabilities, and reports each state visibly.
4. It reads one coherent scene/configuration snapshot for the loaded show.
5. It validates referenced fixture modes, assets, physical instances, output routes, and input
   mappings before replacing any already displayed scene.
6. It opens the required Art-Net and/or sACN receivers.
7. It subscribes to revisioned scene and visualizer-configuration events.
8. It displays the scene even before DMX arrives, using fixture defaults and an explicit
   **Waiting for DMX** state.
9. On the first accepted UDP frame, it decodes changed slots and updates only affected fixture
   parameters and render instances.
10. It continues to accept camera/view/render-mode messages from the desk while local camera
    interaction remains responsive.

### Quick Settings

**Command+,** on macOS and **Ctrl+,** on Windows/Linux open **Quick Settings** without leaving or
restarting the renderer. The same command appears in the application menu so the workflow is
discoverable without knowing the shortcut.

The menu contains:

- **Source:** **Lighting desk** or **Planning software**;
- **Server:** hostname or IP address;
- **Port:** API/control-protocol port, validated as `1..=65535`;
- authentication/session controls required by the selected source;
- network interface and Art-Net/sACN input summary for the lighting-desk provider;
- **Rendering quality:** **Follow source** plus the supported explicit quality/preset choices;
- **Fog amount:** `0–100%`, the renderer's own haze;
- current connection, scene revision, input health, and renderer status; and
- **Connect/Reconnect** plus an explicit way to cancel without replacing the current valid
  connection.

The source control is capability-driven. In the initial desk-only build it may show
**Planning software — Not available in this build** rather than pretending that the future
provider can connect. Once packaged, the same control enables it without changing the renderer
core or the rest of this menu.

The lighting-desk defaults are server `127.0.0.1` and port `5000`. The planning-software provider
uses its eventual documented local default, not the desk port by assumption. Changing server,
port, or source stages a new connection and uses the atomic provider-switch behavior above.
Changing rendering quality or the fog amount applies immediately without reconnecting or
rebuilding the semantic scene.

An explicit local rendering-quality choice overrides quality messages from the selected source.
**Follow source** accepts the current desk/planner quality setting. This makes the quick setting
useful during a live session without removing remote configuration.

The connection surface must expose at least:

- selected provider, configured endpoint, and resolved address;
- authenticated/unauthenticated state;
- active show/planning-document identity and scene revision;
- selected network interface where the provider uses network input;
- each configured input's protocol, logical universe, destination universe, delivery mode, and
  health;
- last-packet age, sequence/source information, and dropped/out-of-order counts;
- model/profile fallback warnings; and
- renderer backend, quality mode, frame rate, and input-to-visible latency.

There must be no silent indefinite spinner. A server, authentication, socket-bind, multicast-join,
asset, schema, or GPU failure names the failing boundary and offers retry or configuration where
the operator can act.

The server connection uses a read-only visualizer client/session role. Connecting a renderer must
not create an operator programmer, change desk selection, claim a command line, or otherwise
acquire live-control state merely because the current general-purpose session endpoint does so.

## Desk API contract

Implementation must follow [`docs/engineering/api-rules.md`](../../../engineering/api-rules.md):
whole-state reads are snapshots, volatile changes are pushed rather than polled, bodies use typed
wire contracts, unknown fields are tolerated and logged, and active-show races use the existing
show guard and revision rules.

The exact route grouping may reuse current coherent read models or add a dedicated versioned Viz
projection. It must provide the following semantics without requiring the visualizer to assemble a
racy scene from unrelated requests.

Current seams to reuse or adapt include:

- `GET /api/v2/readiness` for server readiness;
- `GET /api/v2/patch` for patched fixtures, physical instances, logical heads, transforms,
  selected immutable profile/mode snapshots, emitters, geometry, channels, and patch addresses;
- `/api/v2/events` and its `show_patch_changed`/show-object events for revisioned changes; and
- `/api/v2/fixture-library/profiles/{id}/revisions/{revision}/package` for revision-pinned model
  and fixture assets.

These routes are implementation evidence, not permission to make the renderer coordinate several
independent revisions itself. Add one renderer-facing effective-scene projection if the current
snapshots cannot provide an atomic scene.

### Scene snapshot

One snapshot contains:

- protocol/schema version, server identity, active show identity, and monotonic scene revision;
- fixture UUID/ID, display name, manufacturer/fixture/mode identity, fixture-library revision, and
  model/icon/physical references;
- logical and physical instance identity, including multi-patch and logical-head relationships;
- patch information sufficient to map every used DMX coarse/fine slot and destination universe;
- complete mode channel/function/range/default information needed to decode raw DMX;
- metric stage position, three-axis mounting rotation, scale/model variant, and attachment
  projection;
- visual-only scenery and venue entities needed to show beam impact and occlusion;
- current visualizer view configuration;
- configured visualizer input mappings derived from or associated with output routes; and
- content-addressed or revisioned asset references with bounded sizes and explicit fallback data.

The MVP may use existing object snapshots internally, but the synchronization boundary must yield
one declared scene revision. It must not present fixture transforms from one show with patch or
fixture definitions from another.

### Revisioned changes

After the snapshot, the server pushes typed, batched changes. Message families cover:

- entity create, update, and delete;
- stage position, mounting rotation, scale, attachment, or model change;
- fixture-library reference, mode, patch, physical-instance, or logical-head change;
- scenery/model/asset change;
- visualizer input-mapping change;
- camera, view preset, rendering mode, and quality-setting change;
- active-show change; and
- resynchronization required.

Entity updates carry only changed semantic fields plus entity and scene revisions. They never send
renderer-library object paths or GPU resource identifiers. A gap, stale revision, incompatible
schema, changed active show, or reconnect causes a fresh full snapshot before later deltas are
accepted.

The event connection is not used as an alternative DMX transport. Frequent output values must
never be projected, serialized, or sent through this scene/configuration subscription.

### Desk-owned visualizer configuration

The main desk can configure the visualizer by updating a desk-owned visualizer-view object through
normal object-intent actions. The server persists the accepted state and publishes the
authoritative change to connected visualizers. At minimum the typed state contains:

- camera position;
- camera rotation/orientation;
- named view mode;
- orthographic size or 3D field of view as applicable;
- rendering quality/preset;
- exposure and operator-safe brightness controls;
- visible layer/filter state reserved for later extension;
- selected input mappings and network interface; and
- a stable visualizer instance/target selector so one desk can address one renderer or an explicit
  renderer group without accidentally moving every connected camera;
- revision and request identity.

The visualizer does not expose its own REST API. If the operator moves the camera directly in the
visualizer, the MVP may keep that move local until the desk sends another authoritative view
message. Bidirectional camera persistence is deferred unless it is later specified as a typed
desk object-intent update with conflict behavior.

## Art-Net and sACN input

### Input mapping

The API supplies explicit input descriptors rather than asking the visualizer to guess from every
packet on the network. Each descriptor includes:

- stable mapping identity and priority;
- logical show universe;
- protocol: `art_net` or `sacn`;
- destination universe using that protocol's numbering;
- delivery mode: Art-Net Broadcast/Unicast or sACN Multicast/Unicast;
- bind interface/address and destination port;
- expected desk/source identity where the protocol can express it; and
- enabled state.

The renderer decodes destination-universe packets into the corresponding logical-universe frame,
then uses the API patch/mode data to update fixtures. Art-Net numbering conversions and sACN
universe rules remain in the shared protocol implementation and are not duplicated in renderer
UI code.

Both protocols may be active at once. If more than one healthy mapping carries the same logical
universe, a stable API-configured priority selects one authoritative mapping. The renderer does not
HTP/LTP merge duplicate Art-Net and sACN copies and does not apply the same logical frame twice.
Failover is explicit, observable, and returns to the higher-priority source only according to a
documented anti-flap rule.

### Protocol behavior

Art-Net input must:

- validate genuine ArtDMX packets, lengths, protocol version, universe, and sequence behavior;
- support the desk's Broadcast and Unicast delivery modes;
- reject malformed and unrelated OpCodes without affecting the current scene;
- distinguish configured sources where source IP is available; and
- tolerate sequence wrap while rejecting provably stale frames.

sACN input must:

- validate the E1.31 root, framing, DMP, universe, slot count, sequence, and source fields;
- support the desk's Multicast and Unicast delivery modes;
- join and leave multicast groups on the explicitly selected interface;
- implement standard source priority, source loss, stream termination, and sequence behavior; and
- reject malformed or unrelated packets without affecting other universes.

UDP receive and decode must be bounded and non-blocking relative to rendering. Bursts are
coalesced to the newest complete frame per logical universe. Packet loss never backpressures or
changes ToskLight output. A renderer slowdown drops obsolete render work rather than allowing an
unbounded queue.

On source loss, the visualizer follows protocol timeout/termination semantics, visibly marks the
affected universes stale, and transitions them to fixture defaults/zero output according to the
shared decoder contract. It must not conceal source loss by fetching live values from the API.

### Same-computer networking

Same-host operation is a release gate for both protocols:

- ToskLight sends Art-Net through a real UDP output route.
- The visualizer receives it through a real Art-Net socket on the same computer.
- ToskLight sends sACN through a real UDP output route.
- The visualizer receives it through a real sACN socket on the same computer.
- No test may substitute direct logical frames, the visualization WebSocket, or an in-process
  shortcut.

The portable default is explicit loopback Unicast:

- Art-Net Unicast to `127.0.0.1` and the configured Art-Net port.
- sACN Unicast to `127.0.0.1` and the configured sACN port.

This still traverses the operating system network stack and proves the exact packet encoders and
receivers. It does not depend on whether a platform reflects global broadcast or multicast packets
back to a local receiver. Broadcast and multicast remain supported for normal network operation
and receive separate same-host tests where the operating system supports loopback delivery.

The implementation must explicitly handle socket address reuse, interface selection, multicast
loopback membership, firewall guidance, and collisions with another local receiver. If a selected
delivery mode cannot return to the sender on that platform, the UI explains the problem and offers
the loopback-Unicast mapping; it must not fail silently.

## DMX-to-fixture projection

The projection layer reuses ToskLight fixture semantics. It must understand:

- coarse and fine bytes in their defined order;
- channel defaults and Highlight-independent normal output;
- mode functions, ranges, splits, inversion, and physical-value conversion;
- fixture-level and logical-head attributes;
- intensity/dimmer and shutter;
- additive and subtractive color systems, fixed color wheels, and conventional gel color;
- pan and tilt, including mounting orientation and configured axis compensation;
- zoom/beam angle, iris, focus, frost, gobo, prism, and rotation where metadata exists;
- strobe and other bounded temporal functions;
- pixel/cell intensity and color;
- hazer/fog output; and
- safe generic behavior for an unknown or incomplete optical attribute.

Only fixtures affected by changed slots are re-decoded. Only changed semantic parameters are sent
to the render scene. A camera move or render-mode change never causes DMX to be reinterpreted.

An incomplete profile or missing model produces a visible generic fixture with a diagnostic.
Unsupported advanced optics may fall back to the closest safe generic emitter, but a
light-producing head must not disappear.

## Fixture and emitter rendering

### Beam-producing fixtures

Every light-producing head has a source transform, emission direction, intensity, color, beam
shape/angle, and optional optical modifiers. The generic renderer covers at least:

- conventional dimmers and profiles;
- static Fresnel, PAR, flood, cyc, and wash fixtures;
- moving profile, spot, beam, and wash fixtures;
- LED and discharge sources;
- multi-cell and multi-head fixtures;
- strobes and blinders;
- follow spots and other aimable fixtures; and
- laser or other narrow-source profiles through an explicit safe generic representation when a
  dedicated optical simulation is not yet available.

The line mode can represent an aim vector without claiming photometric accuracy. Simple and full
modes render an emissive aperture, illuminated surface contribution, and—when atmosphere is
present—a visible beam volume. Surfaces and scenery participate in beam impact and occlusion
according to the selected quality tier.

### Pixel-only emitters

A fixture/profile/head can declare or derive that it is emissive-only. Existing fixture geometry
already distinguishes point/ring/strip/matrix/explicit-pixel emitter layouts and whether an emitter
is directional. Examples include decorative pixels or face-visible lamps that are not intended to
project a stage beam.

- It renders an emissive point, cell, strip, or surface using its decoded intensity and color.
- It contributes local glow/bloom in modes that support those effects.
- It does not create an aim line, cone, volumetric shaft, or invented surface footprint.
- Multi-pixel fixtures preserve cell identity and spatial order.

Directional emitters receive an aim/beam representation. Non-directional emitters receive only
emissive-source rendering. This capability is based on fixture/profile metadata or a documented
fallback rule, not a hardcoded manufacturer/name list.

### Hazers and atmosphere

Atmosphere is the renderer's own **Fog amount**, `0–100%`, and nothing else. A hazer's DMX output
describes how hard a machine is working, not the density the room ends up with, so the renderer
never derives haze from patched hazers, planner environment haze, or any source configuration.
The amount is honoured exactly: `0%` is clear air while a patched hazer runs at full, and the
default is `50%` so beams are visible before the operator touches anything.

The amount is a renderer-local preference, persisted independently from the show or planning
document. It is editable in **Quick Settings**, by rotating the wheel over the fog readout, and
from `--fog <pct>` at startup, and applies immediately without reconnecting.

Hazers remain part of the scene: they are patched, addressed, and drawn as atmosphere machines,
and their fog output is decoded as the emitter's own value. Only the participating-medium density
is out of their hands.

The MVP may use one global or simply zoned density field. It does not need fluid movement. With
zero haze, the operator sees emissive fixture apertures and the places where light hits geometry,
but not a bright beam floating through clear air. As haze increases, more of each beam becomes
visible. Light outside a beam does not reveal generic screen-filling fog.

An emitter-local fog plume whose size and opacity follow hazer fog output is a stretch goal. Add it
only after the beam-density behavior is correct and measured; its absence does not block the MVP.
If implemented, a fixture-local plume can continue to follow that fixture's received/simulated fog
output while the **Fog amount** controls the global beam atmosphere.

## Views, camera, and rendering modes

The source-visible names and wire enum values must map one-to-one. The MVP supports:

| Mode | Projection and direction | Required output |
| --- | --- | --- |
| **Top Down** | Orthographic top-down | Clear stage plan, fixture bodies, orientation, and light-state indication |
| **Left → Right** | Orthographic elevation looking from stage left toward stage right | Spatially correct elevation |
| **Right → Left** | Orthographic elevation looking from stage right toward stage left | Spatially correct elevation |
| **Front → Back** | Orthographic elevation looking from the audience/front toward the back | Spatially correct elevation |
| **Back → Front** | Orthographic elevation looking from the back toward the audience/front | Spatially correct elevation |
| **3D Lines** | Perspective or orthographic 3D camera | Models or proxies plus direction/aim lines; no expensive beam volume required |
| **3D Simple** | Perspective 3D camera | Fast previsualization with fixture emission, color, surface hits, simple beams, and basic haze |
| **3D Full** | Perspective 3D camera | Best available PBR materials, shadows, occlusion, volumetric beams/haze, bloom, and supported optics |

All directions are defined against ToskLight's existing stage coordinate and front/back convention;
the visualizer must not invent another axis system. Orthographic presets calculate a deterministic
camera from scene bounds and then accept configured camera position/rotation/size. Switching modes
does not mutate fixture or stage data.

### Operator navigation

The camera is driven directly, in every mode:

| Input | Action |
| --- | --- |
| `1`–`8` | 3D, Top Down, Front → Back, Left → Right, Right → Left, 3D Simple, Back → Front, 3D Lines |
| Right drag | Pan and tilt the camera on the spot; pan across the floor in a plan view |
| `Shift`+right drag | Pan the view across the stage floor |
| Middle drag | Move the camera on the camera plane |
| Wheel | Zoom |
| Two-axis scrolling | Pan and tilt the camera, exactly as a right drag does |
| `Cmd`/`Ctrl`+two-axis scrolling | Zoom, for a machine with no notched wheel |
| `W` `A` `S` `D` | Walk the camera on the floor plane, facing where the camera points |
| `Enter` | Quick Settings |
| `Space` | Hide and show every overlay |

An orthographic view keeps its exact axis whatever the operator does; nothing may turn a plan into
a slightly crooked plan, so the turning drag pans there instead. Turning keeps the camera where it
is and swings its aim, following the hand. Panning and walking are parallel to the floor and never
change height. A drag covers more ground the further the view is zoomed out.

A notched wheel zooms; continuous two-axis scrolling turns the camera. That separation is what
makes the visualizer usable under a mouse utility that has claimed the right button for its own
panning and delivers that drag to applications as scrolling — the right-button gesture then still
turns the camera instead of zooming the picture.

The left button is not a camera control. It belongs to the status surface, so that a later
selection or inspection gesture has a button to be given.

### The plan views as a stage plot

The orthographic views draw outlines only: no shading and no bloom, fixture bodies as symbols for
their body kind, a lit fixture in the colour it is emitting, and the aim lines. Symbols keep a
constant size on screen so a wide plan stays readable. Fixture number and patch address are drawn
beside each symbol, and a label that would collide with one already placed is dropped rather than
overprinted. Both a light-on-dark and a dark-on-light appearance are supported; dark on light is
the printable stage plot.

### The status surface

The application mark sits in the top left and opens Quick Settings. The bottom left carries one
badge per universe with that universe's own frame rate — green while it is clean, orange after a
rate drop or single broken frames inside the last thirty seconds, red once the rate falls below
20 Hz or more than 20% of one second's frames were broken inside that window — with the render
latency beside them. The bottom right carries the fixture, head, and live-beam counts, the surface
in view (`2D <view>` or `3D <view> <quality>`), the frame rate, the fog, the exposure trim, and the
ambient level. Fog, exposure, and ambient are adjusted by rotating the wheel over their readouts.

Haze renders at the renderer's own fog amount, 50% until the operator changes it, whether or not
the show patches a hazer. Ambient light decides how bright everything that
is not a light source is, held at a constant screen brightness so a rig full of beams cannot pull
the trusses into the dark with the automatic exposure.

### Beam optics and shadows

Zoom narrows the cone and brightens it; an iris narrows it and leaves the brightness alone; focus
is sharp mid-travel and soft either side; frost widens the field and destroys the edge. Gobo,
gobo rotation, prism, prism rotation and the four framing blades are evaluated in the beam's own
gate, so a pattern appears on the surface it lands on and in the shaft of haze at the same time
and from the same geometry.

`3D Full` renders shadow maps for the brightest beams within a per-tier budget. Both the surface
pass and the volumetric pass sample them, so geometry standing in a beam breaks the shaft as well
as the pool. A light with no map is drawn unshadowed rather than dark.

Fixture models come from the profile snapshot the desk already sends: a package stores its GLB
inline, so no separate asset fetch exists. Node names bind the moving parts. A model that cannot
be read leaves the fixture on its procedural proxy with a named reason.

Camera interaction remains smooth while DMX arrives. `3D Full` can reduce expensive shadow or
volumetric allocations under load, but it must expose the active degradation rather than silently
switching to another named mode.

The wire format represents orientation without Euler-order ambiguity, for example as
position/target/up or a normalized quaternion plus declared coordinate convention. The public
contract must not send renderer-library camera objects.

## Scene synchronization and rendering lifecycle

- Build and validate a complete candidate scene before replacing the displayed scene.
- Apply revisioned entity changes incrementally; a fixture move must not rebuild the full scene.
- Keep stable entity and GPU-instance identity across patch, value, and transform updates where
  the model remains compatible.
- Cache models, materials, decoded fixture projections, and pipeline state by content revision.
- Batch compatible fixtures and pixel cells while preserving picking/diagnostic identity.
- Coalesce high-rate DMX to the newest state at the render boundary.
- Keep camera/input processing independent from scene synchronization and DMX decode.
- Release removed-show assets after a bounded grace/cache policy.
- Recreate GPU resources or request a full snapshot after recoverable renderer failure.

The render loop may be continuous while active DMX or camera motion requires it and demand-driven
when the scene is unchanged.

## Performance and isolation gates

Phase 0 records reference hardware before final numeric claims are marketed. The MVP nevertheless
has the following engineering gates:

- no network receiver, decoder, API task, or renderer path can block or backpressure the desk;
- no unbounded packet, scene-delta, asset, or render-command queue;
- stable camera interaction at the display refresh target while live packets arrive;
- no full-scene rebuild for a DMX frame or one fixture-position change;
- packet-to-visible latency measured from decoded receive timestamp to presented frame;
- per-protocol packet loss, out-of-order, decode, coalescing, and source-switch metrics;
- CPU frame, GPU frame, memory, asset load, and scene synchronization metrics;
- no sustained memory growth during a one-hour changing-look soak; and
- quality degradation is bounded, observable, and limited to rendering.

The product-demo show and its approximately 301 physical Stage instances are the first
representative acceptance scene. Measure every named mode with realistic moving, color, pixel, and
hazer activity. Record p50/p95/max packet-to-visible latency and frame pacing rather than reporting
average FPS alone. The implementation phase must set minimum and recommended GPU targets from
these measurements before declaring the MVP complete.

## Failure and recovery behavior

- API unavailable at launch: retain the connection screen and retry with bounded backoff.
- API disconnect after a scene is loaded: retain the last scene, mark configuration stale, keep
  network input visible only while its fixture mapping remains known, and request a full snapshot
  on reconnect.
- Active-show change: stop applying old mapping deltas, stage the new snapshot, clear incompatible
  DMX mappings, then atomically replace the scene.
- Scene revision gap: stop applying later deltas and resynchronize.
- DMX absent: show the scene at defaults/zero output with a visible waiting or stale state.
- One universe absent: mark only affected fixtures/universe stale.
- Malformed packet: count and discard it without changing the last valid frame.
- Asset missing or invalid: render a bounded generic proxy and name the missing reference.
- Fixture definition incomplete: use a generic emitter or emissive-only fallback and name the
  unsupported semantics.
- GPU device/surface loss: attempt bounded recreation and retain connection diagnostics.
- Unsupported protocol/schema version: show both versions and refuse misleading partial output.

Reconnect never replays queued scene mutations or fabricates live values. A fresh authoritative
snapshot plus newly received values from the selected provider is the recovery source. For the
lighting-desk provider, those values are newly received Art-Net/sACN frames.

## Delivery phases

### Phase 0 — contracts and network/render spike

- Confirm the existing fixture, stage, patch, asset, output-route, authentication, and event seams.
- Define the renderer-owned provider interface and prove it with a lighting-desk adapter plus a
  deterministic fake planning-software adapter.
- Define typed Viz scene snapshot, scene delta, view configuration, and input-mapping contracts.
- Confirm the existing fixture-profile emitter metadata and define only the missing fallback or
  atmosphere-outlet semantics without creating a second schema.
- Define the read-only visualizer session role and targeted-renderer identity.
- Prove one standalone `wgpu` window on macOS, Windows, and supported Linux systems.
- Receive and decode one real Art-Net universe and one real sACN universe.
- Prove Art-Net Unicast and sACN Unicast from ToskLight to the visualizer on the same computer.
- Render one static dimmer, one moving color fixture, one pixel-only emitter, and one beam
  through the renderer's haze.
- Record packet-to-visible timing, GPU timing, and socket/interface behavior.

Gate: the two-plane architecture, same-host UDP path, renderer choice, and fixture-decoding reuse
have no unresolved blocker.

### Phase 1 — coherent scene and incremental configuration

- Connect to `127.0.0.1:5000` by default and support a remote desk address.
- Add **Quick Settings**, its cross-platform shortcut, source/server/port configuration, staged
  reconnect, rendering-quality override, and fog-amount controls.
- Load one coherent scene snapshot and validate it before display.
- Resolve real fixture models with generic fallbacks.
- Subscribe to revisioned entity, patch, mode, transform, asset, view, input, and show changes.
- Apply create/update/delete and fixture-position changes without a full-scene rebuild.
- Add visible connection, asset, profile, scene revision, and receiver diagnostics.

Gate: fixture/scenery/model placement follows API state and only configuration changes arrive
through the API.

### Phase 2 — complete live fixture projection

- Implement bounded Art-Net and sACN receivers and source managers.
- Decode fine bytes, ranges, splits, logical heads, color, movement, beam, pixel, and hazer
  semantics from shared fixture definitions.
- Support simultaneous configured routes and deterministic duplicate-universe failover.
- Add source loss, termination, stale mapping, malformed packet, and reconnect handling.
- Exercise conventional, moving, multi-head, multi-cell, pixel-only, and hazer profiles.

Gate: every light-producing fixture head in the representative scene renders or has an explicit
generic fallback; no live value is read from the desk API.

### Phase 3 — all named views and quality tiers

- Implement the five named orthographic directions.
- Implement **3D Lines**, **3D Simple**, and **3D Full**.
- Add camera position/rotation/configuration messages from the desk.
- Add surface hits, occlusion, shadows, simple/full haze, bloom, and supported optical modifiers
  according to quality tier.
- Add bounded adaptive quality with visible diagnostics.
- Apply the renderer's fog amount in every beam-rendering quality.
- Evaluate the optional hazer plume after the required atmosphere behavior passes.

Gate: every named mode is remotely selectable and visually distinguishable, camera changes do not
interrupt DMX, and zero-haze versus active-haze behavior is correct.

### Phase 4 — hardening and independent delivery

- Run cross-platform socket, renderer, packaging, restart, and long-soak coverage.
- Verify same-host and two-computer workflows for both protocols.
- Verify independent main-desktop and visualizer builds.
- Publish minimum/recommended hardware from benchmark evidence.
- Document firewall, interface, route, and loopback setup.
- Package the visualizer independently without changing the main desktop's runtime dependency.

Gate: all MVP acceptance criteria below pass on macOS, Windows, and the supported Linux baseline.

## Acceptance scenarios

### VIZ-MVP-001 — default local connection

With ToskLight server running on the same computer, launching the visualizer without arguments
connects to `http://127.0.0.1:5000`, authenticates, loads the active show scene, and displays
fixture/scenery placement. If authentication requires operator action, that action is explicit and
the attempted address remains visible.

### VIZ-MVP-002 — remote desk connection

Given a desk on another lighting-network host, entering its address loads the same scene and
reports the remote server/show identity. Losing and restoring the network performs a full
revision-safe resynchronization without replaying stale changes.

### VIZ-MVP-003 — API configuration only

Move one fixture, rotate one physical instance, change one model/mode, add and remove one scenery
object, and switch the active show. The visualizer receives typed API snapshot/delta data and
updates only affected scene entities. No DMX packet is required for those configuration changes.

### VIZ-MVP-004 — no live values through the API

With the scene/API connection healthy but no UDP input, fixtures remain at default/zero output and
the app says **Waiting for DMX**. Changing the desk's programmer or playback without a configured
network output route does not animate the visualizer. The visualizer never requests the desk
output-visualization endpoint.

### VIZ-MVP-005 — same-host Art-Net

Configure a real Art-Net Unicast output route to the loopback receiver. From the desk, change
intensity, color, pan, tilt, zoom, and shutter. Decode the emitted ArtDMX packets and show the
corresponding fixture changes. Capture wire-level packet evidence; direct logical-frame injection
does not satisfy this scenario.

### VIZ-MVP-006 — same-host sACN

Repeat VIZ-MVP-005 through a real sACN Unicast output route to the loopback receiver. Validate
E1.31 source, universe, priority, sequence, slot data, termination, and receiver recovery with
wire-level evidence.

### VIZ-MVP-007 — simultaneous protocols and route mapping

Receive different logical universes through Art-Net and sACN at the same time. Both sets of
fixtures respond. Then configure duplicate mappings for one logical universe and prove the
declared priority/failover behavior without double-applying or merging the two copies.

### VIZ-MVP-008 — fixture coverage and fallbacks

Load a representative show containing conventional, profile/spot, wash, beam, PAR/Fresnel/flood,
moving, multi-head, multi-cell, pixel-only, blinder/strobe, follow-spot, and hazer fixtures. Every
light-producing head is visible and responds to the supported semantics. Incomplete optics/model
data produces a named generic fallback, never an invisible fixture or renderer crash.

### VIZ-MVP-009 — pixel-only light

Raise and color a pixel-only fixture through received DMX. Its emissive cells and optional glow
change, but no aim line, cone, volumetric shaft, or surface beam footprint appears.

### VIZ-MVP-010 — haze and beam visibility

Aim an active fixture at scenery. At `0%` **Fog amount**, its aperture and surface hit are visible
without a bright air beam. Raise the fog amount and verify that beam visibility increases
monotonically through the bounded density mapping. Return to `0%` and verify the beam volume
clears. Drive a patched hazer's `fog` output across its full range and verify that the atmosphere
does not move. If implemented, the optional plume still follows that fixture's fog output.

### VIZ-MVP-011 — remote view control

From the desk, select **Top Down**, **Left → Right**, **Right → Left**, **Front → Back**,
**Back → Front**, **3D Lines**, **3D Simple**, and **3D Full** in turn. The visualizer receives
authoritative typed configuration messages and presents the exact requested direction/mode.
Camera position and rotation changes update the view without changing scene placement or
interrupting UDP reception.

### VIZ-MVP-012 — stale and malformed input

Send out-of-order, truncated, wrong-universe, and malformed packets, then stop one valid source.
Invalid packets do not mutate the scene. Diagnostics identify the affected mapping. Protocol
timeout/termination marks only affected fixtures stale and applies the declared fallback. A new
valid source or restored source recovers without an application restart.

### VIZ-MVP-013 — independent failure boundary

Build, launch, crash, restart, and uninstall the visualizer independently. The ToskLight desktop
and server continue operating and sending DMX throughout. Building or launching the main desktop
does not require the visualizer binary.

### VIZ-MVP-014 — representative performance

Run the product-demo show with approximately 301 physical Stage instances, changing movement,
color, pixels, intensity, and hazer values over real network output. Record frame pacing,
packet-to-visible latency, coalescing, receiver loss, CPU/GPU time, memory, and active quality
degradation for every named view. No queue grows without bound, camera input remains responsive,
and one fixture-position delta does not rebuild the full scene.

### VIZ-MVP-015 — Quick Settings

Open **Quick Settings** with **Command+,** on macOS and **Ctrl+,** on Windows/Linux. Change the
lighting-desk server and port, cancel once without disturbing the current connection, then connect
to a valid second server. The old scene remains visible until the replacement snapshot validates.
Change rendering quality without reconnecting. Select **Follow source** and verify that later
desk-owned quality messages apply; select an explicit local quality and verify that those messages
no longer replace it.

### VIZ-MVP-016 — renderer-owned fog amount

With no hazer in the scene, move **Fog amount** from `0%` to a visible value; active light beams
use that density. Load a scene with a hazer, drive its `fog` output to a different value, and
verify that the rendered atmosphere stays on the operator's amount. Set the amount to `0%` and
verify that the air remains clear despite active hazer DMX. Restart/reconnect and prove the
renderer-local amount does not alter the show or planning document.

### VIZ-MVP-017 — provider-independent render core

Run the same deterministic snapshot, entity delta, fixture-value batch, view change, and full
resynchronization through the lighting-desk adapter and a fake local planning-software adapter.
Both produce the same semantic renderer scene and fixed-camera image. Provider-specific HTTP,
WebSocket, UDP, and local-protocol types do not cross into the render core. Switching adapters
never merges scenes or accepts late values from the previous provider.

## Future planning-provider acceptance

### VIZ-PLANNER-001 — local planning-software connection

Select **Planning software** in **Quick Settings**, enter its local server and port, and connect
without running a ToskLight desk or configuring Art-Net/sACN. The renderer loads the planner's
complete scene, receives fixture/model/position/scenery deltas and local preview-value batches,
uses the same camera, rendering-quality, and fog-amount behavior as the lighting-desk provider,
recovers from a revision gap with a full snapshot, and never opens or writes the planning
document's database directly.

## Verification evidence required at implementation closeout

- Typed wire schemas and generated-client drift checks.
- Fixture decoder unit/property tests for channels, fine bytes, ranges, splits, heads, pixels, and
  hazers.
- Art-Net and sACN decoder conformance and malformed-packet tests.
- Real same-host UDP captures for both protocols.
- Real two-host network captures for both protocols and their supported normal delivery modes.
- API snapshot, revision-gap, reconnect, active-show switch, and stale-guard coverage.
- Fixed-camera golden images for every named view and quality mode.
- Zero-haze, active-haze, pixel-only, generic-fallback, surface-hit, and occlusion image cases.
- GPU loss, asset failure, server disconnect, DMX loss, and renderer restart recovery tests.
- Cross-platform independent build/package evidence.
- Product-demo benchmark artifacts and one-hour memory/queue soak evidence.
- Provider-boundary tests, Quick Settings shortcut/connection tests, and fog-amount coverage.
- Manual operator review of exact view names, directions, camera behavior, diagnostics, and
  same-computer setup.

## Deferred follow-up

After this MVP is complete:

- implement and connect the full Viz planner/editor provider and offline show-document workflow
  through the already-proven provider boundary;
- decide whether and how local visualizer camera changes become persisted desk intent;
- add advanced manufacturer optics, photometric data, gobos/prisms/animation, and richer shadows;
- add zoned or local atmosphere and the optional hazer plume if it was not economical for MVP;
- add recording and offline high-quality output;
- prove the separate in-process host adapter that can replace only the existing Stage viewport's
  pixels; and
- schedule replacement of the Stage renderer only after its independent latency, preload, input,
  window-management, and DMX-isolation acceptance contract is met.

The future Stage integration reuses the renderer core and semantic scene contracts. It does not
turn the main application into a client of this standalone process and does not make live Stage
feedback depend on the visualizer application being installed or running.

## Implementation evidence

Measured on the product-demo show — 262 logical fixtures, **301 physical Stage instances**, 383
light-producing heads — with 13 playbacks and their dynamics running, output over real Art-Net
(universes 1–4) and real sACN (universes 5–8) to loopback receivers, on an Apple M5 Max. Six
seconds per named view, 2880 presented frames.

| View | FPS | Frame p50 | Frame p95 | Latency p50 | Latency p95 | Latency max | DMX | Live beams |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Top Down | 60.0 | 16.68 ms | 17.48 ms | 24.0 ms | 34.4 ms | 40.6 ms | 54.3 Hz | 147 |
| Left → Right | 59.9 | 16.69 ms | 17.93 ms | 22.8 ms | 32.8 ms | 34.9 ms | 58.2 Hz | 144 |
| Right → Left | 60.0 | 16.69 ms | 18.52 ms | 23.7 ms | 33.7 ms | 37.4 ms | 56.4 Hz | 142 |
| Front → Back | 60.0 | 16.70 ms | 18.02 ms | 22.7 ms | 33.2 ms | 37.1 ms | 56.8 Hz | 145 |
| Back → Front | 60.0 | 16.69 ms | 17.38 ms | 23.9 ms | 33.6 ms | 35.6 ms | 56.9 Hz | 139 |
| 3D Lines | 60.0 | 16.69 ms | 17.73 ms | 23.3 ms | 34.2 ms | 43.8 ms | 55.6 Hz | 136 |
| 3D Simple | 60.0 | 16.70 ms | 17.73 ms | 23.1 ms | 32.9 ms | 38.5 ms | 58.5 Hz | 141 |
| 3D Full | 59.8 | 16.74 ms | 18.15 ms | 23.9 ms | 33.8 ms | 37.1 ms | 54.9 Hz | 142 |

Latency is measured from the receive timestamp of the newest accepted packet to the presented
frame, so it includes decoding, the scene update, GPU work, and presentation. Across every mapping
the run recorded `malformed: 0`, `out-of-order: 0`, and zero degraded frames. The frame rate is
presentation-limited: the surface presents at the 60 Hz display refresh, so the reported frame time
is the refresh interval rather than the renderer's ceiling.

The raw report is written to `.artifacts/visual/viz/product-demo-benchmark.txt` by:

```sh
npm run open:viz -- --port 5055 --benchmark 48 --benchmark-all-views
```

Duplicate mappings in that show — two Art-Net routes for universes 1 and 3, and an sACN route
duplicating universe 2 — are reported as `Superseded` with the reason, exercising the declared
priority behaviour without double-applying a frame.

### Test evidence

| Area | Where |
| --- | --- |
| Art-Net and sACN packet conformance, sequence, priority, termination | `crates/viz/dmx/src/packet.rs` |
| Real same-host UDP delivery, malformed and out-of-order handling, shared listeners | `crates/viz/dmx/src/receiver.rs` |
| Channel decoding: fine bytes, inversion, ranges, functions | `crates/viz/project/src/binding.rs` |
| Additive, subtractive, and wheel colour | `crates/viz/project/src/colour.rs` |
| Virtual dimmer, axis inversion, strobe gating, changed-universe-only re-decode | `crates/viz/project/src/decode.rs` |
| Documented fallback rules for every shipped `fixture_type` | `crates/viz/project/src/fallback.rs` |
| Effective transform resolution order and stage axis conversion | `crates/viz/desk/src/transform.rs` |
| Output routes to input mappings, duplicate priority, defaults | `crates/viz/desk/src/routes.rs` |
| Scene assembly from real shipped fixture packages | `crates/viz/desk/src/tests.rs` |
| Renderer-owned haze amount, bounded and independent of hazers | `crates/viz/scene/src/atmosphere.rs` |
| Named views, wire values, orthographic directions | `crates/viz/scene/src/view.rs` |
| Quick Settings staging, cancel, validation, local quality override | `apps/viz-renderer/src/ui.rs` |
| Provider swap never merges scenes or keeps stale values | `apps/viz-renderer/src/session.rs` |
| Desk-owned view: every named mode, patch semantics, replay, one target per renderer | `crates/light/adapters/headless/src/runtime/tests/visualizer_view_route_tests.rs` |
| Desk view stored with the installation rather than the show | `crates/light/adapters/headless/src/runtime/visualizer_view_http.rs` |
| Live values carried across a structural change by head identity | `crates/viz/scene/src/values.rs` |
| A delta applies in place; only a change of show replaces the scene | `crates/viz/desk/src/provider.rs`, `apps/viz-renderer/src/session.rs` |
| Deterministic fake planning-software provider | `apps/viz-renderer/src/demo.rs` |
| Read-only visualizer session role | `crates/light/adapters/headless/src/runtime/tests/runtime_v2_route_tests.rs` |

Fixed-camera images for the named views and quality tiers are written to `.artifacts/visual/viz/`
by rendering with `--view` and `--quality` plus `--capture`. The anti-aliasing pair beside them —
`aa-off.png` and `aa-on.png`, with magnified crops — is the same plan view rendered with
`TOSKLIGHT_VIZ_SAMPLES=1` and with the adapter's own count.

## Known gaps against the full plan

- Two-host captures and the one-hour soak in
  [Verification evidence](#verification-evidence-required-at-implementation-closeout) have not been
  run; the recorded evidence is same-host.
- One shipped profile carries a scan script, so a laser imported from anywhere else stays dark
  until somebody writes its bank.

Closed since the third implementation pass:

- The demo show carries a laser. `Generic Laser` is patched as **Laser 1** at 8.301 on the Floor
  layer, upstage centre on the deck and aimed out over the audience, with the profile revision and
  its scan bank travelling inside the show file — so the demo rig exercises the laser path without
  anything else having to be installed.
- The desk and the Viz editor find each other. Each advertises `_tosklight._tcp` with its role and
  what it is holding, and each offers the other's show: **Load from Desk** in the editor's file
  bar, **Load from Visualizer** in the desk's Load Show menu. Both transfers are a copy through a
  portable show file, and a network without discovery costs the button and nothing else. See
  [78-desk-and-viz-discovery](../../Next/78-desk-and-viz-discovery.md).

- Gobo artwork travels in the fixture package. A profile declares its wheel as `profile.gobos` —
  slot, name and a mask where light passes through white — and the shipped moving heads carry one.
  The projection layer decodes each piece of glass once per rig, the render core holds them in one
  array texture, and the gate samples the slot in the beam. A profile that declares no wheel keeps
  the drawn patterns, and a declared wheel also decides how many slots the channel is divided into
  rather than inheriting a guess.
- Prisms work on their own terms. They had two faults: with artwork in the gate the beam went
  black, because the fold pushed the gate outside the glass; and on an open slot a prism did
  nothing at all, because the gate was only read when something was in it. Each facet is now a
  separated copy with its own aperture, so a prism reads as several beams with or without a gobo,
  and frost blends them back together.
- Shipped fixture packages carry model geometry — twenty of them, lamps and venue objects — and a
  profile with no model of its own is drawn with the shipped body its type and channels imply
  rather than a procedural proxy.

Closed since the second implementation pass:

- The desk persists a visualizer view per renderer target and publishes every accepted change, and
  the renderer follows it. It is desk-level presentation state rather than portable show content:
  `POST /api/v2/visualizer-views/{target}/update` takes an intent patch with request identity,
  `GET /api/v2/visualizer-views` answers the default target even before anything is configured, and
  **Running & Output** carries the eight named views and the quality tier. An operator's local
  selection holds between instructions; an arriving view replaces it, camera included.
- A configuration change is applied as an incremental delta. The desk provider re-reads the scene
  over the connection that is already open and emits `ProviderEvent::SceneDelta` instead of
  dropping the session: the receivers stay bound unless the show actually moved a universe, the
  bindings are re-decoded from the held frames, and every head that still exists keeps its live
  value. Only a change of show still goes through the full snapshot path.
- The renderer speaks the desk's own event protocol. It had been opening `/api/v2/events` and
  reading frames without subscribing, which a desk answers with an error and a close — so every
  configuration change reached it as a dropped connection and a reconnect a couple of seconds
  later, and none of the event kinds it matched on were the ones a desk publishes. It now
  subscribes to the show and desk projections, reads the change out of the typed envelope, and
  still understands the planning window's simpler frame.
- The shaded passes are multisampled — four samples where the adapter offers it, two where it
  offers that, one where it offers neither, named beside the GPU in Quick Settings. Beams resolve
  with the geometry rather than on top of it, and the plan views stopped stair-stepping.

Closed since the first implementation pass:

- Shadow maps are implemented, with a per-tier budget, sampled by both the surface and the
  volumetric pass.
- Fixture models are loaded from the profile snapshot's inline GLB, with node names binding the
  moving parts and a named reason when one cannot be read.
- Gobo, gobo rotation, prism, prism rotation, iris, frost and the framing shapers are evaluated in
  the beam's own gate and drawn.
- The quality tier's render scale is applied: the shaded passes are drawn at the tier's fraction of
  the surface and composited up, while overlays and plan views stay at display resolution.
- The planning provider serves the configuration event stream, so a renderer connected to a
  planning document stays connected instead of reconnecting on its retry interval and rebinding its
  DMX receivers every couple of seconds.
- Renderer-local preferences are kept between launches, beside the operator's application data.
- The connection surface names the GPU and backend, what the GPU spent on a recent frame where the
  adapter can time one, and the aggregate input rate.
- Clicking a fixture inspects it: number, name, patch address and current level.
- A private show server or planning window that exits is noticed and named rather than left looking
  like a slow connection.
