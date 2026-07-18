import * as THREE from "three";
import type {
	AttributeValue,
	FixtureMode,
	GeometryEmitter,
	PatchedFixture,
	Vector3Value,
	VisualizationSnapshot,
} from "../api/types";
import type { StagePosition3d } from "../api/ServerContext";
import {
	createBuiltInFixtureModel,
	movingLightTiltRadians,
} from "./builtInStageModels";

export interface Stage3dFixture {
	fixture: PatchedFixture;
	position: StagePosition3d;
	index: number;
	instanceId?: string;
}

function normalized(value: AttributeValue | undefined, fallback: number) {
	return value?.kind === "normalized" ? value.value : fallback;
}

function discrete(value: AttributeValue | undefined) {
	return value?.kind === "discrete" ? value.value : null;
}

function parameterDefault(
	fixture: PatchedFixture,
	attribute: string,
	fallback: number,
) {
	return (
		fixture.definition.heads
			?.flatMap((head) => head.parameters)
			.find((parameter) => parameter.attribute === attribute)?.default ??
		fallback
	);
}

function capabilityName(
	fixture: PatchedFixture,
	attribute: string,
	value: AttributeValue | undefined,
) {
	const named = discrete(value);
	if (named) return named;
	if (value?.kind !== "normalized") return null;
	const raw = Math.round(value.value * 255);
	return (
		fixture.definition.heads
			?.flatMap((head) => head.parameters)
			.find((parameter) => parameter.attribute === attribute)
			?.capabilities?.find(
				(capability) => raw >= capability.dmx_from && raw <= capability.dmx_to,
			)?.name ?? null
	);
}

export function fallbackEmitterIsDirectional(fixture: PatchedFixture) {
	const text =
		`${fixture.definition.device_type} ${fixture.definition.manufacturer} ${fixture.definition.name} ${fixture.definition.model}`.toLowerCase();
	if (/sun\s*strip|sunstrip|strip light|striplight/.test(text)) return false;
	if (/\bstrobe\b/.test(text) && !/blinder/.test(text)) return false;
	return true;
}

function xyzToColor(
	value: AttributeValue | undefined,
	attributes: Map<string, AttributeValue>,
) {
	if (value?.kind === "color_xyz") {
		const { x, y, z } = value.value;
		const linear = [
			3.2406 * x - 1.5372 * y - 0.4986 * z,
			-0.9689 * x + 1.8758 * y + 0.0415 * z,
			0.0557 * x - 0.204 * y + 1.057 * z,
		];
		const gamma = (channel: number) =>
			channel <= 0.0031308
				? 12.92 * channel
				: 1.055 * Math.pow(channel, 1 / 2.4) - 0.055;
		return new THREE.Color(
			gamma(linear[0]),
			gamma(linear[1]),
			gamma(linear[2]),
		);
	}
	return new THREE.Color(
		normalized(attributes.get("color.red"), 1),
		normalized(attributes.get("color.green"), 1),
		normalized(attributes.get("color.blue"), 1),
	);
}

export function defaultStagePosition(index: number): StagePosition3d {
	return {
		x: -5.25 + (index % 8) * 1.5,
		y: 1 + Math.floor(index / 8) * 1.6,
		z: 5,
		rotationX: 0,
		rotationY: 0,
		rotationZ: 0,
	};
}

export function migrateStagePosition(
	position: { x: number; y: number; rotation: number } | undefined,
	index: number,
): StagePosition3d {
	if (!position) return defaultStagePosition(index);
	return {
		x: (position.x / 100 - 0.5) * 12,
		y: (position.y / 100) * 8,
		z: 5,
		rotationX: 0,
		rotationY: 0,
		rotationZ: position.rotation,
	};
}

function valuesByFixture(snapshot: VisualizationSnapshot | null) {
	const result = new Map<string, Map<string, AttributeValue>>();
	for (const entry of [
		...(snapshot?.values ?? []),
		...(snapshot?.profile_output_values ?? []),
	]) {
		const attributes =
			result.get(entry.fixture_id) ?? new Map<string, AttributeValue>();
		attributes.set(entry.attribute, entry.value);
		result.set(entry.fixture_id, attributes);
	}
	return result;
}

