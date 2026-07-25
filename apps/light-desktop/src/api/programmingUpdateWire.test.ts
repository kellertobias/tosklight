import { describe, expect, it } from "vitest";
import type {
	ProgrammingUpdateActionRequest,
	ProgrammingUpdatePreviewRequest,
	ProgrammingUpdateTargetsRequest,
} from "./generated/light-wire";
import {
	decodeProgrammingUpdateActionOutcome,
	decodeProgrammingUpdateErrorResponse,
	decodeProgrammingUpdatePreviewResponse,
	decodeProgrammingUpdateSettingsProjection,
	decodeProgrammingUpdateTargetsResponse,
	encodeProgrammingUpdateActionRequest,
	encodeProgrammingUpdatePreviewRequest,
} from "./programmingUpdateWire";
import { WireValidationError } from "./wireValidation";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const CUE_LIST_ID = "22222222-2222-4222-8222-222222222222";
const CUE_ID = "33333333-3333-4333-8333-333333333333";
const FIXTURE_ID = "44444444-4444-4444-8444-444444444444";
const CORRELATION_ID = "55555555-5555-4555-8555-555555555555";
const DESK_ID = "66666666-6666-4666-8666-666666666666";
const FINGERPRINT = "a".repeat(64);

function groupRequest(): ProgrammingUpdatePreviewRequest {
	return {
		request_id: "preview-group",
		target: { type: "group", object_id: "front" },
		mode: { target_type: "existing_content", mode: "add_new" },
	};
}

function groupPreview(mode: "update_existing" | "add_new" = "add_new") {
	return {
		target: {
			family: { type: "group" },
			object_id: "front",
			name: "Front",
		},
		mode: { target_type: "existing_content", mode },
		items: [
			{
				address: { type: "group_membership", fixture_id: FIXTURE_ID },
				outcome: {
					outcome: mode === "add_new" ? "add_new" : "update_existing",
				},
			},
		],
	};
}

function groupPreviewResponse() {
	return {
		request_id: "preview-group",
		correlation_id: CORRELATION_ID,
		show_id: SHOW_ID,
		show_revision: 10,
		object: { kind: "group", object_id: "front", object_revision: 4 },
		programmer_revision: FINGERPRINT,
		preview: groupPreview(),
	};
}

function actionRequest(): ProgrammingUpdateActionRequest {
	return {
		request_id: "apply-group",
		action: {
			type: "confirm_preview",
			target: { type: "group", object_id: "front" },
			mode: { target_type: "existing_content", mode: "add_new" },
			expected_object_revision: 4,
			expected_programmer_revision: FINGERPRINT,
		},
	};
}

function actionOutcome() {
	return {
		status: "changed",
		request_id: "apply-group",
		correlation_id: CORRELATION_ID,
		replayed: false,
		show_id: SHOW_ID,
		show_revision: 11,
		projection: {
			kind: "group",
			object_id: "front",
			object_revision: 5,
			body: {
				id: "front",
				name: "Front",
				fixtures: [FIXTURE_ID],
				future_group: { retained: true },
			},
		},
		event_sequence: 42,
		summary: {
			target: {
				family: { type: "group" },
				object_id: "front",
				name: "Front",
			},
			revision_before: 4,
			revision_after: 5,
			eligible_count: 1,
			changed_count: 1,
			added_count: 1,
			ignored_count: 0,
			changed_cues: [],
			programmer_values_retained: true,
		},
	};
}

