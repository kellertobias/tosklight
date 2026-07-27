import type {
	DynamicControllerActionOutcome,
	DynamicControllerValueActionRequest,
	DynamicFixAtActionRequest,
	DynamicInstanceActionOutcome,
	DynamicInstanceOverridesProjection,
	DynamicOffActionRequest,
	DynamicRuntimeSnapshotProjection,
	DynamicStartActionRequest,
	DynamicValueTimingProjection,
} from "../generated/light-wire";
import { jsonRequest, type LiveClientTransport } from "./transport";

const NO_TIMING: DynamicValueTimingProjection = {};
const DEFAULT_OVERRIDES: DynamicInstanceOverridesProjection = {
	size: 1,
	speed_multiplier: { numerator: 1, denominator: 1 },
	phase_offset_degrees: 0,
};

export class DynamicsApiClient {
	constructor(private readonly transport: LiveClientTransport) {}

	runtime(showId: string): Promise<DynamicRuntimeSnapshotProjection> {
		return this.transport.request("/api/v2/dynamics/runtime", {
			headers: { "x-tosk-show": showId },
		});
	}

	start(
		showId: string,
		dynamicId: string,
		targets: string[] = [],
		overrides: DynamicInstanceOverridesProjection = DEFAULT_OVERRIDES,
		timing: DynamicValueTimingProjection = NO_TIMING,
	): Promise<DynamicInstanceActionOutcome> {
		const request: DynamicStartActionRequest = {
			request_id: crypto.randomUUID(),
			targets,
			overrides,
			timing,
		};
		return this.post(
			`/api/v2/dynamics/${encodeURIComponent(dynamicId)}/start`,
			showId,
			request,
		);
	}

	toggle(
		dynamicId: string,
		targets: string[] = [],
		overrides: DynamicInstanceOverridesProjection = DEFAULT_OVERRIDES,
		timing: DynamicValueTimingProjection = NO_TIMING,
	): Promise<DynamicInstanceActionOutcome> {
		const requestId = crypto.randomUUID();
		const request: DynamicStartActionRequest = {
			request_id: requestId,
			targets,
			overrides,
			timing,
		};
		return this.transport.sendAction(
			{
				type: "dynamic_toggle",
				request: { dynamic_id: dynamicId, request },
			},
			requestId,
		) as Promise<DynamicInstanceActionOutcome>;
	}

	off(
		showId: string,
		controllerId: string,
		timing: DynamicValueTimingProjection = NO_TIMING,
	): Promise<DynamicInstanceActionOutcome> {
		const request: DynamicOffActionRequest = {
			request_id: crypto.randomUUID(),
			timing,
		};
		return this.post(
			`/api/v2/dynamic-instances/${encodeURIComponent(controllerId)}/off`,
			showId,
			request,
		);
	}

	offLive(
		controllerId: string,
		timing: DynamicValueTimingProjection = NO_TIMING,
	): Promise<DynamicInstanceActionOutcome> {
		const requestId = crypto.randomUUID();
		const request: DynamicOffActionRequest = {
			request_id: requestId,
			timing,
		};
		return this.transport.sendAction(
			{
				type: "dynamic_off",
				request: { controller_id: controllerId, request },
			},
			requestId,
		) as Promise<DynamicInstanceActionOutcome>;
	}

	setControllerValue(
		showId: string,
		controllerId: string,
		field: "size" | "speed" | "phase",
		value: number,
		undoGroup?: string,
	): Promise<DynamicControllerActionOutcome> {
		const request: DynamicControllerValueActionRequest = {
			request_id: crypto.randomUUID(),
			value,
			undo_group: undoGroup ?? null,
		};
		return this.post(
			`/api/v2/dynamic-instances/${encodeURIComponent(controllerId)}/${field}`,
			showId,
			request,
		);
	}

	setControllerValueLive(
		controllerId: string,
		field: "size" | "speed" | "phase",
		value: number,
		undoGroup?: string,
	): Promise<DynamicControllerActionOutcome> {
		const requestId = crypto.randomUUID();
		const request: DynamicControllerValueActionRequest = {
			request_id: requestId,
			value,
			undo_group: undoGroup ?? null,
		};
		return this.transport.sendAction(
			{
				type: `dynamic_${field}`,
				request: { controller_id: controllerId, request },
			},
			requestId,
		) as Promise<DynamicControllerActionOutcome>;
	}

	fixAt(
		showId: string,
		request: Omit<DynamicFixAtActionRequest, "request_id">,
	): Promise<DynamicControllerActionOutcome> {
		return this.post("/api/v2/programmer/values/fix-at", showId, {
			...request,
			request_id: crypto.randomUUID(),
		});
	}

	fixAtLive(
		request: Omit<DynamicFixAtActionRequest, "request_id">,
	): Promise<DynamicControllerActionOutcome> {
		const requestId = crypto.randomUUID();
		return this.transport.sendAction(
			{
				type: "dynamic_fix_at",
				request: { ...request, request_id: requestId },
			},
			requestId,
		) as Promise<DynamicControllerActionOutcome>;
	}

	private post<T>(path: string, showId: string, body: unknown): Promise<T> {
		const request = jsonRequest("POST", body);
		return this.transport.request<T>(path, {
			...request,
			headers: {
				...request.headers,
				"x-tosk-show": showId,
			},
		});
	}
}