function fixtureBody(selected: boolean) {
	const group = new THREE.Group();
	group.name = "fixture-placeholder";
	const dark = new THREE.MeshStandardMaterial({
		color: selected ? 0x136f80 : 0x252c33,
		roughness: 0.55,
		metalness: 0.35,
	});
	const base = new THREE.Mesh(
		new THREE.CylinderGeometry(0.22, 0.27, 0.18, 16),
		dark,
	);
	const yoke = new THREE.Mesh(new THREE.BoxGeometry(0.46, 0.42, 0.12), dark);
	yoke.position.y = -0.25;
	const head = new THREE.Mesh(
		new THREE.CylinderGeometry(0.2, 0.24, 0.42, 16),
		dark,
	);
	head.rotation.z = Math.PI / 2;
	head.position.y = -0.52;
	group.add(base, yoke, head);
	if (selected)
		for (const mesh of [base, yoke, head]) {
			const outline = new THREE.LineSegments(
				new THREE.EdgesGeometry(mesh.geometry),
				new THREE.LineBasicMaterial({ color: 0x378eff }),
			);
			outline.position.copy(mesh.position);
			outline.rotation.copy(mesh.rotation);
			outline.scale.setScalar(1.035);
			outline.name = "selection-outline";
			group.add(outline);
		}
	return group;
}

function addSelectionOutline(object: THREE.Object3D) {
	object.traverse((child) => {
		if (!(child instanceof THREE.Mesh)) return;
		// Some imported or procedural models legitimately contain marker meshes
		// without vertex data. They cannot produce an EdgesGeometry outline.
		if (!child.geometry.getAttribute("position")?.count) return;
		const outline = new THREE.LineSegments(
			new THREE.EdgesGeometry(child.geometry),
			new THREE.LineBasicMaterial({ color: 0x378eff }),
		);
		outline.name = "selection-outline";
		outline.scale.setScalar(1.025);
		child.add(outline);
	});
}

function profileMode(fixture: PatchedFixture) {
	const profile = fixture.definition.profile_snapshot;
	return (
		profile?.modes.find((mode) => mode.id === fixture.definition.mode_id) ??
		profile?.modes.find((mode) => mode.name === fixture.definition.mode) ??
		null
	);
}

function headOwnerId(
	fixture: PatchedFixture,
	mode: FixtureMode,
	headId: string,
) {
	const index = mode.heads.findIndex((head) => head.id === headId);
	const head = mode.heads[index];
	if (!head || head.master_shared) return fixture.fixture_id;
	return (
		fixture.logical_heads.find((candidate) => candidate.head_index === index)
			?.fixture_id ??
		fixture.logical_heads.find(
			(candidate) => candidate.head_index === index + 1,
		)?.fixture_id ??
		fixture.fixture_id
	);
}

function attributesForHead(
	fixture: PatchedFixture,
	mode: FixtureMode,
	headId: string,
	byFixture: Map<string, Map<string, AttributeValue>>,
) {
	const attributes = new Map(byFixture.get(fixture.fixture_id) ?? []);
	const owner = headOwnerId(fixture, mode, headId);
	if (owner !== fixture.fixture_id)
		for (const [attribute, value] of byFixture.get(owner) ?? [])
			attributes.set(attribute, value);
	return attributes;
}

function channelDefault(
	mode: FixtureMode,
	headId: string,
	attribute: string,
	fallback: number,
) {
	const channel = mode.channels.find(
		(candidate) =>
			candidate.head_id === headId && candidate.attribute === attribute,
	);
	if (!channel) return fallback;
	const maximum = { u8: 0xff, u16: 0xffff, u24: 0xffffff, u32: 0xffffffff }[
		channel.resolution
	];
	return channel.default_raw / maximum;
}

const millimetres = (value: Vector3Value) =>
	new THREE.Vector3(value.x / 1_000, value.y / 1_000, value.z / 1_000);

