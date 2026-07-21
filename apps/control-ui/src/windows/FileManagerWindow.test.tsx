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
import type { FileEntry } from "../api/types";
import { Button } from "../components/common/controls";
import { requestPaneRemoval } from "../components/shell/paneRemovalGuard";
import { createCommandLineTestAuthority } from "../features/programmingInteraction/testing/commandLineTestAuthority";
import {
	FileManager,
	nextKeepBothName,
	operationFromCommandLine,
	pickerSelectionIsValid,
	sortFileEntries,
	validItemName,
} from "./FileManagerWindow";

const entries: FileEntry[] = [
	{
		name: "image.png",
		path: "image.png",
		kind: "file",
		size: 2048,
		modified_millis: 2_000,
		created_millis: null,
		hidden: false,
		writable: true,
	},
	{
		name: "Folder",
		path: "Folder",
		kind: "folder",
		size: 0,
		modified_millis: 1_000,
		created_millis: 500,
		hidden: false,
		writable: true,
	},
	{
		name: "alpha.txt",
		path: "alpha.txt",
		kind: "file",
		size: 12,
		modified_millis: null,
		created_millis: null,
		hidden: false,
		writable: false,
	},
];

vi.mock("../features/shellStatus/ShellStatusState", () => ({
	useConnectionStatus: () => mocks.server.status,
	useServerError: () => null,
}));

const mocks = vi.hoisted(() => ({
	server: {
		status: "connected",
		commandLine: "FIXTURE",
		commandTargetMode: "FIXTURE",
		commandLinePristine: true,
		selectedFixtures: [],
		selectedGroupId: null,
		pendingCommandChoice: null,
		fileRoots: vi.fn(),
		fileEntries: vi.fn(),
		fileContent: vi.fn(),
		fileStreamUrl: vi.fn(),
		fileThumbnail: vi.fn(),
		fileOperation: vi.fn(),
		claimFileInput: vi.fn(),
		releaseFileInput: vi.fn(),
		readFileNote: vi.fn(),
		saveFileNote: vi.fn(),
		readTextFile: vi.fn(),
		saveTextFile: vi.fn(),
		resetCommandLine: vi.fn(),
		setCommandLine: vi.fn(),
		executeCommandLine: vi.fn(),
		cancelCommandChoice: vi.fn(),
	},
}));

vi.mock("../api/ServerContext", () => ({ useServer: () => mocks.server }));

vi.mock("../features/files/FilesContext", () => ({
	useFiles: () => mocks.server,
}));

describe("FileManager helpers", () => {
	it("sorts folders first and names naturally", () => {
		expect(sortFileEntries(entries).map((entry) => entry.name)).toEqual([
			"Folder",
			"alpha.txt",
			"image.png",
		]);
	});

	it("recognizes only unowned desk file actions", () => {
		expect(operationFromCommandLine(" COPY ")).toBe("copy");
		expect(operationFromCommandLine("MOV")).toBe("move");
		expect(operationFromCommandLine("DELETE 2")).toBeNull();
		expect(operationFromCommandLine("FIXTURE")).toBeNull();
	});

	it("validates picker targets, extensions, and cardinality", () => {
		const file = { rootId: "shows", entry: entries[0] };
		const folder = { rootId: "shows", entry: entries[1] };
		const picker = {
			target: "files" as const,
			allowedExtensions: [".PNG"],
			onSelect: vi.fn(),
			onCancel: vi.fn(),
		};
		expect(pickerSelectionIsValid([file], picker)).toBe(true);
		expect(pickerSelectionIsValid([folder], picker)).toBe(false);
		expect(pickerSelectionIsValid([file, file], picker)).toBe(false);
		expect(
			pickerSelectionIsValid([file, file], { ...picker, multiple: true }),
		).toBe(true);
		expect(
			pickerSelectionIsValid([folder], { ...picker, target: "either" }),
		).toBe(true);
	});

	it("generates safe Keep Both names", () => {
		expect(nextKeepBothName("plot.png", ["plot.png", "plot copy.png"])).toBe(
			"plot copy 2.png",
		);
		expect(nextKeepBothName("Folder", ["folder", "Folder copy"])).toBe(
			"Folder copy 2",
		);
		expect(validItemName("Cue Notes.txt")).toBe(true);
		expect(validItemName("../Cue Notes.txt")).toBe(false);
	});
});

function chooseHeaderAction(menu: "Edit" | "New" | "View", action: string) {
	fireEvent.click(screen.getByRole("button", { name: menu }));
	const popup = screen.getByRole("menu", { name: `${menu} menu` });
	const item =
		within(popup).queryByRole("menuitem", { name: action }) ??
		within(popup).queryByRole("menuitemradio", { name: action }) ??
		within(popup).getByRole("menuitemcheckbox", { name: action });
	fireEvent.click(item);
}

