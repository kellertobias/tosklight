import { createElement } from "react";
import { FileBar } from "../FileBar";
import { ShowPaperwork } from "../ShowPaperwork";
import { act, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { documentSession, type DocumentSummary } from "./session";
import { usePaperworkAutosave } from "./usePaperworkAutosave";
import { registerPendingDocumentSave } from "./pendingDocumentSaves";

vi.mock("../useDiscoveredDesks", () => ({ useDiscoveredDesks: () => [] }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn(async () => "/copy.show"), open: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
const first = { showId: "first", name: "Original", project: "Original project" } as DocumentSummary;
let active: string;
let stored: Map<string, DocumentSummary>;
let commands: string[];
const native = vi.mocked(invoke);

function deferred() {
	let resolve!: () => void;
	const promise = new Promise<void>((done) => { resolve = done; });
	return { promise, resolve };
}

beforeEach(() => {
	vi.useFakeTimers();
	active = "first";
	stored = new Map([["first", first]]);
	commands = [];
	native.mockReset().mockImplementation(async (command, args) => {
		const request = args as { showId: string; paperwork: Partial<DocumentSummary> };
		commands.push(command);
		if (command === "save_document_paperwork") {
			if (request.showId !== active) throw new Error("The open show changed");
			const saved = { ...stored.get(active)!, ...request.paperwork };
			stored.set(active, saved);
			return saved;
		}
		if (command === "open_demo_show" || command === "save_document_as") {
			const next = { ...stored.get(active)!, showId: "second" };
			active = "second";
			stored.set(active, next);
			return next;
		}
		return null;
	});
});
afterEach(() => vi.useRealTimers());

describe("project saves before native document actions", () => {
	it.each(["Open Demo Show", "Save As"])("saves a typed Project before clicking %s switches the native identity", async (button) => {
		render(createElement(FileBar, {
			document: first, onDocument: vi.fn(), onError: vi.fn(),
			onReloadProfiles: vi.fn(), onReloadDocument: vi.fn(),
		}, createElement(ShowPaperwork, { document: first, onDocument: vi.fn(), onError: vi.fn() })));
		fireEvent.change(screen.getByLabelText("Project"), { target: { value: "Latest project" } });
		// No timer advance: this is the operator's immediate physical file-action path.
		await act(async () => fireEvent.click(screen.getByRole("button", { name: button })));
		expect(stored.get("first")?.project).toBe("Latest project");
		expect(stored.get("second")?.project).toBe("Latest project");
		expect(active).toBe("second");
		const relevant = commands.filter((command) => command !== "source_desk");
		expect(relevant).toEqual(["save_document_paperwork", button === "Open Demo Show" ? "open_demo_show" : "save_document_as"]);
	});

	it("waits for an active save and every newer queued edit before making the copy", async () => {
		const gate = deferred();
		const implementation = native.getMockImplementation()!;
		native.mockImplementationOnce(async (command, args) => { await gate.promise; return implementation(command, args); });
		const { result } = renderHook(() => usePaperworkAutosave(first, vi.fn()));
		act(() => result.current.change("project", "First draft"));
		await act(async () => { await vi.advanceTimersByTimeAsync(350); });
		let copy!: Promise<DocumentSummary>;
		act(() => { copy = documentSession.saveAs("/copy.show"); });
		act(() => result.current.change("project", "Newest draft"));
		expect(commands).toEqual([]);
		await act(async () => { gate.resolve(); await copy; });
		expect(commands).toEqual(["save_document_paperwork", "save_document_paperwork", "save_document_as"]);
		expect(stored.get("first")?.project).toBe("Newest draft");
		expect(stored.get("second")?.project).toBe("Newest draft");
	});

	it("blocks switching on failure and retains the draft and Retry state", async () => {
		native.mockRejectedValueOnce(new Error("Disk full"));
		const { result } = renderHook(() => usePaperworkAutosave(first, vi.fn()));
		act(() => result.current.change("project", "Unsaved project"));
		await act(async () => { await expect(documentSession.openDemoShow()).rejects.toThrow("Disk full"); });
		expect(active).toBe("first");
		expect(commands).toEqual([]);
		expect(result.current.error).toContain("Disk full");
		expect(result.current.draft.project).toBe("Unsaved project");
		await act(async () => result.current.retry());
		await act(async () => { await documentSession.openDemoShow(); });
		expect(stored.get("first")?.project).toBe("Unsaved project");
		expect(active).toBe("second");
	});

	it("keeps an unmounted form's active cleanup save in the barrier until it finishes", async () => {
		const gate = deferred();
		const implementation = native.getMockImplementation()!;
		native.mockImplementationOnce(async (command, args) => { await gate.promise; return implementation(command, args); });
		const { result, unmount } = renderHook(() => usePaperworkAutosave(first, vi.fn()));
		act(() => result.current.change("project", "Before leaving Show"));
		unmount();
		const replacement = documentSession.openDemoShow();
		expect(active).toBe("first");
		await act(async () => { gate.resolve(); await replacement; });
		expect(stored.get("first")?.project).toBe("Before leaving Show");
		expect(active).toBe("second");
	});

	it.each([
		["create_document", () => documentSession.create("/new.show", "New")],
		["open_document", () => documentSession.open("/recent.show")],
		["load_desk_show", () => documentSession.loadDeskShow("desk", "Show", "id")],
		["load_from_desk", () => documentSession.loadFromDesk("desk")],
		["publish_to_desk", () => documentSession.publishToDesk("desk")],
		["export_mvr", () => documentSession.exportMvr("/rig.mvr")],
		["import_mvr", () => documentSession.importMvr("token")],
	] as const)("keeps %s behind a pending editor save", async (command, action) => {
		const gate = deferred();
		const unregister = registerPendingDocumentSave(() => gate.promise);
		try {
			const work = action();
			expect(native).not.toHaveBeenCalled();
			gate.resolve();
			await work;
			expect(commands).toEqual([command]);
		} finally { unregister(); }
	});
});