function layoutOffsets(layout: GeometryEmitter["layout"]) {
	if (layout.type === "point") return [new THREE.Vector3()];
	if (layout.type === "ring")
		return Array.from({ length: layout.count }, (_, index) => {
			const angle = (index / layout.count) * Math.PI * 2;
			return new THREE.Vector3(
				(Math.cos(angle) * layout.radius_millimetres) / 1_000,
				0,
				(Math.sin(angle) * layout.radius_millimetres) / 1_000,
			);
		});
	if (layout.type === "strip")
		return Array.from(
			{ length: layout.count },
			(_, index) =>
				new THREE.Vector3(
					((index - (layout.count - 1) / 2) * layout.spacing_millimetres) /
						1_000,
					0,
					0,
				),
		);
	if (layout.type === "explicit_pixels")
		return layout.positions.map(millimetres);
	const offsets: THREE.Vector3[] = [];
	for (let row = 0; row < layout.rows; row++)
		for (let column = 0; column < layout.columns; column++)
			offsets.push(
				new THREE.Vector3(
					((column - (layout.columns - 1) / 2) * layout.spacing.x) / 1_000,
					((row - (layout.rows - 1) / 2) * layout.spacing.y) / 1_000,
					((row - (layout.rows - 1) / 2) * layout.spacing.z) / 1_000,
				),
			);
	return offsets;
}

function emitterSurfaceMaterial(color: THREE.Color, intensity: number) {
	return intensity > 0.001
		? new THREE.MeshBasicMaterial({
				color: color
					.clone()
					.lerp(new THREE.Color(0xffffff), 0.75)
					.multiplyScalar(2.3),
				toneMapped: false,
				side: THREE.DoubleSide,
			})
		: new THREE.MeshStandardMaterial({
				color: 0x56616a,
				roughness: 0.34,
				metalness: 0.18,
				side: THREE.DoubleSide,
			});
}

function geometryBeam(
	emitter: GeometryEmitter,
	attributes: Map<string, AttributeValue>,
	intensity: number,
	color: THREE.Color,
	showBeamGuides: boolean,
) {
	const distance = 7;
	const zoom = normalized(
		attributes.get("beam.zoom") ?? attributes.get("zoom"),
		0.5,
	);
	const focus = normalized(
		attributes.get("beam.focus") ?? attributes.get("focus"),
		emitter.focus,
	);
	const zoomScale = 0.6 + zoom * 0.8;
	const beamAngle = emitter.beam_angle_degrees * zoomScale;
	const fieldAngle = emitter.field_angle_degrees * zoomScale;
	const beamRadius =
		Math.tan(THREE.MathUtils.degToRad(beamAngle / 2)) * distance;
	const radius = Math.tan(THREE.MathUtils.degToRad(fieldAngle / 2)) * distance;
	const group = new THREE.Group();
	group.name = `geometry-emitter:${emitter.id}`;
	group.userData.beamAngleDegrees = beamAngle;
	group.userData.fieldAngleDegrees = fieldAngle;
	group.userData.feather = emitter.feather;
	group.userData.focus = focus;
	group.userData.sourceCount = layoutOffsets(emitter.layout).length;
	group.userData.intensity = intensity;
	group.userData.color = `#${color.getHexString()}`;
	group.position.copy(millimetres(emitter.origin));
	group.rotation.set(
		THREE.MathUtils.degToRad(emitter.orientation_degrees.x),
		THREE.MathUtils.degToRad(emitter.orientation_degrees.y),
		THREE.MathUtils.degToRad(emitter.orientation_degrees.z),
	);
	layoutOffsets(emitter.layout).forEach((offset, index) => {
		const beam = new THREE.Group();
		beam.name = `geometry-source:${emitter.id}:${index}`;
		beam.position.copy(offset);
		beam.userData.emitterId = emitter.id;
		beam.userData.headId = emitter.head_id;
		beam.userData.layout = emitter.layout.type;
		const active = intensity > 0.001;
		const source = new THREE.Mesh(
			new THREE.CircleGeometry(
				Math.max(0.012, Math.min(0.08, radius / 18)),
				12,
			),
			emitterSurfaceMaterial(color, intensity),
		);
		source.name = "light-emitting-surface";
		source.userData.active = active;
		source.rotation.x = -Math.PI / 2;
		const coneGeometry = new THREE.ConeGeometry(radius, distance, 24, 1, true);
		coneGeometry.translate(0, -distance / 2, 0);
		const volume = new THREE.Mesh(
			coneGeometry,
			new THREE.MeshBasicMaterial({
				color,
				transparent: true,
				opacity:
					intensity * (0.025 + (1 - emitter.feather) * 0.035 + focus * 0.04),
				side: THREE.DoubleSide,
				depthWrite: false,
				blending: THREE.AdditiveBlending,
			}),
		);
		volume.name = "beam-volume";
		const coreGeometry = new THREE.ConeGeometry(
			beamRadius,
			distance,
			24,
			1,
			true,
		);
		coreGeometry.translate(0, -distance / 2, 0);
		const core = new THREE.Mesh(
			coreGeometry,
			new THREE.MeshBasicMaterial({
				color,
				transparent: true,
				opacity: intensity * (0.02 + focus * 0.045),
				side: THREE.DoubleSide,
				depthWrite: false,
				blending: THREE.AdditiveBlending,
			}),
		);
		core.name = "beam-core";
		const outline = new THREE.LineSegments(
			new THREE.EdgesGeometry(coneGeometry, 28),
			active
				? new THREE.LineBasicMaterial({
						color,
						transparent: true,
						opacity: 0.25 + intensity * 0.5,
					})
				: new THREE.LineDashedMaterial({
						color: 0x7b858d,
						transparent: true,
						opacity: 0.3,
						dashSize: 0.18,
						gapSize: 0.14,
					}),
		);
		outline.name = "beam-outline";
		if (!active) outline.computeLineDistances();
		beam.add(source, volume, core);
		if (active || ((emitter.directional ?? true) && showBeamGuides))
			beam.add(outline);
		if (!active && (emitter.directional ?? true) && showBeamGuides) {
			const guideGeometry = new THREE.BufferGeometry().setFromPoints([
				new THREE.Vector3(),
				new THREE.Vector3(0, -distance, 0),
			]);
			const guide = new THREE.Line(
				guideGeometry,
				new THREE.LineDashedMaterial({
					color: 0x8d989f,
					transparent: true,
					opacity: 0.42,
					dashSize: 0.18,
					gapSize: 0.14,
				}),
			);
			guide.name = "beam-direction-guide";
			guide.computeLineDistances();
			beam.add(guide);
		}
		group.add(beam);
	});
	return group;
}