function resetFileManagerMocks() {
	mocks.server.status = "connected";
	mocks.server.commandLine = "FIXTURE";
	mocks.server.fileRoots.mockReset().mockResolvedValue([
		{
			id: "shows",
			label: "Shows",
			icon: "shows",
			removable: false,
			writable: true,
		},
		{
			id: "usb",
			label: "Tour USB",
			icon: "drive",
			removable: true,
			writable: true,
		},
	]);
	mocks.server.fileEntries
		.mockReset()
		.mockImplementation(
			async (root: string, path: string, hidden: boolean) => ({
				root_id: root,
				path,
				entries:
					path === "Folder"
						? [
								{
									name: "nested.txt",
									path: "Folder/nested.txt",
									kind: "file",
									size: 4,
									modified_millis: null,
									created_millis: null,
									hidden: false,
									writable: true,
								},
							]
						: hidden
							? [
									...entries,
									{
										name: ".secret",
										path: ".secret",
										kind: "file",
										size: 1,
										modified_millis: null,
										created_millis: null,
										hidden: true,
										writable: true,
									},
								]
							: entries,
			}),
		);
	mocks.server.fileContent.mockReset().mockResolvedValue(new Blob(["preview"]));
	mocks.server.fileStreamUrl
		.mockReset()
		.mockResolvedValue(
			"http://light.test/api/v1/files/shows/content?path=audio.wav&ticket=capability",
		);
	mocks.server.fileThumbnail
		.mockReset()
		.mockResolvedValue(new Blob(["thumbnail"]));
	mocks.server.fileOperation
		.mockReset()
		.mockResolvedValue({ paths: [], complete: true, items: [] });
	mocks.server.claimFileInput.mockReset().mockResolvedValue({});
	mocks.server.releaseFileInput.mockReset().mockResolvedValue(undefined);
	mocks.server.readFileNote
		.mockReset()
		.mockResolvedValue({ supported: false, note: null });
	mocks.server.saveFileNote
		.mockReset()
		.mockImplementation(
			async (root_id: string, path: string, note: string) => ({
				root_id,
				path,
				supported: true,
				note,
			}),
		);
	mocks.server.readTextFile.mockReset().mockResolvedValue({
		root_id: "shows",
		path: "alpha.txt",
		text: "Alpha",
		revision: "1",
		read_only: false,
	});
	mocks.server.saveTextFile
		.mockReset()
		.mockImplementation(async (root: string, path: string, text: string) => ({
			root_id: root,
			path,
			text,
			revision: "2",
			read_only: false,
		}));
	mocks.server.resetCommandLine.mockReset().mockImplementation(() => {
		mocks.server.commandLine = "FIXTURE";
	});
}

function cleanupFileManagerTest() {
	cleanup();
	vi.restoreAllMocks();
	vi.useRealTimers();
}

