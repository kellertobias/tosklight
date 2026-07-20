import type {
	UpdateMenuEntry,
	UpdateMode,
	UpdatePreview,
	UpdateResult,
	UpdateSettings,
	UpdateTargetFilter,
	UpdateTargetRequest,
} from "../types";
import type { ClientTransport } from "./transport";

export class UpdateApiClient {
	constructor(private readonly transport: ClientTransport) {}

	updateSettings(): Promise<UpdateSettings> {
		return this.transport.request("/api/v1/update/settings");
	}

	saveUpdateSettings(settings: UpdateSettings): Promise<UpdateSettings> {
		return this.transport.request("/api/v1/update/settings", {
			method: "PUT",
			headers: { "content-type": "application/json" },
			body: JSON.stringify(settings),
		});
	}

	previewUpdate(target: UpdateTargetRequest, mode: UpdateMode) {
		return this.transport.request<UpdatePreview>("/api/v1/update/preview", {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ target, mode }),
		});
	}

	applyUpdate(
		target: UpdateTargetRequest,
		mode: UpdateMode,
		expectedRevision?: number,
		expectedProgrammerRevision?: string,
		expectedShowRevision?: number,
	) {
		return this.transport.request<UpdateResult>("/api/v1/update/apply", {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({
				target,
				mode,
				...(expectedRevision == null
					? {}
					: { expected_revision: expectedRevision }),
				...(expectedProgrammerRevision == null
					? {}
					: { expected_programmer_revision: expectedProgrammerRevision }),
				...(expectedShowRevision == null
					? {}
					: { expected_show_revision: expectedShowRevision }),
			}),
		});
	}

	updateTargets(filter: UpdateTargetFilter): Promise<UpdateMenuEntry[]> {
		return this.transport.request(
			`/api/v1/update/targets?filter=${encodeURIComponent(filter)}`,
		);
	}
}
