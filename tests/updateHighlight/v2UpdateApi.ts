import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";

interface LegacyTarget {
	family: { type: "cue" | "preset" | "group" };
	object_id: string;
	playback_number?: number;
	cue_id?: string;
	cue_number?: number;
	validate_active_context?: boolean;
}

interface LegacyUpdateRequest {
	target: LegacyTarget;
	mode: {
		target_type: "cue" | "existing_content";
		mode: string;
	};
	expected_revision?: number;
}

export async function programmingUpdateSettings(api: ApiDriver) {
	const deskId = requiredDeskId(api);
	const response = await api.request<{ settings: unknown }>(
		"GET",
		`/api/v2/desks/${deskId}/programming-update/settings`,
	);
	return response.settings;
}

export async function previewProgrammingUpdate(
	api: ApiDriver,
	request: LegacyUpdateRequest,
) {
	const showId = await activeShowId(api);
	const response = await api.request<any>(
		"POST",
		"/api/v2/programming-update/preview",
		{
			request_id: crypto.randomUUID(),
			target: target(request.target),
			mode: request.mode,
		},
		true,
		undefined,
		{ showId },
	);
	return response.preview;
}

export async function applyProgrammingUpdate(
	api: ApiDriver,
	request: LegacyUpdateRequest,
) {
	const showId = await activeShowId(api);
	const preview = await api.request<any>(
		"POST",
		"/api/v2/programming-update/preview",
		{
			request_id: crypto.randomUUID(),
			target: target(request.target),
			mode: request.mode,
		},
		true,
		undefined,
		{ showId },
	);
	const outcome = await api.request<any>(
		"POST",
		"/api/v2/programming-update/actions",
		{
			request_id: crypto.randomUUID(),
			action: {
				type: "confirm_preview",
				target: target(request.target),
				mode: request.mode,
				expected_object_revision: preview.object.object_revision,
				expected_programmer_revision: preview.programmer_revision,
			},
		},
		true,
		preview.show_revision,
		{ showId },
	);
	return outcome.summary;
}

function target(input: LegacyTarget) {
	if (input.family.type === "cue") {
		return {
			type: "cue",
			cue_list_id: input.object_id,
			playback_number: input.playback_number ?? null,
			cue_id: input.cue_id ?? null,
			cue_number: input.cue_number ?? null,
			validate_active_context: input.validate_active_context ?? false,
		};
	}
	return { type: input.family.type, object_id: input.object_id };
}

async function activeShowId(api: ApiDriver) {
	const bootstrap = await api.request<{ active_show: { id: string } | null }>(
		"GET",
		"/api/v2/bootstrap",
		undefined,
		false,
	);
	if (!bootstrap.active_show) throw new Error("No active show");
	return bootstrap.active_show.id;
}

function requiredDeskId(api: ApiDriver) {
	if (!api.session) throw new Error("API session is not initialized");
	return api.session.desk.id;
}
