import type {
	SpeedGroupAction,
	SpeedGroupActionRequest,
} from "../features/speedGroupRuntime/contracts";
import {
	assertAction,
	assertRequestId,
} from "../features/speedGroupRuntime/projectionValue";
import type { SpeedGroupActionRequest as WireSpeedGroupActionRequest } from "./generated/light-wire";
import { integerAt } from "./playbackWirePrimitives";
import { programmingUuidAt } from "./programmingWireProjection";
import { WireValidationError } from "./wireValidation";

export function encodeSpeedGroupActionRequest(
	request: SpeedGroupActionRequest,
): WireSpeedGroupActionRequest {
	try {
		assertRequestId(request.requestId);
		assertAction(request.action);
	} catch {
		throw new WireValidationError("$", "valid Speed Group action", request);
	}
	programmingUuidAt(request.expectedAuthorityId, "$.expectedAuthorityId");
	integerAt(request.expectedRevision, "$.expectedRevision");
	return {
		request_id: request.requestId,
		expected_authority_id: request.expectedAuthorityId,
		expected_revision: request.expectedRevision,
		action: actionToWire(request.action),
	};
}

function actionToWire(action: SpeedGroupAction) {
	if (action.type === "set_bpm") return { ...action };
	if (action.type === "adjust_bpm")
		return {
			type: action.type,
			group: action.group,
			delta_bpm: action.deltaBpm,
		};
	return { ...action };
}