function schemaV2Geometry(
	fixture: PatchedFixture,
	mode: FixtureMode,
	byFixture: Map<string, Map<string, AttributeValue>>,
	selected: boolean,
	snapshot: VisualizationSnapshot | null,
	projectedOwners: Set<string>,
	showBeamGuides: boolean,
	virtualHighlight = false,
) {
	const graph = mode.geometry;
	if (!graph.nodes.length) return null;
	const result = new THREE.Group();
	result.name = "fixture-profile-geometry";
	const rootAttributes =
		byFixture.get(fixture.fixture_id) ?? new Map<string, AttributeValue>();
	const descendants = (nodeId: string) =>
		graph.emitters.filter((emitter) => {
			let cursor: string | null = emitter.node_id;
			while (cursor) {
				if (cursor === nodeId) return true;
				cursor =
					graph.nodes.find((node) => node.id === cursor)?.parent_id ?? null;
			}
			return false;
		});
	const nodes = new Map<string, { group: THREE.Group; anchor: THREE.Group }>();
	graph.nodes.forEach((node, index) => {
		const group = new THREE.Group();
		const anchor = new THREE.Group();
		group.name = `geometry-node:${node.id}`;
		anchor.name = `geometry-node-anchor:${node.id}`;
		const relatedHeads = [
			...new Set(descendants(node.id).map((emitter) => emitter.head_id)),
		];
		const attributes =
			relatedHeads.length === 1
				? attributesForHead(fixture, mode, relatedHeads[0], byFixture)
				: new Map(rootAttributes);
		if (relatedHeads.length > 1)
			for (const headId of relatedHeads)
				for (const [attribute, value] of attributesForHead(
					fixture,
					mode,
					headId,
					byFixture,
				))
					if (!attributes.has(attribute)) attributes.set(attribute, value);
		const translation = millimetres(node.transform.translation);
		const rotation = { ...node.transform.rotation_degrees };
		if (node.motion) {
			const level = normalized(attributes.get(node.motion.attribute), 0.5);
			const physical =
				node.motion.physical_min +
				(node.motion.physical_max - node.motion.physical_min) * level;
			if (node.motion.kind === "rotation") {
				rotation.x += node.motion.axis.x * physical;
				rotation.y += node.motion.axis.y * physical;
				rotation.z += node.motion.axis.z * physical;
			} else {
				translation.add(
					millimetres({
						x: node.motion.axis.x * physical,
						y: node.motion.axis.y * physical,
						z: node.motion.axis.z * physical,
					}),
				);
			}
		}
		const pivot = millimetres(node.pivot);
		group.position.copy(translation).add(pivot);
		group.rotation.set(
			THREE.MathUtils.degToRad(rotation.x),
			THREE.MathUtils.degToRad(rotation.y),
			THREE.MathUtils.degToRad(rotation.z),
		);
		group.scale.set(
			node.transform.scale.x || 1,
			node.transform.scale.y || 1,
			node.transform.scale.z || 1,
		);
		anchor.position.copy(pivot).multiplyScalar(-1);
		const dimensions =
			index === 0
				? [
						fixture.definition.physical.width_millimetres,
						fixture.definition.physical.height_millimetres,
						fixture.definition.physical.depth_millimetres,
					].map((value) => Math.max(0.08, (value ?? 160) / 1_000))
				: [0.12, 0.12, 0.12];
		const marker = new THREE.Mesh(
			new THREE.BoxGeometry(dimensions[0], dimensions[1], dimensions[2]),
			new THREE.MeshStandardMaterial({
				color: selected ? 0x136f80 : 0x252c33,
				roughness: 0.55,
				metalness: 0.35,
			}),
		);
		marker.name = `geometry-part:${node.id}`;
		anchor.add(marker);
		group.add(anchor);
		nodes.set(node.id, { group, anchor });
	});
	for (const node of graph.nodes) {
		const current = nodes.get(node.id)!;
		const parent = node.parent_id ? nodes.get(node.parent_id)?.anchor : null;
		(parent ?? result).add(current.group);
	}
	for (const emitter of graph.emitters) {
		const attributes = attributesForHead(
			fixture,
			mode,
			emitter.head_id,
			byFixture,
		);
		const owner = headOwnerId(fixture, mode, emitter.head_id);
		const resolvedIntensity = normalized(
			attributes.get("intensity"),
			channelDefault(mode, emitter.head_id, "intensity", 1),
		);
		const intensity = virtualHighlight
			? 1
			: projectedOwners.has(owner)
				? resolvedIntensity
				: (snapshot?.blackout ? 0 : resolvedIntensity) *
					(snapshot?.grand_master ?? 1);
		const color = xyzToColor(attributes.get("color"), attributes);
		(nodes.get(emitter.node_id)?.anchor ?? result).add(
			geometryBeam(emitter, attributes, intensity, color, showBeamGuides),
		);
	}
	if (selected) addSelectionOutline(result);
	return result;
}