describe("FileManager layout", () => {
	beforeEach(resetFileManagerMocks);
	afterEach(cleanupFileManagerTest);

	it("renders the normal three-column layout, folders-first list, properties, and toggles", async () => {
		const { container } = render(<FileManager instanceId="layout" />);
		expect(screen.getByText("File Manager")).toBeVisible();
		expect(screen.getByText("Browse and manage files")).toBeVisible();
		expect(
			await screen.findByRole("button", { name: "Current path /" }),
		).toHaveTextContent("/");
		expect(screen.queryByText("Shows: /")).not.toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Edit" })).toBeVisible();
		expect(screen.getByRole("button", { name: "New" })).toBeVisible();
		expect(screen.getByRole("button", { name: "View" })).toBeVisible();
		expect(
			screen.getByRole("button", { name: "Edit" }).querySelector("svg"),
		).toBeTruthy();
		fireEvent.click(screen.getByRole("button", { name: "Current path /" }));
		expect(
			within(screen.getByRole("menu", { name: "Location menu" })).getByRole(
				"menuitem",
				{ name: "Shows" },
			),
		).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Current path /" }));
		expect(
			screen.queryByRole("button", { name: "Close File Manager" }),
		).not.toBeInTheDocument();
		expect(
			await screen.findByRole("heading", { name: "Locations" }),
		).toBeVisible();
		expect(screen.getByRole("heading", { name: "Properties" })).toBeVisible();
		expect(container.querySelector(".file-columns")?.children).toHaveLength(3);
		const rows = await screen.findAllByRole("button", {
			name: /, (?:folder|file)$/,
		});
		expect(rows.map((row) => row.getAttribute("aria-label"))).toEqual([
			"Folder, folder",
			"alpha.txt, file",
			"image.png, file",
		]);

		fireEvent.click(screen.getByRole("button", { name: "alpha.txt, file" }));
		fireEvent.click(screen.getByRole("button", { name: "Edit" }));
		const editMenu = screen.getByRole("menu", { name: "Edit menu" });
		for (const [name, className] of [
			["Rename", "file-menu-rename"],
			["Copy", "file-menu-copy"],
			["Move", "file-menu-move"],
			["Delete", "file-menu-delete"],
		]) {
			const item = within(editMenu).getByRole("menuitem", { name });
			expect(item).toHaveClass(className);
			expect(item.querySelector("svg")).toBeTruthy();
		}
		fireEvent.click(screen.getByRole("button", { name: "Edit" }));
		fireEvent.click(screen.getByRole("button", { name: "New" }));
		expect(
			within(screen.getByRole("menu", { name: "New menu" }))
				.getByRole("menuitem", { name: "New File" })
				.querySelector("svg"),
		).toBeTruthy();
		expect(
			within(screen.getByRole("menu", { name: "New menu" }))
				.getByRole("menuitem", { name: "New Folder" })
				.querySelector("svg"),
		).toBeTruthy();
		fireEvent.click(screen.getByRole("button", { name: "New" }));
		const properties = screen.getByRole("complementary", {
			name: "Selection properties",
		});
		expect(within(properties).getByText("Read only")).toBeVisible();
		expect(within(properties).getAllByText("Unavailable")).toHaveLength(2);
		expect(within(properties).getByLabelText("Notes")).toBeDisabled();

		chooseHeaderAction("View", "Show Hidden Files");
		expect(
			await screen.findByRole("button", { name: ".secret, file" }),
		).toBeVisible();
		expect(mocks.server.fileEntries).toHaveBeenCalledWith("shows", "", true);
		chooseHeaderAction("View", "Grid");
		expect(container.querySelector(".file-grid")).toBeTruthy();
		chooseHeaderAction("View", "Show Properties Sidebar");
		expect(container.querySelector(".file-manager")).toHaveClass(
			"fm-properties-hidden",
		);

		fireEvent.click(screen.getByRole("button", { name: "View" }));
		expect(screen.getByRole("menuitemradio", { name: "Grid" })).toHaveAttribute(
			"aria-checked",
			"true",
		);
		expect(
			screen.getByRole("menuitemcheckbox", { name: "Show Hidden Files" }),
		).toHaveAttribute("aria-checked", "true");
		expect(
			screen.getByRole("menuitemcheckbox", { name: "Show Properties Sidebar" }),
		).toHaveAttribute("aria-checked", "false");
		expect(screen.getByRole("separator")).toBeVisible();
	});
});

