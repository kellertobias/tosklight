import { decodeExactAddress } from "./playbackWireProjection";
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
	const kind = enumAt(owner.kind, `${path}.kind`, ["playback"]);
	exactRecordAt(owner, path, ["kind", "page", "position"]);
	return {kind, ...decodeExactAddress(owner, path)};
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
): PlaybackActionRequest {
	const target = decodeDynamicRuntimeStopOwner(owner, "$.stop_owner");
	return {
		address: target,
		control_source: {surface_id: "running-output", control_id: 0},
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