/** Build the same hierarchy and beam objects used on Stage for the profile editor's live preview. */
export function buildFixtureProfileGeometryPreview(mode: FixtureMode) {
	const fixture = {
		fixture_id: "fixture-profile-preview",
		logical_heads: [],
		definition: {
			physical: {},
			heads: [],
		},
	} as unknown as PatchedFixture;
	return (
		schemaV2Geometry(fixture, mode, new Map(), false, null, new Set(), true) ??
		new THREE.Group()
	);
}

function removeGeometryMarker(root: THREE.Object3D, nodeId: string) {
	const marker = root.getObjectByName(`geometry-part:${nodeId}`);
	marker?.parent?.remove(marker);
}

function normalizedModelScale(model: THREE.Object3D, fixture: PatchedFixture) {
	const size = new THREE.Box3()
		.setFromObject(model)
		.getSize(new THREE.Vector3());
	const desiredHeight =
		(fixture.definition.physical.height_millimetres ?? 600) / 1_000;
	return desiredHeight / Math.max(size.y, size.x, size.z, 0.001);
}

function removeNestedBoundParts(
	model: THREE.Object3D,
	boundNames: Set<string>,
	ownName: string,
) {
	const nested: THREE.Object3D[] = [];
	model.traverse((object) => {
		if (
			object !== model &&
			object.name !== ownName &&
			boundNames.has(object.name)
		)
			nested.push(object);
	});
	for (const object of nested) object.parent?.remove(object);
}

/**
 * Attach a loaded profile GLB to the schema-v2 geometry hierarchy. A bound GLB node contributes
 * visual content while the profile node remains authoritative for hierarchy, pivot, and motion.
 */
