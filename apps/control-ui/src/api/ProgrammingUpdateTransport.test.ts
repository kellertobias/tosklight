import { describe, expect, it, vi } from "vitest";
import type {
	ProgrammingUpdateActionRequest,
	ProgrammingUpdatePreviewRequest,
	ProgrammingUpdateSettings,
	ProgrammingUpdateTargetsRequest,
} from "./generated/light-wire";
import {
	HttpProgrammingUpdateTransport,
	type ProgrammingUpdateHttpError,
} from "./ProgrammingUpdateTransport";
import { WireValidationError } from "./wireValidation";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const DESK_ID = "22222222-2222-4222-8222-222222222222";
const FIXTURE_ID = "33333333-3333-4333-8333-333333333333";
const CORRELATION_ID = "44444444-4444-4444-8444-444444444444";
const FINGERPRINT = "a".repeat(64);

function previewRequest(): ProgrammingUpdatePreviewRequest {
	return {
		request_id: "preview-front",
		target: { type: "group", object_id: "front" },
		mode: { target_type: "existing_content", mode: "add_new" },
	};
}

function targetsRequest(): ProgrammingUpdateTargetsRequest {
	return { request_id: "targets-front", filter: "show_all_active" };
}

function actionRequest(): ProgrammingUpdateActionRequest {
	return {
		request_id: "apply-front",
		action: {
			type: "apply_direct",
			target: { type: "group", object_id: "front" },
			mode: { target_type: "existing_content", mode: "add_new" },
		},
	};
}

