import * as THREE from "three";
import type {
	FixtureChannel,
	FixtureMode,
	PatchedFixture,
} from "../../api/types";
import { normalized } from "./attributeValues";
import type { FixtureAttributeValues, StageShaperState } from "./types";

const BLADE_ATTRIBUTES = [
	["shaper.blade.1.position", "shaper.blade.1.angle"],
	["shaper.blade.2.position", "shaper.blade.2.angle"],
	["shaper.blade.3.position", "shaper.blade.3.angle"],
	["shaper.blade.4.position", "shaper.blade.4.angle"],
] as const;

function channelsForHead(mode: FixtureMode, headId?: string) {
	if (!headId) return mode.channels;
	const sharedHeads = new Set(
		mode.heads.filter((head) => head.master_shared).map((head) => head.id),
	);
	return mode.channels.filter(
		(channel) => channel.head_id === headId || sharedHeads.has(channel.head_id),
	);
}

function channel(channels: readonly FixtureChannel[], attribute: string) {
	return channels.find(
		(candidate) =>
			candidate.attribute === attribute ||
			candidate.fixture_attribute === attribute,
	);
}

function physicalValue(
	definition: FixtureChannel,
	attributes: FixtureAttributeValues,
) {
	const level = normalized(attributes.get(definition.attribute), NaN);
	const fallbackLevel = Number.isFinite(level)
		? level
		: normalized(attributes.get(definition.fixture_attribute), 0);
	const minimum = definition.physical_min ?? 0;
	const maximum = definition.physical_max ?? 1;
	return minimum + (maximum - minimum) * fallbackLevel;
}

export function resolveStageShaper(
	fixture: PatchedFixture,
	mode: FixtureMode | null,
	attributes: FixtureAttributeValues,
	appearance = fixture.installed_appearance,
	moduleRotation = fixture.shaper_angle,
	headId?: string,
): StageShaperState {
	const channels = mode ? channelsForHead(mode, headId) : [];
	const supported = BLADE_ATTRIBUTES.map(([position, angle]) =>
		Boolean(channel(channels, position) || channel(channels, angle)),
	) as StageShaperState["supported"];
	const insertions = BLADE_ATTRIBUTES.map(([position]) => {
		const definition = channel(channels, position);
		return definition
			? THREE.MathUtils.clamp(
					normalized(
						attributes.get(definition.attribute) ??
							attributes.get(definition.fixture_attribute),
						0,
					),
					0,
					1,
				)
			: 0;
	}) as StageShaperState["insertions"];
	const anglesDegrees = BLADE_ATTRIBUTES.map(([, angle], index) => {
		const definition = channel(channels, angle);
		if (definition) return physicalValue(definition, attributes);
		return supported[index]
			? (appearance?.shaper_angles_degrees[index] ?? 0)
			: 0;
	}) as StageShaperState["anglesDegrees"];
	const liveRotation = channel(channels, "shaper.rotation");
	return {
		supported,
		insertions,
		anglesDegrees,
		moduleRotationDegrees: liveRotation
			? physicalValue(liveRotation, attributes)
			: supported.some(Boolean)
				? (moduleRotation ?? 0)
				: 0,
	};
}

function createBlade(index: number) {
	const group = new THREE.Group();
	group.name = `stage-shaper-blade:${index + 1}`;
	const blade = new THREE.Mesh(
		new THREE.PlaneGeometry(0.16, 0.045),
		new THREE.MeshBasicMaterial({
			color: 0x171b1e,
			side: THREE.DoubleSide,
		}),
	);
	blade.rotation.x = -Math.PI / 2;
	group.add(blade);
	return group;
}

/**
 * Mount the four typed physical blade roles at the beam gate. The renderer deliberately attaches
 * this to its semantic emitter source; arbitrary imported GLB node names are never inspected.
 */
export function applyStageShaper(
	beam: THREE.Object3D,
	state: StageShaperState,
) {
	beam.userData.stageShaper = state;
	let module = beam.getObjectByName("stage-shaper-module");
	if (!state.supported.some(Boolean)) {
		if (module instanceof THREE.Group) module.visible = false;
		return;
	}
	if (!(module instanceof THREE.Group)) {
		module = new THREE.Group();
		module.name = "stage-shaper-module";
		module.position.y = -0.002;
		beam.add(module);
	}
	module.visible = true;
	module.rotation.y = THREE.MathUtils.degToRad(state.moduleRotationDegrees);
	for (let index = 0; index < 4; index++) {
		let blade = module.getObjectByName(`stage-shaper-blade:${index + 1}`);
		if (!(blade instanceof THREE.Group)) {
			blade = createBlade(index);
			module.add(blade);
		}
		blade.visible = state.supported[index];
		const side = index % 2 === 0 ? 1 : -1;
		const vertical = index < 2;
		const edge = side * (0.09 - state.insertions[index] * 0.075);
		blade.position.set(vertical ? 0 : edge, 0, vertical ? edge : 0);
		blade.rotation.y = THREE.MathUtils.degToRad(
			state.anglesDegrees[index] + (vertical ? 0 : 90),
		);
	}
}