describe("FileManager navigation and previews", () => {
	beforeEach(resetFileManagerMocks);
	afterEach(cleanupFileManagerTest);

	it("clears a transient directory error after the next successful refresh", async () => {
		mocks.server.fileEntries
			.mockRejectedValueOnce(new Error("temporary root race"))
			.mockImplementation(async (root: string, path: string) => ({
				root_id: root,
				path,
				entries,
			}));
		render(<FileManager instanceId="transient-directory-error" />);

		expect(await screen.findByRole("status")).toHaveTextContent(
			"Could not open this location",
		);
		chooseHeaderAction("View", "Show Hidden Files");

		expect(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		).toBeVisible();
		await waitFor(() => expect(screen.queryByRole("status")).toBeNull());
	});

	it("loads tree folders lazily and preserves breadcrumb/history navigation", async () => {
		render(<FileManager instanceId="tree" />);
		const tree = await screen.findByRole("complementary", {
			name: "Folder navigation",
		});
		const shows = within(tree).getByRole("button", { name: /Shows/ });
		expect(await screen.findByRole("button", { name: /Folder/ })).toBeVisible();
		fireEvent.click(shows);
		await waitFor(() =>
			expect(mocks.server.fileEntries).toHaveBeenCalledWith("shows", "", false),
		);
		const folderTreeButton = await within(tree).findByRole("button", {
			name: /Folder/,
		});
		fireEvent.click(folderTreeButton);
		expect(
			await screen.findByRole("button", { name: "nested.txt, file" }),
		).toBeVisible();
		expect(
			screen.getByRole("navigation", { name: "Breadcrumb" }),
		).toHaveTextContent("/ Folder");
		fireEvent.click(screen.getByRole("button", { name: "Back" }));
		expect(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Forward" }));
		expect(
			await screen.findByRole("button", { name: "nested.txt, file" }),
		).toBeVisible();
	});

	it("gives audio players a range-capable stream URL instead of downloading a whole Blob", async () => {
		mocks.server.fileEntries.mockResolvedValue({
			root_id: "shows",
			path: "",
			entries: [
				{
					name: "walk-in.wav",
					path: "walk-in.wav",
					kind: "file",
					size: 4096,
					modified_millis: null,
					created_millis: null,
					hidden: false,
					writable: true,
				},
			],
		});
		render(<FileManager instanceId="audio-preview" />);
		fireEvent.click(
			await screen.findByRole("button", { name: "walk-in.wav, file" }),
		);

		const player = await screen.findByLabelText("Audio preview of walk-in.wav");
		expect(player).toHaveAttribute(
			"src",
			expect.stringContaining("ticket=capability"),
		);
		expect(mocks.server.fileStreamUrl).toHaveBeenCalledWith(
			"shows",
			"walk-in.wav",
		);
		expect(mocks.server.fileContent).not.toHaveBeenCalled();
	});
});

describe("FileManager picker contracts", () => {
	beforeEach(resetFileManagerMocks);
	afterEach(cleanupFileManagerTest);

	it("supports picker target, extension, callback, ENTER, and ESC contracts", async () => {
		const onSelect = vi.fn();
		const onCancel = vi.fn();
		render(
			<FileManager
				instanceId="picker"
				picker={{
					target: "files",
					allowedExtensions: ["png"],
					onSelect,
					onCancel,
				}}
			/>,
		);
		const select = await screen.findByRole("button", { name: "Select" });
		expect(select).toBeDisabled();
		fireEvent.click(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		);
		expect(select).toBeDisabled();
		expect(onSelect).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "image.png, file" }));
		expect(select).toBeEnabled();
		expect(onSelect).not.toHaveBeenCalled();
		fireEvent.keyDown(window, { key: "Enter" });
		expect(onSelect).toHaveBeenCalledWith([
			expect.objectContaining({
				rootId: "shows",
				entry: expect.objectContaining({ path: "image.png" }),
			}),
		]);
		fireEvent.keyDown(window, { key: "Escape" });
		expect(onCancel).toHaveBeenCalledOnce();
	});

	it("supports Ctrl toggles and Shift range selection", async () => {
		render(<FileManager instanceId="selection" />);
		const folder = await screen.findByRole("button", {
			name: "Folder, folder",
		});
		const alpha = screen.getByRole("button", { name: "alpha.txt, file" });
		const image = screen.getByRole("button", { name: "image.png, file" });
		fireEvent.click(folder);
		fireEvent.click(image, { ctrlKey: true });
		expect(screen.getByText("2 items selected")).toBeVisible();
		fireEvent.click(alpha);
		fireEvent.click(image, { shiftKey: true });
		expect(screen.getByText("2 items selected")).toBeVisible();
	});

	it("honors an initial picker location and allows folder targets", async () => {
		const onSelect = vi.fn();
		render(
			<FileManager
				instanceId="folder-picker"
				picker={{
					target: "folders",
					multiple: true,
					initialRootId: "usb",
					initialDirectory: "",
					onSelect,
					onCancel: vi.fn(),
				}}
			/>,
		);
		const breadcrumb = await screen.findByRole("navigation", {
			name: "Breadcrumb",
		});
		expect(breadcrumb).toHaveTextContent("Tour USB");
		const folder = await screen.findByRole("button", {
			name: "Folder, folder",
		});
		expect(mocks.server.fileEntries).toHaveBeenCalledWith("usb", "", false);
		fireEvent.click(folder);
		fireEvent.click(screen.getByRole("button", { name: "Select" }));
		expect(onSelect).toHaveBeenCalledWith([
			expect.objectContaining({
				rootId: "usb",
				entry: expect.objectContaining({ kind: "folder" }),
			}),
		]);
	});

	it("allows an either-target picker to return a deliberate multi-selection", async () => {
		const onSelect = vi.fn();
		render(
			<FileManager
				instanceId="either-picker"
				picker={{
					target: "either",
					multiple: true,
					onSelect,
					onCancel: vi.fn(),
				}}
			/>,
		);
		const folder = await screen.findByRole("button", {
			name: "Folder, folder",
		});
		fireEvent.click(folder);
		fireEvent.click(screen.getByRole("button", { name: "alpha.txt, file" }), {
			ctrlKey: true,
		});
		expect(onSelect).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Select" }));
		expect(onSelect).toHaveBeenCalledWith([
			expect.objectContaining({
				entry: expect.objectContaining({ kind: "folder" }),
			}),
			expect.objectContaining({
				entry: expect.objectContaining({ path: "alpha.txt" }),
			}),
		]);
	});

	it("opens a picker at its requested non-root directory", async () => {
		const onSelect = vi.fn();
		render(
			<FileManager
				instanceId="directory-picker"
				picker={{
					target: "files",
					initialRootId: "usb",
					initialDirectory: "Folder",
					allowedExtensions: ["txt"],
					onSelect,
					onCancel: vi.fn(),
				}}
			/>,
		);
		expect(
			await screen.findByRole("button", { name: "nested.txt, file" }),
		).toBeVisible();
		expect(
			screen.getByRole("navigation", { name: "Breadcrumb" }),
		).toHaveTextContent(/Tour USB.*Folder/);
		fireEvent.click(screen.getByRole("button", { name: "nested.txt, file" }));
		fireEvent.click(screen.getByRole("button", { name: "Select" }));
		expect(onSelect).toHaveBeenCalledWith([
			expect.objectContaining({
				rootId: "usb",
				entry: expect.objectContaining({ path: "Folder/nested.txt" }),
			}),
		]);
	});
});