export function mountFixtureModel(
	root: THREE.Object3D,
	model: THREE.Object3D,
	fixture: PatchedFixture,
	selected = false,
) {
	const instanceId = root.userData.instanceId ?? fixture.fixture_id;
	model.updateMatrixWorld(true);
	// Visual-only Venue packages author GLB geometry in metres so transferred scenic elements
	// retain exact real-world dimensions instead of being normalized like lamp body models.
	const scale =
		fixture.definition.profile_snapshot?.model_units === "metres"
			? 1
			: normalizedModelScale(model, fixture);
	const mode = profileMode(fixture);
	const bindings = mode?.geometry.nodes.filter((node) => node.glb_node) ?? [];
	const boundNames = new Set(
		bindings.flatMap((node) => (node.glb_node ? [node.glb_node] : [])),
	);
	let mounted = 0;

	for (const node of bindings) {
		const source = node.glb_node ? model.getObjectByName(node.glb_node) : null;
		const anchor = root.getObjectByName(`geometry-node-anchor:${node.id}`);
		if (!source || !anchor) continue;
		const part = source.clone(true);
		removeNestedBoundParts(part, boundNames, source.name);
		// The profile graph owns this node's transform. Retain GLB-local scale and child transforms,
		// but do not apply the source node's authored position/rotation a second time.
		part.position.set(0, 0, 0);
		part.quaternion.identity();
		const wrapper = new THREE.Group();
		wrapper.name = `fixture-model-part:${node.id}`;
		wrapper.scale.setScalar(scale);
		wrapper.add(part);
		wrapper.traverse((object) => {
			object.userData.fixtureId = fixture.fixture_id;
			object.userData.instanceId = instanceId;
		});
		if (selected) addSelectionOutline(wrapper);
		removeGeometryMarker(root, node.id);
		anchor.add(wrapper);
		mounted += 1;
	}

	if (mounted) return mounted;

	model.name = "fixture-model";
	model.traverse((object) => {
		object.userData.fixtureId = fixture.fixture_id;
		object.userData.instanceId = instanceId;
	});
	model.scale.setScalar(scale);
	const scaledBox = new THREE.Box3().setFromObject(model);
	const center = scaledBox.getCenter(new THREE.Vector3());
	model.position.sub(center);
	model.position.y -= scaledBox.min.y - center.y;
	if (selected) addSelectionOutline(model);
	const profileRoot = mode?.geometry.nodes.find(
		(node) => node.parent_id == null,
	);
	const target = profileRoot
		? (root.getObjectByName(`geometry-node-anchor:${profileRoot.id}`) ?? root)
		: root;
	if (profileRoot) removeGeometryMarker(root, profileRoot.id);
	target.add(model);
	return 1;
}

