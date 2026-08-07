import type { StoredGroup } from "../../api/types";
import type {
	Position3d,
	ProjectionPreset,
	RadarSweep,
	RadialDirection,
	RankDirection,
	SpatialProjection,
	SpatialSelectionMapping,
	SpatialSelectionShape,
} from "../../features/spatialMapping/contracts";
import type { Group } from "./model";

export type {
	Position3d,
	ProjectionPreset,
	RadarSweep,
	RadialDirection,
	RankDirection,
	SpatialProjection,
	SpatialSelectionMapping,
	SpatialSelectionShape,
};

export type GroupMappingPresentation =
	| { type: "none"; label: "Mapping: None"; mapping: null }
	| {
			type: "local";
			label: "Local override";
			mapping: SpatialSelectionMapping;
	  }
	| {
			type: "inherited";
			label: string;
			mapping: SpatialSelectionMapping;
			sourceGroupIds: string[];
	  }
	| {
			type: "mixed";
			label: "Mixed source mappings — source order";
			mapping: null;
	  };

type GroupSource =
	| { type: "explicit"; fixture_ids: string[] }
	| {
			type: "references";
			references: Array<{ group_id: string; rule: { type: string } }>;
	  };

export function storedMapping(
	body: StoredGroup,
): SpatialSelectionMapping | null {
	const mapping = (body as StoredGroup & { mapping?: unknown }).mapping;
	return isSpatialSelectionMapping(mapping) ? mapping : null;
}

export function groupSourceSummary(body: StoredGroup) {
	const source = canonicalSource(body);
	if (source?.type === "references") {
		if (!source.references.length) return "References: none";
		return `References: ${source.references
			.map((reference) => `Group ${reference.group_id}`)
			.join(" · ")}`;
	}
	const count =
		source?.type === "explicit"
			? source.fixture_ids.length
			: body.fixtures.length;
	return `Explicit membership: ${count} ${count === 1 ? "fixture" : "fixtures"}`;
}

export function hasGroupReferenceSource(body: StoredGroup) {
	return canonicalSource(body)?.type === "references";
}

export function resolveMappingPresentation(
	group: Group,
	groups: readonly Group[],
): GroupMappingPresentation {
	const byId = new Map(groups.map((candidate) => [candidate.id, candidate]));
	return resolveMapping(group, byId, new Set());
}

function resolveMapping(
	group: Group,
	groups: ReadonlyMap<string, Group>,
	visiting: Set<string>,
): GroupMappingPresentation {
	const local = storedMapping(group.body);
	if (local) return { type: "local", label: "Local override", mapping: local };
	if (visiting.has(group.id)) return none();
	const source = canonicalSource(group.body);
	if (source?.type !== "references" || !source.references.length) return none();

	const nestedVisiting = new Set(visiting).add(group.id);
	const inherited = source.references.flatMap((reference) => {
		const sourceGroup = groups.get(reference.group_id);
		if (!sourceGroup) return [];
		const presentation = resolveMapping(sourceGroup, groups, nestedVisiting);
		return presentation.mapping
			? [
					{
						groupId: reference.group_id,
						mapping: presentation.mapping,
					},
				]
			: [];
	});
	if (!inherited.length) return none();
	const first = JSON.stringify(inherited[0]?.mapping);
	if (inherited.some(({ mapping }) => JSON.stringify(mapping) !== first))
		return {
			type: "mixed",
			label: "Mixed source mappings — source order",
			mapping: null,
		};
	const sourceGroupIds = inherited.map(({ groupId }) => groupId);
	const mapping = inherited[0]?.mapping;
	if (!mapping) return none();
	return {
		type: "inherited",
		label: `Inherited from ${sourceGroupIds.map((id) => `Group ${id}`).join(" · ")}`,
		mapping,
		sourceGroupIds,
	};
}

export function defaultSpatialMapping(): SpatialSelectionMapping {
	return {
		projection: projectionForPreset("top"),
		shape: { type: "grid", angle_degrees: 0, direction: "ascending" },
	};
}

export function projectionForPreset(
	preset: ProjectionPreset,
): SpatialProjection {
	return {
		anchor: { x: 0, y: 0, z: 0 },
		view_direction:
			preset === "top"
				? { x: 0, y: 0, z: -1 }
				: preset === "front"
					? { x: 0, y: 1, z: 0 }
					: preset === "back"
						? { x: 0, y: -1, z: 0 }
						: preset === "left"
							? { x: 1, y: 0, z: 0 }
							: { x: -1, y: 0, z: 0 },
		rotation_degrees: 0,
		preset,
	};
}

export function validateSpatialMapping(mapping: SpatialSelectionMapping) {
	const values = [
		mapping.projection.anchor.x,
		mapping.projection.anchor.y,
		mapping.projection.anchor.z,
		mapping.projection.view_direction.x,
		mapping.projection.view_direction.y,
		mapping.projection.view_direction.z,
		mapping.projection.rotation_degrees,
		...(mapping.shape.type === "grid"
			? [mapping.shape.angle_degrees]
			: mapping.shape.type === "radial"
				? [mapping.shape.center_u, mapping.shape.center_v]
				: [
						mapping.shape.center_u,
						mapping.shape.center_v,
						mapping.shape.start_angle_degrees,
					]),
	];
	if (values.some((value) => !Number.isFinite(value)))
		return "Every projection and Phase value must be a finite number.";
	const direction = mapping.projection.view_direction;
	if (Math.hypot(direction.x, direction.y, direction.z) <= 1e-12)
		return "Direction must not be zero.";
	return null;
}

function canonicalSource(body: StoredGroup): GroupSource | null {
	const source = (body as StoredGroup & { source?: unknown }).source;
	if (!source || typeof source !== "object") return null;
	const candidate = source as Partial<GroupSource>;
	if (candidate.type === "explicit" && Array.isArray(candidate.fixture_ids))
		return candidate as GroupSource;
	if (candidate.type === "references" && Array.isArray(candidate.references))
		return candidate as GroupSource;
	return null;
}

function isSpatialSelectionMapping(
	value: unknown,
): value is SpatialSelectionMapping {
	if (!value || typeof value !== "object") return false;
	const mapping = value as Partial<SpatialSelectionMapping>;
	if (!mapping.projection || !mapping.shape) return false;
	const projection = mapping.projection as Partial<SpatialProjection>;
	if (
		!(
			isPosition(projection.anchor) &&
			isPosition(projection.view_direction) &&
			typeof projection.rotation_degrees === "number"
		)
	)
		return false;
	const shape = mapping.shape as Partial<SpatialSelectionShape>;
	if (shape.type === "grid")
		return (
			typeof shape.angle_degrees === "number" &&
			["ascending", "descending"].includes(shape.direction ?? "")
		);
	if (shape.type === "radial")
		return (
			typeof shape.center_u === "number" &&
			typeof shape.center_v === "number" &&
			["outward", "inward"].includes(shape.direction ?? "")
		);
	if (shape.type === "radar")
		return (
			typeof shape.center_u === "number" &&
			typeof shape.center_v === "number" &&
			typeof shape.start_angle_degrees === "number" &&
			["clockwise", "counter_clockwise"].includes(shape.sweep ?? "")
		);
	return false;
}

function isPosition(value: unknown): value is Position3d {
	if (!value || typeof value !== "object") return false;
	const position = value as Partial<Position3d>;
	return [position.x, position.y, position.z].every(
		(component) => typeof component === "number",
	);
}

function none(): GroupMappingPresentation {
	return { type: "none", label: "Mapping: None", mapping: null };
}