describe("FileManager operation contracts", () => {
	beforeEach(resetFileManagerMocks);
	afterEach(cleanupFileManagerTest);

	it("replaces toolbar mutations with operation controls and confirms permanent delete", async () => {
		render(<FileManager instanceId="operations" />);
		const alpha = await screen.findByRole("button", {
			name: "alpha.txt, file",
		});
		fireEvent.click(alpha);
		chooseHeaderAction("Edit", "Copy");
		expect(screen.getByRole("button", { name: "Copy Here" })).toBeVisible();
		expect(screen.queryByRole("button", { name: "Rename" })).toBeNull();
		fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

		fireEvent.click(alpha);
		chooseHeaderAction("Edit", "Delete");
		expect(
			screen.getByRole("dialog", { name: "Confirm permanent deletion" }),
		).toHaveTextContent("This deletion is permanent");
		expect(mocks.server.fileOperation).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Delete Permanently" }));
		await waitFor(() =>
			expect(mocks.server.fileOperation).toHaveBeenCalledWith("shows", {
				operation: "delete",
				sources: ["alpha.txt"],
			}),
		);
	});

	it("offers Replace, Keep Both, Skip, and applies one decision to all conflicts", async () => {
		mocks.server.fileOperation.mockRejectedValueOnce(
			new Error('{"error":"one or more destination names already exist"}'),
		);
		render(<FileManager instanceId="conflict" />);
		fireEvent.click(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "image.png, file" }), {
			ctrlKey: true,
		});
		chooseHeaderAction("Edit", "Copy");
		fireEvent.click(screen.getByRole("button", { name: "Copy Here" }));
		const dialog = await screen.findByRole("dialog", {
			name: "Resolve name conflict",
		});
		expect(
			within(dialog).getByRole("button", { name: "Replace" }),
		).toBeVisible();
		expect(
			within(dialog).getByRole("button", { name: "Keep Both" }),
		).toBeVisible();
		expect(within(dialog).getByRole("button", { name: "Skip" })).toBeVisible();
		expect(within(dialog).getByLabelText("Apply to All")).toBeVisible();
		fireEvent.click(within(dialog).getByLabelText("Apply to All"));
		fireEvent.click(within(dialog).getByRole("button", { name: "Keep Both" }));
		await waitFor(() =>
			expect(mocks.server.fileOperation).toHaveBeenLastCalledWith(
				"shows",
				expect.objectContaining({
					operation: "copy",
					conflict: "keep_both",
					apply_to_all: true,
				}),
			),
		);
	});

	it("copies across configured roots through the server cross-root contract", async () => {
		render(<FileManager instanceId="cross-root" />);
		fireEvent.click(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		);
		chooseHeaderAction("Edit", "Copy");
		fireEvent.click(screen.getByRole("button", { name: /Tour USB/ }));
		await waitFor(() =>
			expect(mocks.server.fileEntries).toHaveBeenCalledWith("usb", "", false),
		);
		fireEvent.click(screen.getByRole("button", { name: "Copy Here" }));
		await waitFor(() =>
			expect(mocks.server.fileOperation).toHaveBeenCalledWith(
				"shows",
				expect.objectContaining({
					operation: "copy",
					sources: ["alpha.txt"],
					destination_root_id: "usb",
				}),
			),
		);
	});
});

