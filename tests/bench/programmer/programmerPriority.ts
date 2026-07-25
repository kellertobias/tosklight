import { HttpProgrammerPriorityTransport } from "../../../apps/light-desktop/src/api/ProgrammerPriorityTransport";
import { WireValidationError } from "../../../apps/light-desktop/src/api/wireValidation";
import type { ProgrammerPriorityActionOutcome } from "../../../apps/light-desktop/src/features/programmerPriority/contracts";
import type { ApiDriver } from "../core/api";
import {
	type IntentHttpDependencies,
	intentFetch,
	intentRequestId,
	intentSession,
} from "../core/v2IntentHttp";

export interface SetProgrammerPriorityIntent {
	surface: "api";
	priority: number;
}

export class BrowserProgrammer {
	readonly priority = {
		via: {
			api: {
				set: (priority: number) =>
					setProgrammerPriority(this.api, { surface: "api", priority }),
			},
		},
	};

	constructor(private readonly api: ApiDriver) {}
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