function preview(mode: "update_existing" | "add_new" = "add_new") {
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

function previewResponse() {
	return {
		request_id: "preview-front",
		correlation_id: CORRELATION_ID,
		show_id: SHOW_ID,
		show_revision: 10,
		object: { kind: "group", object_id: "front", object_revision: 4 },
		programmer_revision: FINGERPRINT,
		preview: preview(),
	};
}

function targetsResponse() {
	return {
		request_id: "targets-front",
		correlation_id: CORRELATION_ID,
		show_id: SHOW_ID,
		show_revision: 10,
		targets: [
			{
				request_target: { type: "group", object_id: "front" },
				object: { kind: "group", object_id: "front", object_revision: 4 },
				programmer_revision: FINGERPRINT,
				active_or_referenced: true,
				existing_preview: preview("update_existing"),
				add_new_preview: preview(),
			},
		],
	};
}

function actionOutcome() {
	return {
		status: "changed",
		request_id: "apply-front",
		correlation_id: CORRELATION_ID,
		replayed: false,
		show_id: SHOW_ID,
		show_revision: 11,
		projection: {
			kind: "group",
			object_id: "front",
			object_revision: 5,
			body: { id: "front", name: "Front", fixtures: [FIXTURE_ID] },
		},
		event_sequence: 19,
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

function settings(): ProgrammingUpdateSettings {
	return {
		cue_mode: "existing_only",
		preset_mode: "update_existing",
		group_mode: "add_new",
		show_update_modal_on_touch: true,
	};
}

function response(value: unknown, status = 200, etag?: string) {
	return new Response(JSON.stringify(value), {
		status,
		headers: etag ? { etag } : undefined,
	});
}

function harness(fetch = vi.fn<typeof globalThis.fetch>()) {
	return {
		fetch,
		transport: new HttpProgrammingUpdateTransport({
			baseUrl: "http://desk.local/",
			sessionToken: "session-token",
			deskBoundaryToken: "desk-token",
			fetch,
		}),
	};
}

describe("Programming Update v2 HTTP adapter", () => {
	it("is dormant until preview or target data is explicitly requested", async () => {
		const fetch = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValueOnce(response(previewResponse(), 200, '"10"'))
			.mockResolvedValueOnce(response(targetsResponse(), 200, '"10"'));
		const { transport } = harness(fetch);
		expect(fetch).not.toHaveBeenCalled();

		await expect(
			transport.preview(SHOW_ID, previewRequest()),
		).resolves.toMatchObject({
			show_revision: 10,
		});
		await expect(
			transport.targets(SHOW_ID, targetsRequest()),
		).resolves.toMatchObject({
			targets: [{ programmer_revision: FINGERPRINT }],
		});

		expect(fetch.mock.calls.map(([url]) => String(url))).toEqual([
			`http://desk.local/api/v2/shows/${SHOW_ID}/programming-update/preview`,
			`http://desk.local/api/v2/shows/${SHOW_ID}/programming-update/targets`,
		]);
		for (const [, init] of fetch.mock.calls) {
			const headers = init?.headers as Headers;
			expect(headers.get("authorization")).toBe("Bearer session-token");
			expect(headers.get("x-light-desk-token")).toBe("desk-token");
			expect(headers.get("if-match")).toBeNull();
		}
		expect(fetch.mock.calls.flat().join(" ")).not.toMatch(/bootstrap|api\/v1/);
	});

	it("sends one action with a quoted If-Match and verifies its authoritative ETag", async () => {
		const fetch = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValue(response(actionOutcome(), 200, '"11"'));
		const { transport } = harness(fetch);

		await expect(
			transport.apply(SHOW_ID, 10, actionRequest()),
		).resolves.toMatchObject({
			status: "changed",
			show_revision: 11,
			event_sequence: 19,
		});
		expect(fetch).toHaveBeenCalledOnce();
		const [url, init] = fetch.mock.calls[0];
		expect(String(url)).toContain("/programming-update/actions");
		const headers = init?.headers as Headers;
		expect(headers.get("if-match")).toBe('"10"');
		expect(JSON.parse(String(init?.body))).toEqual(actionRequest());
	});

	it("rejects missing and mismatched successful ETags", async () => {
		for (const etag of [undefined, '"10"']) {
			const fetch = vi
				.fn<typeof globalThis.fetch>()
				.mockResolvedValue(response(actionOutcome(), 200, etag));
			const { transport } = harness(fetch);
			await expect(
				transport.apply(SHOW_ID, 10, actionRequest()),
			).rejects.toBeInstanceOf(WireValidationError);
		}
	});

	it("surfaces exact conflict revisions only with the matching error ETag", async () => {
		const conflict = {
			kind: "conflict",
			error: "stale object revision",
			current_object_revision: 5,
			current_show_revision: 12,
			retryable: false,
		};
		const fetch = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValue(response(conflict, 409, '"12"'));
		const { transport } = harness(fetch);
		await expect(transport.apply(SHOW_ID, 10, actionRequest())).rejects.toEqual(
			expect.objectContaining<Partial<ProgrammingUpdateHttpError>>({
				name: "ProgrammingUpdateHttpError",
				kind: "conflict",
				status: 409,
				currentObjectRevision: 5,
				currentShowRevision: 12,
				retryable: false,
			}),
		);
	});

	it("rejects mismatched typed-error status and revision headers", async () => {
		const conflict = {
			kind: "conflict",
			error: "stale object revision",
			current_show_revision: 12,
			retryable: false,
		};
		for (const result of [
			response(conflict, 409),
			response(conflict, 409, '"11"'),
			response({ ...conflict, kind: "unavailable" }, 409, '"12"'),
		]) {
			const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(result);
			const { transport } = harness(fetch);
			await expect(
				transport.apply(SHOW_ID, 10, actionRequest()),
			).rejects.toBeInstanceOf(WireValidationError);
		}
	});

	it("loads and saves settings only for the exact desk scope", async () => {
		const projection = { desk_id: DESK_ID, settings: settings() };
		const fetch = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValueOnce(response(projection))
			.mockResolvedValueOnce(response(projection));
		const { transport } = harness(fetch);

		await expect(transport.loadSettings(DESK_ID)).resolves.toEqual(projection);
		await expect(transport.saveSettings(DESK_ID, settings())).resolves.toEqual(
			projection,
		);
		const [getUrl, getInit] = fetch.mock.calls[0];
		const [putUrl, putInit] = fetch.mock.calls[1];
		expect(getUrl).toBe(putUrl);
		expect(String(getUrl)).toContain(
			`/desks/${DESK_ID}/programming-update/settings`,
		);
		expect(getInit?.method).toBeUndefined();
		expect(putInit?.method).toBe("PUT");
		expect(JSON.parse(String(putInit?.body))).toEqual(settings());

		const foreignFetch = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValue(response({ ...projection, desk_id: SHOW_ID }));
		await expect(
			harness(foreignFetch).transport.loadSettings(DESK_ID),
		).rejects.toBeInstanceOf(WireValidationError);
	});

	it("marks network failure retryable without replaying the request", async () => {
		const fetch = vi
			.fn<typeof globalThis.fetch>()
			.mockRejectedValue(new Error("offline"));
		const { transport } = harness(fetch);
		await expect(transport.preview(SHOW_ID, previewRequest())).rejects.toEqual(
			expect.objectContaining<Partial<ProgrammingUpdateHttpError>>({
				name: "ProgrammingUpdateHttpError",
				kind: "unavailable",
				status: 0,
				retryable: true,
			}),
		);
		expect(fetch).toHaveBeenCalledOnce();
	});
});