describe("FileManager native metadata and Trash", () => {
	beforeEach(resetFileManagerMocks);
	afterEach(cleanupFileManagerTest);

	it("edits native notes and uses Trash only when the filesystem advertises it", async () => {
		mocks.server.fileEntries.mockImplementation(
			async (root: string, path: string) => ({
				root_id: root,
				path,
				entries: [
					{ ...entries[2], note_supported: true, trash_supported: true },
				],
			}),
		);
		mocks.server.readFileNote.mockResolvedValue({
			root_id: "shows",
			path: "alpha.txt",
			supported: true,
			note: "Old note",
		});
		render(<FileManager instanceId="native-metadata" />);
		fireEvent.click(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		);
		const notes = await screen.findByLabelText("Notes");
		expect(notes).toHaveValue("Old note");
		fireEvent.change(notes, { target: { value: "Updated note" } });
		fireEvent.click(screen.getByRole("button", { name: "Save Note" }));
		await waitFor(() =>
			expect(mocks.server.saveFileNote).toHaveBeenCalledWith(
				"shows",
				"alpha.txt",
				"Updated note",
			),
		);

		chooseHeaderAction("Edit", "Delete");
		const confirmation = screen.getByRole("dialog", {
			name: "Confirm move to trash",
		});
		expect(confirmation).toHaveTextContent("platform Trash");
		fireEvent.click(
			within(confirmation).getByRole("button", { name: "Move to Trash" }),
		);
		await waitFor(() =>
			expect(mocks.server.fileOperation).toHaveBeenCalledWith("shows", {
				operation: "trash",
				sources: ["alpha.txt"],
			}),
		);
	});

	it("reports a failed Trash operation and never retries it as permanent deletion", async () => {
		mocks.server.fileEntries.mockResolvedValue({
			root_id: "shows",
			path: "",
			entries: [{ ...entries[2], trash_supported: true }],
		});
		mocks.server.fileOperation.mockRejectedValue(
			new Error("platform Trash refused the item"),
		);
		render(<FileManager instanceId="failed-trash" />);
		fireEvent.click(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		);
		chooseHeaderAction("Edit", "Delete");
		fireEvent.click(screen.getByRole("button", { name: "Move to Trash" }));

		await waitFor(() =>
			expect(
				screen.getByText(/File operation failed.*Trash refused/i),
			).toBeVisible(),
		);
		expect(mocks.server.fileOperation).toHaveBeenCalledTimes(1);
		expect(mocks.server.fileOperation).toHaveBeenCalledWith("shows", {
			operation: "trash",
			sources: ["alpha.txt"],
		});
	});
});

describe("FileManager connection and input ownership", () => {
	beforeEach(resetFileManagerMocks);
	afterEach(cleanupFileManagerTest);

	it("falls back visibly when a removable root disconnects", async () => {
		vi.useFakeTimers();
		mocks.server.fileRoots
			.mockResolvedValueOnce([
				{
					id: "shows",
					label: "Shows",
					icon: "shows",
					removable: false,
					writable: true,
				},
				{
					id: "usb",
					label: "Tour USB",
					icon: "drive",
					removable: true,
					writable: true,
				},
			])
			.mockResolvedValue([
				{
					id: "shows",
					label: "Shows",
					icon: "shows",
					removable: false,
					writable: true,
				},
			]);
		render(
			<FileManager
				instanceId="disconnect"
				picker={{ initialRootId: "usb", onSelect: vi.fn(), onCancel: vi.fn() }}
			/>,
		);
		await act(async () => Promise.resolve());
		expect(
			screen.getByRole("navigation", { name: "Breadcrumb" }),
		).toHaveTextContent("Tour USB");

		await act(async () => vi.advanceTimersByTimeAsync(5_100));
		expect(screen.getByText(/location “usb” was disconnected/i)).toBeVisible();
		expect(
			screen.getByRole("navigation", { name: "Breadcrumb" }),
		).toHaveTextContent("Shows");
	});

	it("releases a claimed operation with a visible reason when the connection is lost", async () => {
		const view = render(<FileManager instanceId="connection-loss" />);
		fireEvent.click(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		);
		chooseHeaderAction("Edit", "Copy");
		await waitFor(() => expect(mocks.server.claimFileInput).toHaveBeenCalled());

		mocks.server.status = "disconnected";
		view.rerender(<FileManager instanceId="connection-loss" />);
		expect(
			screen.getByText(/cancelled because the desk connection was lost/i),
		).toBeVisible();
		expect(
			screen.queryByRole("button", { name: "Copy Here" }),
		).not.toBeInTheDocument();
		expect(mocks.server.releaseFileInput).toHaveBeenCalledWith(
			"connection-loss",
		);
	});
});

