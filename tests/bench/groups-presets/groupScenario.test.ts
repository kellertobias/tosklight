import { afterEach, describe, expect, it, vi } from "vitest";
import type { Page } from "@playwright/test";
import { ApiDriver } from "../core/api";
import { BrowserGroups } from "./groupScenario";

const transport = vi.hoisted(() => ({ settings: vi.fn(), manage: vi.fn() }));
vi.mock("../../../apps/light-desktop/src/api/GroupManagementTransport", () => ({
	HttpGroupManagementTransport: class {
		settings = transport.settings;
		manage = transport.manage;
	},
}));
afterEach(() => vi.resetAllMocks());
function groups() {
	const api = new ApiDriver("http://desk.local");
	api.session = {
		session_id: "session",
		token: "token",
		client_id: "client",
		desk: { id: "desk", name: "Desk" },
	};
	const before = {
		kind: "group",
		id: "310",
		revision: 2,
		updated_at: "now",
		body: { name: "Original", fixtures: [] },
	};
	const after = {
		...before,
		revision: 8,
		body: { name: "Updated", fixtures: [], color: "#112233", icon: "star" },
	};
	vi.spyOn(api, "showObject").mockResolvedValue(after);
	const helper = new BrowserGroups(
		api,
		{} as Page,
		{} as never,
		{} as never,
		{} as never,
		{} as never,
		() => "show",
		"seed",
	);
	vi.spyOn(helper, "requiredObject")
		.mockResolvedValueOnce(before)
		.mockResolvedValue(after);
	return helper;
}
describe("Group helper guarded property edits", () => {
	it("uses fresh settings show and object revisions in one canonical update", async () => {
		const helper = groups();
		transport.settings.mockResolvedValue({
			showId: "show",
			showRevision: 19,
			group: { id: "310", revision: 7 },
		});
		transport.manage.mockImplementation(async (showId, request) => {
			expect(showId).toBe("show");
			expect(request).toMatchObject({
				groupId: "310",
				expectedShowRevision: 19,
				expectedObjectRevision: 7,
				operation: {
					type: "update_properties",
					properties: { name: "Updated", color: "#112233", icon: "star" },
				},
			});
		});
		await helper.editVia("api", 310, {
			name: "Updated",
			color: "#112233",
			icon: "star",
		});
		expect(transport.settings).toHaveBeenCalledExactlyOnceWith("show", "310");
		expect(transport.manage).toHaveBeenCalledOnce();
	});
	it("does not mutate after fresh settings authority fails", async () => {
		const helper = groups();
		transport.settings.mockRejectedValue(new Error("show replaced"));
		await expect(
			helper.editVia("api", 310, {
				name: "Updated",
				color: "#112233",
				icon: "star",
			}),
		).rejects.toThrow("show replaced");
		expect(transport.manage).not.toHaveBeenCalled();
	});
});