export function buildStageScene(
	fixtures: Stage3dFixture[],
	snapshot: VisualizationSnapshot | null,
	selected: Set<string> = new Set(),
	environmentBrightness = 1,
	showFloorGrid = true,
	showBeamGuides = true,
	virtualHighlight: Set<string> = new Set(),
) {
	const scene = new THREE.Scene();
	scene.background = new THREE.Color(0x080b0f).lerp(
		new THREE.Color(0x26323a),
		environmentBrightness * 0.18,
	);
	scene.add(
		new THREE.HemisphereLight(0xa9c8dc, 0x11151a, environmentBrightness * 1.5),
	);
	if (showFloorGrid) {
		const floor = new THREE.Mesh(
			new THREE.PlaneGeometry(12, 8),
			new THREE.MeshStandardMaterial({ color: 0x151b20, roughness: 0.9 }),
		);
		floor.name = "stage-floor";
		floor.rotation.x = -Math.PI / 2;
		floor.position.set(0, 0, -4);
		scene.add(floor);
		const grid = new THREE.GridHelper(12, 24, 0x24798a, 0x263039);
		grid.name = "stage-floor-grid";
		grid.position.z = -4;
		scene.add(grid);
	}
	const byFixture = valuesByFixture(snapshot);
	const projectedOwners = new Set(
		(snapshot?.profile_output_values ?? []).map((entry) => entry.fixture_id),
	);
	const fixtureObjects = new Map<string, THREE.Object3D>();

	for (const item of fixtures) {
		const id = item.fixture.fixture_id;
		const attributes = byFixture.get(id) ?? new Map<string, AttributeValue>();
		const root = new THREE.Group();
		const instanceId = item.instanceId ?? id;
		root.name = `fixture:${id}:${instanceId}`;
		root.userData.fixtureId = id;
		root.userData.instanceId = instanceId;
		root.userData.stageSelected = selected.has(id);
		root.position.set(item.position.x, item.position.z, -item.position.y);
		root.rotation.set(
			THREE.MathUtils.degToRad(item.position.rotationX),
			THREE.MathUtils.degToRad(item.position.rotationZ),
			THREE.MathUtils.degToRad(item.position.rotationY),
		);
		const mode = profileMode(item.fixture);
		const profileGeometry = mode
			? schemaV2Geometry(
					item.fixture,
					mode,
					byFixture,
					selected.has(id),
					snapshot,
					projectedOwners,
					showBeamGuides,
					virtualHighlight.has(id),
				)
			: null;
		if (profileGeometry) {
			root.add(profileGeometry);
			scene.add(root);
			fixtureObjects.set(instanceId, root);
			continue;
		}
		const intensity = virtualHighlight.has(id)
			? 1
			: (snapshot?.blackout
					? 0
					: normalized(
							attributes.get("intensity"),
							parameterDefault(item.fixture, "intensity", 0),
						)) * (snapshot?.grand_master ?? 1);
		const pan =
			(normalized(
				attributes.get("pan"),
				parameterDefault(item.fixture, "pan", 0.5),
			) -
				0.5) *
			Math.PI *
			2;
		const tilt = movingLightTiltRadians(
			normalized(
				attributes.get("tilt"),
				parameterDefault(item.fixture, "tilt", 0.5),
			),
		);
		const zoom = normalized(
			attributes.get("zoom"),
			parameterDefault(item.fixture, "zoom", 0.35),
		);
		const focus = normalized(
			attributes.get("focus"),
			parameterDefault(item.fixture, "focus", 0.65),
		);
		const color = xyzToColor(attributes.get("color"), attributes);
		const distance = 7;
		const radius = Math.tan(THREE.MathUtils.degToRad(4 + zoom * 23)) * distance;
		let beamParent: THREE.Object3D;
		if (item.fixture.definition.model_asset) {
			const placeholder = fixtureBody(selected.has(id));
			root.add(placeholder);
			beamParent = root;
		} else {
			const model = createBuiltInFixtureModel(
				item.fixture,
				color,
				intensity,
				pan,
				tilt,
			);
			model.object.name = "fixture-placeholder";
			if (selected.has(id)) addSelectionOutline(model.object);
			root.add(model.object);
			beamParent = model.beamMount;
		}
		const beam = new THREE.Group();
		if (beamParent === root) {
			beam.position.y = -0.62;
			// Match the built-in yoke: local tilt is around X, followed by pan around Y.
			const direction = new THREE.Vector3(
				-Math.sin(pan) * Math.sin(tilt),
				-Math.cos(tilt),
				-Math.cos(pan) * Math.sin(tilt),
			).normalize();
			beam.quaternion.setFromUnitVectors(
				new THREE.Vector3(0, -1, 0),
				direction,
			);
		}
		const coneGeometry = new THREE.ConeGeometry(radius, distance, 32, 1, true);
		coneGeometry.translate(0, -distance / 2, 0);
		const volume = new THREE.Mesh(
			coneGeometry,
			new THREE.MeshBasicMaterial({
				color,
				transparent: true,
				opacity: intensity * (0.035 + focus * 0.055),
				side: THREE.DoubleSide,
				depthWrite: false,
				blending: THREE.AdditiveBlending,
			}),
		);
		const activeBeam = intensity > 0.001;
		if (beamParent === root) {
			const source = new THREE.Mesh(
				new THREE.CircleGeometry(
					Math.max(0.04, Math.min(0.11, radius / 16)),
					24,
				),
				emitterSurfaceMaterial(color, intensity),
			);
			source.name = "light-emitting-surface";
			source.userData.active = activeBeam;
			source.rotation.x = -Math.PI / 2;
			beam.add(source);
		}
		const directional = fallbackEmitterIsDirectional(item.fixture);
		const guideColor = activeBeam ? color : new THREE.Color(0x7b858d);
		const guideMaterial = activeBeam
			? new THREE.LineBasicMaterial({
					color: guideColor,
					transparent: true,
					opacity: 0.28 + intensity * 0.55,
				})
			: new THREE.LineDashedMaterial({
					color: guideColor,
					transparent: true,
					opacity: 0.3,
					dashSize: 0.18,
					gapSize: 0.14,
				});
		const outline = new THREE.LineSegments(
			new THREE.EdgesGeometry(coneGeometry, 28),
			guideMaterial,
		);
		if (!activeBeam) outline.computeLineDistances();
		const centerGeometry = new THREE.BufferGeometry().setFromPoints([
			new THREE.Vector3(),
			new THREE.Vector3(0, -distance, 0),
		]);
		const center = new THREE.Line(
			centerGeometry,
			activeBeam
				? new THREE.LineBasicMaterial({
						color,
						transparent: true,
						opacity: 0.45 + intensity * 0.4,
					})
				: new THREE.LineDashedMaterial({
						color: 0x7b858d,
						transparent: true,
						opacity: 0.35,
						dashSize: 0.18,
						gapSize: 0.14,
					}),
		);
		center.name = activeBeam ? "beam-centerline" : "beam-direction-guide";
		if (!activeBeam) center.computeLineDistances();
		beam.add(volume);
		if (activeBeam || (directional && showBeamGuides)) beam.add(outline);
		if (activeBeam || (directional && showBeamGuides)) beam.add(center);
		const gobo = capabilityName(item.fixture, "gobo", attributes.get("gobo"));
		if (gobo && gobo.toLowerCase() !== "open") {
			for (let spoke = 0; spoke < 6; spoke++) {
				const angle = (spoke / 6) * Math.PI * 2;
				const line = new THREE.BufferGeometry().setFromPoints([
					new THREE.Vector3(),
					new THREE.Vector3(
						Math.cos(angle) * radius,
						-distance,
						Math.sin(angle) * radius,
					),
				]);
				beam.add(
					new THREE.Line(
						line,
						new THREE.LineBasicMaterial({
							color,
							transparent: true,
							opacity: intensity * 0.45,
						}),
					),
				);
			}
		}
		beamParent.add(beam);
		scene.add(root);
		fixtureObjects.set(instanceId, root);
	}
	return { scene, fixtureObjects };
}