describe("FileManager input ownership", () => {
	beforeEach(resetFileManagerMocks);
	afterEach(cleanupFileManagerTest);

	it("claims an unowned desk action only after a pointer interaction inside this instance", async () => {
		mocks.server.commandLine = "COPY";
		const authority = createCommandLineTestAuthority({ text: "COPY" });
		render(authority.wrap(<FileManager instanceId="claim" />));
		await act(authority.settle);
		const manager = await screen.findByRole("region", { name: "File Manager" });
		fireEvent.focus(manager);
		expect(mocks.server.resetCommandLine).not.toHaveBeenCalled();
		fireEvent.pointerDown(
			within(manager).getByRole("heading", { name: "Locations" }),
		);
		await waitFor(() =>
			expect(mocks.server.claimFileInput).toHaveBeenCalledWith(
				"claim",
				"copy",
				"pending",
			),
		);
		await waitFor(() =>
			expect(authority.writes).toEqual([
				{
					deskId: authority.deskId,
					text: "",
					expectedRevision: 1,
				},
			]),
		);
		expect(mocks.server.resetCommandLine).not.toHaveBeenCalled();
		expect(
			await within(manager).findByRole("button", { name: "Copy Here" }),
		).toBeVisible();
		fireEvent.click(
			await within(manager).findByRole("button", { name: "alpha.txt, file" }),
		);
		fireEvent.keyDown(window, { key: "Enter" });
		await waitFor(() =>
			expect(mocks.server.fileOperation).toHaveBeenCalledWith(
				"shows",
				expect.objectContaining({ operation: "copy", sources: ["alpha.txt"] }),
			),
		);
	});

	it("lets only the clicked File Manager claim a pending action and leaves outside controls untouched", async () => {
		mocks.server.commandLine = "FIXTURE";
		const outside = vi.fn();
		render(
			<>
				<FileManager instanceId="first-manager" />
				<FileManager instanceId="second-manager" />
				<Button onClick={outside}>Lighting control</Button>
			</>,
		);
		const managers = await screen.findAllByRole("region", {
			name: "File Manager",
		});

		act(() => {
			window.dispatchEvent(
				new CustomEvent("light:desk-action", { detail: "copy" }),
			);
		});
		fireEvent.click(screen.getByRole("button", { name: "Lighting control" }));
		expect(outside).toHaveBeenCalledOnce();
		expect(mocks.server.claimFileInput).not.toHaveBeenCalled();

		act(() => {
			window.dispatchEvent(
				new CustomEvent("light:desk-action", { detail: "copy" }),
			);
		});
		fireEvent.pointerDown(
			within(managers[1]).getByRole("heading", { name: "Locations" }),
		);
		await waitFor(() =>
			expect(mocks.server.claimFileInput).toHaveBeenCalledWith(
				"second-manager",
				"copy",
				"pending",
			),
		);
		expect(
			within(managers[0]).queryByRole("button", { name: "Copy Here" }),
		).not.toBeInTheDocument();
		expect(
			within(managers[1]).getByRole("button", { name: "Copy Here" }),
		).toBeVisible();
	});
});

describe("FileManager owned input routing", () => {
	beforeEach(resetFileManagerMocks);
	afterEach(cleanupFileManagerTest);

	it.each([
		"keyboard",
		"touch",
		"osc",
	] as const)("routes matching ENTER and ESC behavior from %s while a File Manager owns input", async (source) => {
		const instanceId = `routing-${source}`;
		render(
			<>
				<FileManager instanceId={instanceId} />
				<Button data-keypad-key="ENT">Touch ENTER</Button>
				<Button data-keypad-key="ESC">Touch ESC</Button>
			</>,
		);
		const alpha = await screen.findByRole("button", {
			name: "alpha.txt, file",
		});
		const route = (action: "enter" | "escape") => {
			if (source === "keyboard")
				fireEvent.keyDown(window, {
					key: action === "enter" ? "Enter" : "Escape",
				});
			else if (source === "touch")
				fireEvent.click(
					screen.getByRole("button", {
						name: action === "enter" ? "Touch ENTER" : "Touch ESC",
					}),
				);
			else
				act(() => {
					window.dispatchEvent(
						new CustomEvent("light:file-manager-input", {
							detail: { instance_id: instanceId, action },
						}),
					);
				});
		};

		fireEvent.click(alpha);
		chooseHeaderAction("Edit", "Copy");
		route("escape");
		expect(
			screen.queryByRole("button", { name: "Copy Here" }),
		).not.toBeInTheDocument();
		expect(mocks.server.releaseFileInput).toHaveBeenCalledWith(instanceId);

		fireEvent.click(alpha);
		chooseHeaderAction("Edit", "Copy");
		route("enter");
		await waitFor(() =>
			expect(mocks.server.fileOperation).toHaveBeenCalledWith(
				"shows",
				expect.objectContaining({ operation: "copy", sources: ["alpha.txt"] }),
			),
		);
	});
});

