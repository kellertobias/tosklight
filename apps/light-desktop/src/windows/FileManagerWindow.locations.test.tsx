import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FileRoot } from "../api/types";
import { FileManager } from "./FileManagerWindow";

const mocks = vi.hoisted(() => ({
	fileRoots: vi.fn(),
	fileEntries: vi.fn(),
	fileContent: vi.fn(),
	readFileNote: vi.fn(),
	fileThumbnail: vi.fn(),
}));
vi.mock("../features/files/FilesContext", () => ({ useFiles: () => mocks }));
vi.mock("../features/shellStatus/ShellStatusState", () => ({
	useConnectionStatus: () => "connected",
	useServerError: () => null,
}));
const root: FileRoot = {
	id: "shows",
	label: "Shows",
	icon: "shows",
	removable: false,
	writable: true,
};
function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: Error) => void;
	const promise = new Promise<T>((accept, decline) => {
		resolve = accept;
		reject = decline;
	});
	return { promise, resolve, reject };
}

beforeEach(() => {
	mocks.fileRoots.mockReset().mockResolvedValue([root]);
	mocks.fileEntries
		.mockReset()
		.mockImplementation(async (root_id: string, path: string) => ({
			root_id,
			path,
			entries: [],
		}));
	mocks.fileContent.mockReset().mockResolvedValue(new Blob());
	mocks.readFileNote
		.mockReset()
		.mockResolvedValue({ supported: false, note: null });
	mocks.fileThumbnail.mockReset().mockResolvedValue(new Blob());
});
afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