export function disposeScene(scene: THREE.Scene) {
	scene.traverse((object) => {
		const mesh = object as THREE.Mesh;
		mesh.geometry?.dispose();
		const materials = Array.isArray(mesh.material)
			? mesh.material
			: mesh.material
				? [mesh.material]
				: [];
		for (const material of materials) material.dispose();
	});
}

export function cueVisualization(
	base: VisualizationSnapshot | null,
	changes: Array<{
		fixture_id: string;
		attribute: string;
		value: AttributeValue | null;
	}>,
) {
	const entries = new Map(
		(base?.values ?? []).map((entry) => [
			`${entry.fixture_id}\0${entry.attribute}`,
			entry,
		]),
	);
	for (const change of changes) {
		const key = `${change.fixture_id}\0${change.attribute}`;
		if (change.value) entries.set(key, { ...change, value: change.value });
		else entries.delete(key);
	}
	return {
		revision: base?.revision ?? 0,
		generated_at: new Date().toISOString(),
		grand_master: 1,
		blackout: false,
		values: [...entries.values()],
	} satisfies VisualizationSnapshot;
}

export function renderStageThumbnail(
	fixtures: Stage3dFixture[],
	snapshot: VisualizationSnapshot,
	width = 240,
	height = 135,
) {
	const { scene } = buildStageScene(fixtures, snapshot);
	const renderer = new THREE.WebGLRenderer({
		antialias: true,
		preserveDrawingBuffer: true,
		alpha: false,
	});
	renderer.setSize(width, height, false);
	renderer.setPixelRatio(1);
	const camera = new THREE.PerspectiveCamera(48, width / height, 0.1, 100);
	camera.position.set(10, 8, 11);
	camera.lookAt(0, 1.8, -4);
	renderer.render(scene, camera);
	const result = renderer.domElement.toDataURL("image/webp", 0.8);
	disposeScene(scene);
	renderer.forceContextLoss();
	renderer.dispose();
	return result;
}