describe("FileManager external editor synchronization", () => {
	beforeEach(resetFileManagerMocks);
	afterEach(cleanupFileManagerTest);

	it("reflects a clean save from a dedicated Text Editor", async () => {
		render(<FileManager instanceId="file-manager" />);
		fireEvent.click(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Edit Text" }));
		const editor = await screen.findByLabelText("File text");
		expect(editor).toHaveValue("Alpha");

		act(() => {
			window.dispatchEvent(
				new CustomEvent("light:text-file-saved", {
					detail: {
						document: {
							root_id: "shows",
							path: "alpha.txt",
							text: "Updated elsewhere",
							revision: "2",
							read_only: false,
						},
						sourcePaneId: "text-editor",
					},
				}),
			);
		});

		expect(editor).toHaveValue("Updated elsewhere");
		expect(
			screen.getByText(/File Manager editor has been updated/),
		).toBeVisible();
	});

	it("preserves a dirty File Manager draft when another editor saves", async () => {
		render(<FileManager instanceId="file-manager" />);
		fireEvent.click(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Edit Text" }));
		const editor = await screen.findByLabelText("File text");
		fireEvent.change(editor, { target: { value: "My File Manager draft" } });

		act(() => {
			window.dispatchEvent(
				new CustomEvent("light:text-file-saved", {
					detail: {
						document: {
							root_id: "shows",
							path: "alpha.txt",
							text: "Updated elsewhere",
							revision: "2",
							read_only: false,
						},
						sourcePaneId: "text-editor",
					},
				}),
			);
		});

		expect(editor).toHaveValue("My File Manager draft");
		expect(
			screen.getByText("Conflict", { selector: "span[role=status]" }),
		).toBeVisible();
		expect(screen.getByRole("alert")).toHaveTextContent("newer file revision");
		expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
	});
});

describe("FileManager moved and deleted editor files", () => {
	beforeEach(resetFileManagerMocks);
	afterEach(cleanupFileManagerTest);

	it("follows a moved file in the embedded editor and keeps its dirty text", async () => {
		render(<FileManager instanceId="file-manager" />);
		fireEvent.click(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Edit Text" }));
		const editor = await screen.findByLabelText("File text");
		fireEvent.change(editor, { target: { value: "Draft after move" } });

		act(() => {
			window.dispatchEvent(
				new CustomEvent("light:file-operation", {
					detail: {
						operation: "move",
						items: [
							{
								source_root_id: "shows",
								source: "alpha.txt",
								destination_root_id: "usb",
								destination: "notes/renamed.txt",
								status: "completed",
								error: null,
							},
						],
					},
				}),
			);
		});

		expect(editor).toHaveValue("Draft after move");
		expect(
			screen.getByText("notes/renamed.txt", { selector: ".file-editor b" }),
		).toBeVisible();
		expect(
			screen.getByText("Unsaved", {
				selector: ".file-editor span[role=status]",
			}),
		).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Save" }));
		await waitFor(() =>
			expect(mocks.server.saveTextFile).toHaveBeenCalledWith(
				"usb",
				"notes/renamed.txt",
				"Draft after move",
				"1",
			),
		);
	});

	it("retains and can recreate an embedded editor file after deletion", async () => {
		vi.spyOn(window, "confirm").mockReturnValue(true);
		render(<FileManager instanceId="file-manager" />);
		fireEvent.click(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Edit Text" }));
		const editor = await screen.findByLabelText("File text");

		act(() => {
			window.dispatchEvent(
				new CustomEvent("light:file-operation", {
					detail: {
						operation: "delete",
						items: [
							{
								source_root_id: "shows",
								source: "alpha.txt",
								destination_root_id: null,
								destination: null,
								status: "completed",
								error: null,
							},
						],
					},
				}),
			);
		});

		expect(editor).toHaveValue("Alpha");
		expect(editor).toHaveAttribute("readonly");
		expect(
			screen.getByText("Missing", {
				selector: ".file-editor span[role=status]",
			}),
		).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Recreate File" }));
		await waitFor(() =>
			expect(mocks.server.saveTextFile).toHaveBeenCalledWith(
				"shows",
				"alpha.txt",
				"Alpha",
				null,
			),
		);
		expect(
			screen.getByText("Saved", { selector: ".file-editor span[role=status]" }),
		).toBeVisible();
	});
});

describe("FileManager dirty pane guard", () => {
	beforeEach(resetFileManagerMocks);
	afterEach(cleanupFileManagerTest);

	it("protects a dirty embedded editor when its File Manager pane is removed", async () => {
		vi.spyOn(window, "confirm").mockReturnValue(false);
		render(<FileManager instanceId="manager-pane" paneId="manager-pane" />);
		fireEvent.click(
			await screen.findByRole("button", { name: "alpha.txt, file" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Edit Text" }));
		fireEvent.change(await screen.findByLabelText("File text"), {
			target: { value: "Unsaved embedded draft" },
		});

		expect(requestPaneRemoval("manager-pane")).toBe(false);
		expect(window.confirm).toHaveBeenCalledWith(
			"File Manager has unsaved text changes.\n\nRemove this pane and discard those changes?",
		);
	});
});
