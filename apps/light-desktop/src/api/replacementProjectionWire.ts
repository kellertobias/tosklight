import {
	arrayAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";
import type {
	ReplacementProgramProjection,
	ReplacementProfileContext,
} from "./generated/light-wire";

function uuidAt(value: unknown, path: string): string {
	const id = stringAt(value, path);
	if (
		!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
			id,
		) ||
		/^0{8}-0{4}-0{4}-0{4}-0{12}$/.test(id)
	)
		throw new WireValidationError(path, "non-nil UUID", value);
	return id;
}
function contextAt(value: unknown, path: string): ReplacementProfileContext {
	const body = exactRecordAt(value, path, [
		"profile_id",
		"profile_revision",
		"mode_id",
	]);
	const revision = integerAt(body.profile_revision, `${path}.profile_revision`);
	if (revision === 0)
		throw new WireValidationError(path, "positive profile revision", value);
	return {
		profile_id: uuidAt(body.profile_id, `${path}.profile_id`),
		profile_revision: revision,
		mode_id: uuidAt(body.mode_id, `${path}.mode_id`),
	};
}
export function decodeReplacementProjection(
	value: unknown,
	path: string,
	owner?: string,
): ReplacementProgramProjection {
	const body = exactRecordAt(value, path, [
		"source_owner",
		"source_profile",
		"source_head_id",
		"target_profile",
		"targets",
	]);
	const source = uuidAt(body.source_owner, `${path}.source_owner`);
	if (owner && owner !== source)
		throw new WireValidationError(
			path,
			"projection of original authored owner",
			value,
		);
	const fixtures = new Set<string>();
	const heads = new Set<string>();
	const targets = arrayAt(body.targets, `${path}.targets`).map(
		(value, index) => {
			const targetPath = `${path}.targets[${index}]`;
			const target = exactRecordAt(value, targetPath, [
				"profile_head_id",
				"fixture_id",
			]);
			const fixture = uuidAt(target.fixture_id, `${targetPath}.fixture_id`);
			const head = uuidAt(
				target.profile_head_id,
				`${targetPath}.profile_head_id`,
			);
			if (fixtures.has(fixture) || heads.has(head))
				throw new WireValidationError(
					targetPath,
					"unique destination head and fixture",
					value,
				);
			fixtures.add(fixture);
			heads.add(head);
			return { fixture_id: fixture, profile_head_id: head };
		},
	);
	// Explicit empty destinations are consented dormant programming; they are not absent metadata.
	return {
		source_owner: source,
		source_profile: contextAt(body.source_profile, `${path}.source_profile`),
		source_head_id: uuidAt(body.source_head_id, `${path}.source_head_id`),
		target_profile: contextAt(body.target_profile, `${path}.target_profile`),
		targets,
	};
}
export function decodeReplacementMap(
	value: unknown,
	path: string,
): Record<string, ReplacementProgramProjection> {
	return Object.fromEntries(
		Object.entries(recordAt(value, path)).map(([owner, projection]) => [
			uuidAt(owner, `${path}.owner`),
			decodeReplacementProjection(projection, `${path}.${owner}`, owner),
		]),
	);
}

export function decodePresetReplacementFields(
	body: Record<string, unknown>,
	path: string,
) {
	const fixture = body.fixture_replacement_projections;
	const group = body.group_replacement_projections;
	return {
		...(fixture === undefined
			? {}
			: {
					fixture_replacement_projections: Object.fromEntries(
						Object.entries(
							recordAt(fixture, `${path}.fixture_replacement_projections`),
						).map(([owner, values]) => [
							owner,
							Object.fromEntries(
								Object.entries(
									recordAt(
										values,
										`${path}.fixture_replacement_projections.${owner}`,
									),
								).map(([attribute, projection]) => [
									attribute,
									decodeReplacementProjection(
										projection,
										`${path}.fixture_replacement_projections.${owner}.${attribute}`,
										owner,
									),
								]),
							),
						]),
					),
				}),
		...(group === undefined
			? {}
			: {
					group_replacement_projections: Object.fromEntries(
						Object.entries(
							recordAt(group, `${path}.group_replacement_projections`),
						).map(([owner, values]) => [
							owner,
							Object.fromEntries(
								Object.entries(
									recordAt(
										values,
										`${path}.group_replacement_projections.${owner}`,
									),
								).map(([attribute, map]) => [
									attribute,
									decodeReplacementMap(
										map,
										`${path}.group_replacement_projections.${owner}.${attribute}`,
									),
								]),
							),
						]),
					),
				}),
	};
}
