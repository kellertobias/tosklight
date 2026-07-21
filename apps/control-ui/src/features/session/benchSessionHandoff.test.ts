import { describe, expect, it, vi } from "vitest";
import type { Page } from "@playwright/test";
import { BrowserSessionHandoff } from "../../../e2e/bench/sessionHandoff";

describe("browser session handoff authority", () => {
	it("clears a prior document before accepting its replacement", async () => {
		const page = fakePage();
		const handoff = new BrowserSessionHandoff(page.value);
		await handoff.install();
		page.publish(capture("document-a", 1, "session-a", "desk-a"));
		expect(handoff.currentSession()?.session_id).toBe("session-a");
		const checkpoint = handoff.checkpoint();

		page.navigate("document-b");
		await handoff.adoptCurrentDocument();
		expect(handoff.currentSession()).toBeNull();
		const replacement = handoff.waitForCapture(checkpoint);
		page.publish(capture("document-a", 2, "late-session-a", "desk-a"));
		expect(handoff.currentSession()).toBeNull();
		page.publish(capture("document-b", 1, "session-b", "desk-b"));
		await expect(replacement).resolves.toMatchObject({
			session_id: "session-b",
			desk: { id: "desk-b" },
		});
		page.close();
		expect(handoff.currentSession()).toBeNull();
		await expect(handoff.waitForCapture(handoff.checkpoint())).rejects.toThrow(
			"disposed",
		);
		page.publish(capture("document-b", 2, "late-session-b", "desk-b"));
		expect(handoff.currentSession()).toBeNull();
		handoff.dispose();
	});

	it("rejects a late prior document that never captured", async () => {
		const page = fakePage();
		const handoff = new BrowserSessionHandoff(page.value);
		await handoff.install();
		await handoff.adoptCurrentDocument();
		const checkpoint = handoff.checkpoint();

		page.navigate("document-b");
		page.publish(capture("document-a", 9, "late-session-a", "desk-a"));
		expect(handoff.currentSession()).toBeNull();
		await handoff.adoptCurrentDocument();
		const replacement = handoff.waitForCapture(checkpoint);
		page.publish(capture("document-b", 1, "session-b", "desk-b"));
		await expect(replacement).resolves.toMatchObject({
			session_id: "session-b",
			desk: { id: "desk-b" },
		});
		handoff.dispose();
	});
});

function fakePage() {
	const frame = {};
	const handlers = new Map<string, Set<(...arguments_: unknown[]) => void>>();
	let binding: ((source: unknown, value: unknown) => void) | undefined;
	let documentId = "document-a";
	const value = {
		on: (event: string, handler: (...arguments_: unknown[]) => void) => {
			const listeners = handlers.get(event) ?? new Set();
			listeners.add(handler);
			handlers.set(event, listeners);
		},
		off: (event: string, handler: (...arguments_: unknown[]) => void) =>
			handlers.get(event)?.delete(handler),
		mainFrame: () => frame,
		exposeBinding: vi.fn(async (_name, handler) => {
			binding = handler;
		}),
		addInitScript: vi.fn().mockResolvedValue(undefined),
		evaluate: vi.fn(async () => documentId),
	} as unknown as Page;
	return {
		value,
		navigate: (nextDocumentId: string) => {
			documentId = nextDocumentId;
			for (const handler of handlers.get("framenavigated") ?? []) handler(frame);
		},
		close: () => {
			for (const handler of handlers.get("close") ?? []) handler();
		},
		publish: (publication: unknown) => binding?.({}, publication),
	};
}

function capture(
	documentId: string,
	generation: number,
	sessionId: string,
	deskId: string,
) {
	return {
		document_id: documentId,
		publication: {
			type: "captured",
			generation,
			session: {
				session_id: sessionId,
				client_id: `client-${sessionId}`,
				token: `token-${sessionId}`,
				user: { id: "user-a", name: "Operator", enabled: true },
				desk: {
					id: deskId,
					name: deskId,
					osc_alias: deskId,
					columns: 10,
					rows: 4,
					buttons: 40,
				},
			},
		},
	};
}
