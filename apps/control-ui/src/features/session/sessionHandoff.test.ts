import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SessionResponse } from "../../api/types";
import {
	createSessionHandoff,
	SESSION_HANDOFF_RECEIVER,
	type SessionHandoffPublication,
} from "./sessionHandoff";

beforeEach(() => vi.stubGlobal("localStorage", memoryStorage()));

afterEach(() => {
	localStorage.clear();
	document.body.replaceChildren();
	vi.unstubAllGlobals();
});

describe("session handoff", () => {
	it("stays inert when no controllable receiver was installed", () => {
		const handoff = createSessionHandoff({} as Window);
		expect(() => handoff.capture(1, session("one", "user-a", "desk-a")))
			.not.toThrow();
		expect(localStorage).toHaveLength(0);
		expect(document.body.textContent).toBe("");
	});

	it("publishes replacements without storing or rendering credentials", async () => {
		const publications: SessionHandoffPublication[] = [];
		const runtime = {
			[SESSION_HANDOFF_RECEIVER]: (publication: SessionHandoffPublication) => {
				publications.push(publication);
			},
		} as unknown as Window;
		const handoff = createSessionHandoff(runtime);
		handoff.capture(1, session("one", "user-a", "desk-a"));
		handoff.capture(2, session("two", "user-a", "desk-b"));
		handoff.capture(3, session("three", "user-b", "desk-c"));
		await Promise.resolve();
		expect(
			publications
				.filter((item) => item.type === "captured")
				.map((item) => [item.session.user.id, item.session.desk.id]),
		).toEqual([
			["user-a", "desk-a"],
			["user-a", "desk-b"],
			["user-b", "desk-c"],
		]);
		const stored = Array.from({ length: localStorage.length }, (_, index) => {
			const key = localStorage.key(index) ?? "";
			return `${key}:${localStorage.getItem(key) ?? ""}`;
		});
		expect(stored.join("\n")).not.toContain("token-three");
		expect(document.body.textContent).not.toContain("token-three");
		expect(document.documentElement.outerHTML).not.toContain("token-three");
	});

	it("clears current authority and ignores a late older bootstrap", async () => {
		const receiver = vi.fn();
		const runtime = { [SESSION_HANDOFF_RECEIVER]: receiver } as unknown as Window;
		const handoff = createSessionHandoff(runtime);
		handoff.capture(4, session("new", "user-a", "desk-b"));
		handoff.release(5, "new");
		handoff.capture(3, session("late", "user-a", "desk-a"));
		await Promise.resolve();
		expect(receiver).toHaveBeenCalledTimes(2);
		expect(receiver).toHaveBeenLastCalledWith({
			type: "released",
			generation: 5,
			session_id: "new",
		});
	});
});

function session(
	id: string,
	userId: string,
	deskId: string,
): SessionResponse {
	return {
		session_id: id,
		client_id: `client-${id}`,
		token: `token-${id}`,
		user: { id: userId, name: userId, enabled: true },
		desk: {
			id: deskId,
			name: deskId,
			osc_alias: deskId,
			columns: 10,
			rows: 4,
			buttons: 40,
		},
	};
}

function memoryStorage(): Storage {
	const values = new Map<string, string>();
	return {
		get length() {
			return values.size;
		},
		clear: () => values.clear(),
		getItem: (key) => values.get(key) ?? null,
		key: (index) => [...values.keys()][index] ?? null,
		removeItem: (key) => values.delete(key),
		setItem: (key, value) => values.set(key, value),
	};
}
