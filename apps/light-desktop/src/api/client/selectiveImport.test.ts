import { describe, expect, it, vi } from "vitest";
import { SelectiveImportApiClient } from "./selectiveImport";
import type { ClientTransport } from "./transport";

function transport() {
	return {
		request: vi.fn().mockResolvedValue({}),
		blob: vi.fn(),
		absoluteUrl: vi.fn(),
	} satisfies ClientTransport;
}

describe("SelectiveImportApiClient", () => {
	it("retains backend-owned section and patch-layer catalog metadata", async () => {
		const wire = transport();
		wire.request.mockResolvedValue({
			source_show_id: "source",
			source_show_name: "Tour",
			source_revision: 7,
			objects: [
				{
					key: { kind: "patched_fixture", id: "fixture-a" },
					object_revision: 3,
					display_name: "Front Wash",
					section: "fixture_patch",
					patch_layer_id: "front",
				},
			],
		});

		const catalog = await new SelectiveImportApiClient(wire).catalog(
			"target",
			"source",
		);

		expect(catalog.objects[0]).toMatchObject({
			section: "fixture_patch",
			patchLayerId: "front",
		});
	});

	it("keeps preview side-effect free and carries both revisions into apply", async () => {
		const wire = transport();
		wire.request
			.mockResolvedValueOnce({
				source_show_id: "source",
				target_show_id: "target",
				source_revision: 4,
				target_revision: 9,
				objects: [],
				dependencies: [],
				conflicts: [],
				profiles: [],
				managed_assets: [],
				blockers: [],
				can_apply: true,
			})
			.mockResolvedValueOnce({
				request_id: "import-front",
				correlation_id: "correlation",
				changed: false,
				show_id: "target",
				show_revision: 9,
				event_sequence: null,
				outcomes: [],
				objects: [],
				profiles: [],
				managed_assets: [],
			});
		const client = new SelectiveImportApiClient(wire);
		await client.preview("target", "source", {
			mode: "replace_by_position",
			selectedObjects: [{ kind: "group", id: "front" }],
			conflictResolutions: [],
			profileConflictResolutions: [],
		});
		expect(wire.request).toHaveBeenLastCalledWith(
			"/api/v2/selective-imports/source/preview",
			expect.objectContaining({ method: "POST" }),
		);
		const previewInit = wire.request.mock.calls.at(-1)?.[1] as RequestInit;
		expect(new Headers(previewInit.headers).get("x-tosk-show")).toBe("target");

		await client.apply("target", "source", {
			requestId: "import-front",
			expectedSourceRevision: 4,
			expectedTargetRevision: 9,
			mode: "replace_by_position",
			selectedObjects: [{ kind: "group", id: "front" }],
			conflictResolutions: [],
			profileConflictResolutions: [],
		});
		const init = wire.request.mock.calls.at(-1)?.[1] as RequestInit;
		expect(new Headers(init.headers).get("if-match")).toBe("9");
		expect(new Headers(init.headers).get("x-tosk-show")).toBe("target");
		expect(JSON.parse(String(init.body))).toMatchObject({
			expected_source_revision: 4,
			expected_target_revision: 9,
		});
	});

	it("rejects unsupported server actions instead of guessing at future semantics", async () => {
		const wire = transport();
		wire.request.mockResolvedValue({
			source_show_id: "source",
			target_show_id: "target",
			source_revision: 4,
			target_revision: 9,
			objects: [
				{
					source: { kind: "group", id: "front" },
					destination: { kind: "group", id: "front" },
					action: { type: "silently_drop_unknown" },
				},
			],
			dependencies: [],
			conflicts: [],
			profiles: [],
			managed_assets: [],
			blockers: [],
			can_apply: false,
		});
		const client = new SelectiveImportApiClient(wire);

		await expect(
			client.preview("target", "source", {
				mode: "replace_by_position",
				selectedObjects: [{ kind: "group", id: "front" }],
				conflictResolutions: [],
				profileConflictResolutions: [],
			}),
		).rejects.toThrow("objects[0].action.type has an unsupported value");
	});

	it("rejects unknown blocker discriminators at the transport boundary", async () => {
		const wire = transport();
		wire.request.mockResolvedValue({
			source_show_id: "source",
			target_show_id: "target",
			source_revision: 4,
			target_revision: 9,
			objects: [],
			dependencies: [],
			conflicts: [],
			profiles: [],
			managed_assets: [],
			blockers: [{ type: "silently_ignore_dependency" }],
			can_apply: false,
		});
		const client = new SelectiveImportApiClient(wire);

		await expect(
			client.preview("target", "source", {
				mode: "replace_by_position",
				selectedObjects: [{ kind: "group", id: "front" }],
				conflictResolutions: [],
				profileConflictResolutions: [],
			}),
		).rejects.toThrow("blockers[0].type has an unsupported value");
	});
});
