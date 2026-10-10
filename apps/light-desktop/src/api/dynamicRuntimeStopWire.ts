import type {
	DynamicRuntimeSnapshotProjection,
	DynamicRuntimeStopOwner,
	PlaybackActionRequest,
} from "./generated/light-wire";
import {
	arrayAt,
	enumAt,
	exactRecordAt,
	positiveIntegerAt,
	recordAt,
} from "./playbackWirePrimitives";
import { programmerValuesUuidAt } from "./programmerValuesWireProjection";
import { WireValidationError } from "./wireValidation";

export interface DynamicRuntimeStopIdentity {
	dynamicId: string;
	instanceId: string;
	controllerId: string;
}

export function decodeDynamicRuntimeStopOwner(
	value: unknown,
	path: string,
): DynamicRuntimeStopOwner {
	const owner = recordAt(value, path);
	const kind = enumAt(owner.kind, `${path}.kind`, [
		"physical_playback",
		"virtual_playback",
	]);
	exactRecordAt(
		value,
		path,
		kind === "virtual_playback"
			? ["kind", "page", "playback_number"]
			: ["kind", "playback_number"],
	);
	const number = positiveIntegerAt(
		owner.playback_number,
		`${path}.playback_number`,
	);
	if (kind === "physical_playback") {
		if (number > 1000)
			throw new WireValidationError(
				`${path}.playback_number`,
				"physical playback within 1-1000",
				number,
			);
		return { kind, playback_number: number };
	}
	const page = positiveIntegerAt(owner.page, `${path}.page`);
	if (page > 127)
		throw new WireValidationError(
			`${path}.page`,
			"virtual page within 1-127",
			page,
		);
	const first = 1001 + (page - 1) * 300;
	if (number < first || number > first + 299)
		throw new WireValidationError(
			`${path}.playback_number`,
			`virtual playback within page ${page} (${first}-${first + 299})`,
			number,
		);
	return { kind, page, playback_number: number };
}

/** Validate additive owner metadata at the existing runtime read boundary. */
export function decodeDynamicRuntimeStopMetadata(
	value: unknown,
): DynamicRuntimeSnapshotProjection {
	const snapshot = recordAt(value, "$");
	const instances = arrayAt(snapshot.instances, "$.instances");
	for (const [index, item] of instances.entries()) {
		const instance = recordAt(item, `$.instances[${index}]`);
		const controllers = arrayAt(
			instance.controllers,
			`$.instances[${index}].controllers`,
		);
		for (const [controllerIndex, item] of controllers.entries()) {
			const controller = recordAt(
				item,
				`$.instances[${index}].controllers[${controllerIndex}]`,
			);
			if (controller.stop_owner !== undefined)
				decodeDynamicRuntimeStopOwner(
					controller.stop_owner,
					`$.instances[${index}].controllers[${controllerIndex}].stop_owner`,
				);
		}
	}
	return value as DynamicRuntimeSnapshotProjection;
}

export function dynamicRuntimeStopRequest(
	owner: DynamicRuntimeStopOwner,
	identity: DynamicRuntimeStopIdentity,
	requestId: string,
): PlaybackActionRequest {
	const target = decodeDynamicRuntimeStopOwner(owner, "$.stop_owner");
	return {
		request_id: requestId,
		address:
			target.kind === "physical_playback"
				? { kind: "playback", playback_number: target.playback_number }
				: {
						kind: "virtual",
						page: target.page,
						playback_number: target.playback_number,
					},
		surface: target.kind === "physical_playback" ? "physical" : "virtual",
		action: {
			type: "runtime_stop_dynamic",
			dynamic_id: programmerValuesUuidAt(identity.dynamicId, "$.dynamic_id"),
			instance_id: programmerValuesUuidAt(identity.instanceId, "$.instance_id"),
			controller_id: programmerValuesUuidAt(
				identity.controllerId,
				"$.controller_id",
			),
		},
	};
}

export type { DynamicRuntimeStopOwner } from "./generated/light-wire";
