# Technical wireframe renderer

`Renderer` owns one GPU, presentation surface/configuration, timing sequence and submission path. It selects a lazy `LinesRenderer` for `Lines3d`, or a lazy `ShadedRenderer` for the existing shaded and plan modes. A Lines-only session initializes neither shaded targets/pipelines nor the media atlas. An explicit media upload can initialize the atlas independently. Overlay glyphs and icons remain common and use final physical output resolution.

## Geometry and depth

Lines uses each fixture's actual package GLB, including existing base/yoke/head, scanner, physical-position and mounting transforms. Existing authoritative picking remains unchanged. Procedural geometry is used only for the existing scenery and fixtures whose packages have no usable model.

Feature extraction welds positional seams, suppresses coplanar triangulation edges, retains boundaries and creases of at least 30 degrees, and stores smooth adjacency for camera-dependent silhouette selection. Nonmanifold and reversed topology is conservative. Mesh and edge caches are bounded by the current drawn scene and keyed by actual position/index content, so replacing a model at the same index/revision cannot retain its old outline. Shaded model/projection caches likewise compare positions, normals and indices and invalidate shadow ownership before the frame which uses changed geometry.

The full-resolution output uses one sample. Actual mesh triangles first write depth without colour; the same pass then draws depth-tested feature edges and aim/grid/selection guides directly into LDR output. Screen-space quads provide analytic subpixel coverage without MSAA. Near-plane clipping occurs before perspective division. There are no simulated lights, tile lists, PBR material loops, beam/laser volumes, shadow maps, HDR intermediates, bloom or tone-mapping composite on this path.

## Retained work

CPU instance bundles are cached per fixture and emitter, with a separate scenery/grid/media-geometry bundle. Camera-only movement preserves fixture transforms. Isolated fixture motion rebuilds its affected bundles; dimmer, colour and rotating optics do not rebuild bodies. Point/group-selection changes conservatively invalidate affected ownership. Architectural cutaways retain their camera-dependent visibility and currently rebuild scenery during camera motion.

GPU instance and guide allocations survive unchanged frames. Uploads cover contiguous changed instance ranges only; camera uniforms update separately. Geometry content is currently fingerprinted on each draw to detect even in-place changes with reused revisions. This is a known CPU cost proportional to distinct visible asset bytes; measure it on the reference rigs before introducing an immutable asset-adoption identity. No historical scene cache grows across show switches.

The common timing namespace advances on mode, quality, dimensions, scene and actual asset changes. A delayed sample from a previous asset or a previous visit to a mode cannot be attributed to the new picture. Sampled pass time still excludes other GPU work and cannot establish total GPU time per second or displayed cadence.

## Validation

Focused tests cover real-GLB edges, seam welding, smooth silhouette candidates, malformed topology, incremental articulation/selection/unpatched visibility, dirty upload ranges, standalone shader validation, actual hidden-line pixels, lazy resources/media, same-slot replacement, shading-to-Lines transitions, capture resize and asynchronous readback ownership. Native GPU cases must be run explicitly with their coordinated `--ignored --test-threads=1` filters.

Use the matched packaged measurements in `stage-gpu-measurement.md` and the final acceptance scenarios in `docs/testing/16-stage-performance.md`. Passing source, CPU and headless pixel tests does not establish displayed 60 FPS, the proposed 90 percent Stage GPU reduction, packaged operator behavior or human acceptance.