describe("Programming Update v2 wire", () => {
	it("decodes a strict preview and rejects request, scope, correlation, and nested extras", () => {
		expect(
			decodeProgrammingUpdatePreviewResponse(
				groupPreviewResponse(),
				SHOW_ID,
				groupRequest(),
			),
		).toMatchObject({ show_revision: 10, programmer_revision: FINGERPRINT });

		for (const mutate of [
			(value: ReturnType<typeof groupPreviewResponse>) => {
				value.request_id = "foreign-request";
			},
			(value: ReturnType<typeof groupPreviewResponse>) => {
				value.show_id = CUE_LIST_ID;
			},
			(value: ReturnType<typeof groupPreviewResponse>) => {
				value.correlation_id = "not-a-uuid";
			},
			(value: ReturnType<typeof groupPreviewResponse>) => {
				(value.preview.target as Record<string, unknown>).foreign = true;
			},
		]) {
			const value = groupPreviewResponse();
			mutate(value);
			expect(() =>
				decodeProgrammingUpdatePreviewResponse(value, SHOW_ID, groupRequest()),
			).toThrow(WireValidationError);
		}
	});

	it("rejects mismatched modes and non-printable or oversized request object IDs", () => {
		const mismatched = groupRequest();
		mismatched.mode = { target_type: "cue", mode: "add_new" };
		expect(() => encodeProgrammingUpdatePreviewRequest(mismatched)).toThrow(
			WireValidationError,
		);

		for (const objectId of ["front\u0000", "ü".repeat(129)]) {
			const invalid = groupRequest();
			invalid.target = { type: "group", object_id: objectId };
			expect(() => encodeProgrammingUpdatePreviewRequest(invalid)).toThrow(
				WireValidationError,
			);
		}
	});

	it("accepts semantic Cue IDs that differ from exact legacy storage keys", () => {
		const request: ProgrammingUpdatePreviewRequest = {
			request_id: "cue-preview",
			target: {
				type: "cue",
				cue_list_id: CUE_LIST_ID,
				playback_number: 7,
				cue_id: CUE_ID,
				cue_number: 2,
				validate_active_context: true,
			},
			mode: { target_type: "cue", mode: "existing_only" },
		};
		const preview = cuePreview("existing_only");
		const response = {
			request_id: request.request_id,
			correlation_id: CORRELATION_ID,
			show_id: SHOW_ID,
			show_revision: 3,
			object: {
				kind: "cue_list",
				object_id: "legacy-storage-key",
				object_revision: 8,
			},
			programmer_revision: FINGERPRINT,
			preview,
		};
		expect(
			decodeProgrammingUpdatePreviewResponse(response, SHOW_ID, request).object
				.object_id,
		).toBe("legacy-storage-key");

		const menuRequest: ProgrammingUpdateTargetsRequest = {
			request_id: "cue-menu",
			filter: "show_all_active",
		};
		const menu = {
			request_id: menuRequest.request_id,
			correlation_id: CORRELATION_ID,
			show_id: SHOW_ID,
			show_revision: 3,
			targets: [
				{
					request_target: request.target,
					object: response.object,
					programmer_revision: FINGERPRINT,
					active_or_referenced: true,
					existing_preview: preview,
					add_new_preview: cuePreview("add_new"),
				},
			],
		};
		const secondPlayback = structuredClone(menu.targets[0]);
		if (secondPlayback.request_target.type !== "cue")
			throw new Error("test requires a Cue target");
		secondPlayback.request_target.playback_number = 8;
		secondPlayback.existing_preview.target.playback_number = 8;
		secondPlayback.add_new_preview.target.playback_number = 8;
		menu.targets.push(secondPlayback);
		expect(
			decodeProgrammingUpdateTargetsResponse(menu, SHOW_ID, menuRequest)
				.targets[0].object.object_id,
		).toBe("legacy-storage-key");
		expect(
			decodeProgrammingUpdateTargetsResponse(menu, SHOW_ID, menuRequest)
				.targets,
		).toHaveLength(2);
	});

	it("validates the committed projection body and one-revision action outcome", () => {
		const decoded = decodeProgrammingUpdateActionOutcome(
			actionOutcome(),
			SHOW_ID,
			10,
			actionRequest(),
		);
		expect(decoded.projection.body).toMatchObject({
			fixtures: [FIXTURE_ID],
			future_group: { retained: true },
		});

		const malformed = actionOutcome();
		malformed.projection.body.fixtures = [17 as unknown as string];
		expect(() =>
			decodeProgrammingUpdateActionOutcome(
				malformed,
				SHOW_ID,
				10,
				actionRequest(),
			),
		).toThrow(WireValidationError);

		const inconsistent = actionOutcome();
		inconsistent.summary.revision_after = 6;
		expect(() =>
			decodeProgrammingUpdateActionOutcome(
				inconsistent,
				SHOW_ID,
				10,
				actionRequest(),
			),
		).toThrow(WireValidationError);
	});

	it("requires lowercase SHA-256 Programmer fingerprints", () => {
		const invalid = actionRequest();
		if (invalid.action.type !== "confirm_preview")
			throw new Error("test requires a confirmed preview action");
		invalid.action.expected_programmer_revision = "A".repeat(64);
		expect(() => encodeProgrammingUpdateActionRequest(invalid)).toThrow(
			WireValidationError,
		);

		const response = groupPreviewResponse();
		response.programmer_revision = "short";
		expect(() =>
			decodeProgrammingUpdatePreviewResponse(response, SHOW_ID, groupRequest()),
		).toThrow(WireValidationError);
	});

	it("strictly decodes settings and typed errors", () => {
		expect(
			decodeProgrammingUpdateSettingsProjection(
				{
					desk_id: DESK_ID,
					settings: settings(),
				},
				DESK_ID,
			),
		).toMatchObject({ desk_id: DESK_ID });
		expect(() =>
			decodeProgrammingUpdateSettingsProjection(
				{
					desk_id: SHOW_ID,
					settings: settings(),
				},
				DESK_ID,
			),
		).toThrow(WireValidationError);

		expect(
			decodeProgrammingUpdateErrorResponse({
				kind: "conflict",
				error: "stale revision",
				current_object_revision: 5,
				current_show_revision: 11,
				retryable: false,
			}),
		).toMatchObject({ current_show_revision: 11 });
		expect(() =>
			decodeProgrammingUpdateErrorResponse({
				kind: "conflict",
				error: "stale revision",
				retryable: false,
				foreign: true,
			}),
		).toThrow(WireValidationError);
	});
});

function cuePreview(mode: "existing_only" | "add_new") {
	return {
		target: {
			family: { type: "cue" },
			object_id: CUE_LIST_ID,
			name: "Main",
			playback_number: 7,
			cue: { id: CUE_ID, number: 2 },
		},
		mode: { target_type: "cue", mode },
		items: [],
	};
}

function settings() {
	return {
		cue_mode: "existing_only",
		preset_mode: "update_existing",
		group_mode: "add_new",
		show_update_modal_on_touch: true,
	};
}
