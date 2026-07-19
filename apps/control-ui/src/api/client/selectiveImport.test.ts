import { describe, expect, it, vi } from "vitest";
import type { ClientTransport } from "./transport";
import { SelectiveImportApiClient } from "./selectiveImport";

function transport() {
	return {
		request: vi.fn().mockResolvedValue({}),
		blob: vi.fn(),
		absoluteUrl: vi.fn(),
	} satisfies ClientTransport;
}

describe("SelectiveImportApiClient", () => {
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
			selectedObjects: [{ kind: "group", id: "front" }],
			conflictResolutions: [],
			profileConflictResolutions: [],
		});
		expect(wire.request).toHaveBeenLastCalledWith(
			"/api/v2/shows/target/selective-imports/source/preview",
			expect.objectContaining({ method: "POST" }),
		);

		await client.apply("target", "source", {
			requestId: "import-front",
			expectedSourceRevision: 4,
			expectedTargetRevision: 9,
			selectedObjects: [{ kind: "group", id: "front" }],
			conflictResolutions: [],
			profileConflictResolutions: [],
		});
		const init = wire.request.mock.calls.at(-1)?.[1] as RequestInit;
		expect(new Headers(init.headers).get("if-match")).toBe("9");
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
			objects: [{
				source: { kind: "group", id: "front" },
				destination: { kind: "group", id: "front" },
				action: { type: "silently_drop_unknown" },
			}],
			dependencies: [],
			conflicts: [],
			profiles: [],
			managed_assets: [],
			blockers: [],
			can_apply: false,
		});
		const client = new SelectiveImportApiClient(wire);

		await expect(client.preview("target", "source", {
			selectedObjects: [{ kind: "group", id: "front" }],
			conflictResolutions: [],
			profileConflictResolutions: [],
		})).rejects.toThrow("objects[0].action.type has an unsupported value");
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

		await expect(client.preview("target", "source", {
			selectedObjects: [{ kind: "group", id: "front" }],
			conflictResolutions: [],
			profileConflictResolutions: [],
		})).rejects.toThrow("blockers[0].type has an unsupported value");
	});
});
