import { HttpProgrammerPriorityTransport } from "../../src/api/ProgrammerPriorityTransport";
import { WireValidationError } from "../../src/api/wireValidation";
import type { ProgrammerPriorityActionOutcome } from "../../src/features/programmerPriority/contracts";
import type { ApiDriver } from "./api";
import {
	type IntentHttpDependencies,
	intentFetch,
	intentRequestId,
	intentSession,
} from "./v2IntentHttp";

export interface SetProgrammerPriorityIntent {
	surface: "api";
	priority: number;
}

export async function setProgrammerPriority(
	api: ApiDriver,
	intent: SetProgrammerPriorityIntent,
	dependencies: IntentHttpDependencies = {},
): Promise<ProgrammerPriorityActionOutcome> {
	validateIntent(intent);
	const session = intentSession(api);
	const scope = { userId: session.user.id };
	const transport = new HttpProgrammerPriorityTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
		authenticatedUserId: session.user.id,
		fetch: intentFetch(dependencies),
	});
	const snapshot = await transport.loadSnapshot(scope);
	return transport.applyAction(scope, {
		requestId: intentRequestId(dependencies),
		expectedRevision: snapshot.projection.revision,
		priority: intent.priority,
	});
}

function validateIntent(intent: SetProgrammerPriorityIntent) {
	if (intent.surface !== "api")
		throw new Error("Programmer priority helper supports only the API surface");
	if (
		!Number.isSafeInteger(intent.priority) ||
		intent.priority < -32_768 ||
		intent.priority > 32_767
	)
		throw new WireValidationError(
			"$.priority",
			"signed 16-bit priority",
			intent.priority,
		);
}
