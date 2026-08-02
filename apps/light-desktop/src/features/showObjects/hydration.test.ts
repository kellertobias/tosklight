import { describe, expect, it, vi } from "vitest";
import type { StoredGroup } from "../../api/types";
import type { ShowObject } from "./contracts";
import { loadHydration } from "./hydration";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

function group(id: string, body: Partial<StoredGroup>): ShowObject<"group"> {
	return {
		kind: "group",
		id,
		revision: 1,
		updated_at: "2026-08-02T00:00:00Z",
		body: { fixtures: [], ...body },
	};
}

describe("exact Group hydration", () => {
	it("loads every nested canonical reference once, including missing and cyclic branches", async () => {
		const objects = new Map<string, ShowObject<"group">>([
			[
				"target",
				group("target", {
					source: {
						type: "references",
						references: [
							{ group_id: "left", rule: { type: "all" } },
							{ group_id: "right", rule: { type: "all" } },
							{ group_id: "missing", rule: { type: "all" } },
						],
					},
				}),
			],
			[
				"left",
				group("left", {
					source: {
						type: "references",
						references: [{ group_id: "leaf", rule: { type: "odd" } }],
					},
				}),
			],
			[
				"right",
				group("right", {
					source: {
						type: "references",
						references: [{ group_id: "target", rule: { type: "all" } }],
					},
				}),
			],
			[
				"leaf",
				group("leaf", {
					source: { type: "explicit", fixture_ids: ["a", "b"] },
				}),
			],
		]);
		const loadObject = vi.fn((_showId, _kind, objectId: string) =>
			Promise.resolve({
				object: objects.get(objectId) ?? null,
				showRevision: 7,
			}),
		);

		const loaded = await loadHydration(
			{ kind: "group", objectId: "target" },
			SHOW_ID,
			vi.fn(),
			loadObject,
		);

		expect(loadObject.mock.calls.map((call) => call[2])).toEqual([
			"target",
			"left",
			"right",
			"missing",
			"leaf",
		]);
		expect([...(loaded.groupDependencies ?? [])]).toEqual([
			"left",
			"right",
			"missing",
			"leaf",
			"target",
		]);
		expect(loaded.installs.map((install) => install.objectId)).toEqual([
			"target",
			"left",
			"right",
			"missing",
			"leaf",
		]);
	});

	it("uses the tolerant legacy derived_from dependency when source is absent", async () => {
		const target = group("legacy", {
			derived_from: {
				source_group_id: "source",
				rule: { type: "all" },
			},
		});
		const source = group("source", { fixtures: ["a"] });
		const loaded = await loadHydration(
			{ kind: "group", objectId: "legacy" },
			SHOW_ID,
			vi.fn(),
			vi.fn((_showId, _kind, objectId: string) =>
				Promise.resolve({
					object: objectId === "legacy" ? target : source,
					showRevision: 1,
				}),
			),
		);

		expect([...(loaded.groupDependencies ?? [])]).toEqual(["source"]);
	});
});