describe("File Manager authoritative locations", () => {
	it("shows pending initial locations rather than a false empty configuration", async () => {
		const pending = deferred<FileRoot[]>();
		mocks.fileRoots.mockReturnValue(pending.promise);
		const onCancel = vi.fn();
		render(<FileManager picker={{ onSelect: vi.fn(), onCancel }} />);
		expect(screen.getByRole("status", { name: "Locations" })).toHaveTextContent(
			"Loading locations…",
		);
		expect(
			screen.getByRole("progressbar", { name: "Locations" }),
		).toBeVisible();
		expect(
			screen
				.getByRole("status", { name: "Locations" })
				.querySelector(".ui-spinner"),
		).not.toBeNull();
		fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
		expect(onCancel).toHaveBeenCalledTimes(1);
		expect(
			screen.queryByText("No configured or removable locations are available."),
		).not.toBeInTheDocument();
		await act(async () => pending.resolve([root]));
		expect(
			await within(
				screen.getByRole("navigation", { name: "Breadcrumb" }),
			).findByRole("button", { name: "Shows" }),
		).toBeVisible();
		expect(mocks.fileEntries).toHaveBeenCalledWith("shows", "", false);
	});

	it("does not supersede a slow initial request with automatic polls", async () => {
		vi.useFakeTimers();
		const pending = deferred<FileRoot[]>();
		mocks.fileRoots.mockReturnValue(pending.promise);
		render(<FileManager />);
		await act(async () => vi.advanceTimersByTimeAsync(15000));
		expect(mocks.fileRoots).toHaveBeenCalledTimes(1);
		expect(screen.getByRole("status", { name: "Locations" })).toHaveTextContent(
			"Loading locations…",
		);
		await act(async () => pending.resolve([root]));
		expect(
			screen.getByRole("navigation", { name: "Breadcrumb" }),
		).toHaveTextContent("Shows");
	});

	it("declares empty locations only after a successful empty response", async () => {
		const pending = deferred<FileRoot[]>();
		mocks.fileRoots.mockReturnValue(pending.promise);
		render(<FileManager />);
		expect(
			screen.queryByText("No configured or removable locations are available."),
		).not.toBeInTheDocument();
		await act(async () => pending.resolve([]));
		expect(
			screen.getByText("No configured or removable locations are available."),
		).toBeVisible();
		expect(screen.queryByText("Loading locations…")).not.toBeInTheDocument();
	});

	it("offers local Retry after failure and initializes the requested directory on recovery", async () => {
		mocks.fileRoots.mockRejectedValueOnce(new Error("location request failed"));
		const retry = deferred<FileRoot[]>();
		mocks.fileRoots.mockReturnValueOnce(retry.promise);
		render(
			<FileManager
				picker={{
					initialRootId: "shows",
					initialDirectory: "Tour",
					onSelect: vi.fn(),
					onCancel: vi.fn(),
				}}
			/>,
		);
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"Could not load locations. Retry to check available locations.",
		);
		expect(
			screen.queryByText("No configured or removable locations are available."),
		).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Retry locations" }));
		expect(screen.getByRole("status", { name: "Locations" })).toHaveTextContent(
			"Loading locations…",
		);
		await act(async () => retry.resolve([root]));
		await waitFor(() =>
			expect(mocks.fileEntries).toHaveBeenCalledWith("shows", "Tour", false),
		);
		expect(screen.queryByRole("alert")).not.toBeInTheDocument();
	});

	it("keeps navigation available during a pending background refresh", async () => {
		vi.useFakeTimers();
		const refresh = deferred<FileRoot[]>();
		mocks.fileRoots
			.mockResolvedValueOnce([root])
			.mockReturnValueOnce(refresh.promise);
		render(
			<FileManager
				picker={{
					initialDirectory: "Tour",
					onSelect: vi.fn(),
					onCancel: vi.fn(),
				}}
			/>,
		);
		await act(async () => Promise.resolve());
		await act(async () => vi.advanceTimersByTimeAsync(5100));
		expect(
			screen.queryByRole("progressbar", { name: "Locations" }),
		).not.toBeInTheDocument();
		const breadcrumb = screen.getByRole("navigation", { name: "Breadcrumb" });
		fireEvent.click(within(breadcrumb).getByRole("button", { name: "Shows" }));
		await act(async () => Promise.resolve());
		expect(mocks.fileEntries).toHaveBeenCalledWith("shows", "", false);
		await act(async () => refresh.resolve([root]));
	});

	it("keeps the current directory when an automatic roots refresh fails", async () => {
		vi.useFakeTimers();
		mocks.fileRoots
			.mockResolvedValueOnce([root])
			.mockRejectedValueOnce(new Error("temporary failure"));
		render(
			<FileManager
				picker={{
					initialDirectory: "Tour",
					onSelect: vi.fn(),
					onCancel: vi.fn(),
				}}
			/>,
		);
		await act(async () => Promise.resolve());
		await act(async () => vi.advanceTimersByTimeAsync(5100));
		expect(
			screen.getByRole("navigation", { name: "Breadcrumb" }),
		).toHaveTextContent("Shows/ Tour");
		expect(screen.getByRole("alert")).toHaveTextContent(
			"Could not load locations",
		);
		expect(
			within(screen.getByRole("navigation", { name: "Breadcrumb" })).getByRole(
				"button",
				{ name: "Shows" },
			),
		).toBeVisible();
	});

	it("ignores a late root response after the owning picker request changes", async () => {
		const old = deferred<FileRoot[]>();
		mocks.fileRoots.mockReturnValueOnce(old.promise).mockResolvedValue([root]);
		const first = render(
			<FileManager
				picker={{
					initialDirectory: "Old",
					onSelect: vi.fn(),
					onCancel: vi.fn(),
				}}
			/>,
		);
		first.rerender(
			<FileManager
				picker={{
					initialDirectory: "New",
					onSelect: vi.fn(),
					onCancel: vi.fn(),
				}}
			/>,
		);
		await waitFor(() =>
			expect(mocks.fileEntries).toHaveBeenCalledWith("shows", "New", false),
		);
		await act(async () =>
			old.resolve([{ ...root, id: "old", label: "Old response" }]),
		);
		expect(screen.queryByText("Old response")).not.toBeInTheDocument();
		expect(mocks.fileEntries).not.toHaveBeenCalledWith("old", "Old", false);
	});
});
