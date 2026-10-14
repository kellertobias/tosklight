import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FixtureLibrarySetup } from "./FixtureLibrarySetup";
import { blankFixtureProfile } from "./fixtureProfileModel";

const mocks = vi.hoisted(() => ({
	library: {
		fixtureProfiles: [] as unknown[],
		fixtureLibrary: [],
		fixtureProfileWarnings: [],
		exportFixturePackage: vi.fn(),
		exportFixturePackageFile: vi.fn(),
	},
	files: { fileRoots: vi.fn(), fileEntries: vi.fn() },
	objectUrl: vi.fn(),
	anchorClick: vi.fn(),
}));
vi.mock("../../features/fixtureLibrary/FixtureLibraryContext", () => ({
	useFixtureLibrary: () => mocks.library,
}));
vi.mock("../../features/files/FilesContext", () => ({
	useFiles: () => mocks.files,
}));
vi.mock("../../api/ServerContext", () => ({ useServer: () => mocks.library }));

beforeEach(() => {
	const profile = blankFixtureProfile();
	profile.id = "export-profile";
	profile.revision = 7;
	profile.manufacturer = "Acme";
	profile.name = "Transfer";
	profile.short_name = "Transfer";
	mocks.library.fixtureProfiles = [profile];
	mocks.library.exportFixturePackage
		.mockReset()
		.mockResolvedValue(new Blob([new Uint8Array([80, 75, 3, 4])]));
	mocks.library.exportFixturePackageFile.mockReset();
	mocks.files.fileRoots.mockReset().mockResolvedValue([
		{
			id: "readonly",
			label: "Read only",
			writable: false,
			icon: "folder",
			removable: false,
		},
		{
			id: "shows",
			label: "Shows",
			writable: true,
			icon: "folder",
			removable: false,
		},
	]);
	mocks.files.fileEntries.mockReset().mockResolvedValue({
		root_id: "shows",
		path: "",
		entries: [
			{ name: "exports", path: "exports", kind: "folder", writable: true },
		],
	});
	mocks.objectUrl.mockReset().mockReturnValue("blob:test");
	vi.stubGlobal(
		"URL",
		Object.assign(URL, {
			createObjectURL: mocks.objectUrl,
			revokeObjectURL: vi.fn(),
		}),
	);
	vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(
		mocks.anchorClick,
	);
});
afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
});

function openExport() {
	const rendered = render(<FixtureLibrarySetup />, { wrapper: ModalProvider });
	fireEvent.click(screen.getByRole("button", { name: "Export fixture" }));
	return rendered;
}

describe("fixture package explicit export", () => {
	it("uses an explicit confined destination, locks while saving and reports its exact result", async () => {
		let finish!: (value: unknown) => void;
		mocks.library.exportFixturePackageFile.mockReturnValue(
			new Promise((resolve) => {
				finish = resolve;
			}),
		);
		const rendered = openExport();
		expect(
			screen.getByRole("dialog", { name: "Export fixture" }),
		).toHaveAttribute("aria-modal", "true");
		expect(
			screen.getByRole("heading", { name: "Export fixture" }),
		).toBeInTheDocument();
		await screen.findByRole("button", { name: "exports" });
		fireEvent.click(screen.getByRole("button", { name: "exports" }));
		fireEvent.change(screen.getByLabelText("File name"), {
			target: { value: "transfer.toskfixture" },
		});
		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "Save fixture package" }),
			).toBeEnabled(),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Save fixture package" }),
		);
		expect(screen.getByText("Saving fixture package…")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Cancel Export fixture" }),
		).toBeDisabled();
		expect(
			screen.getByRole("button", { name: "Cancel Export fixture" }),
		).toBeDisabled();
		fireEvent.keyDown(document, { key: "Escape" });
		fireEvent.pointerDown(
			screen.getByRole("dialog", { name: "Export fixture" }).parentElement!,
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Save fixture package" }),
		);
		expect(
			screen.getByRole("dialog", { name: "Export fixture" }),
		).toBeInTheDocument();
		expect(mocks.library.exportFixturePackageFile).toHaveBeenCalledOnce();
		const refreshed = {
			...(mocks.library.fixtureProfiles[0] as object),
			revision: 8,
		};
		mocks.library.fixtureProfiles = [refreshed];
		rendered.rerender(<FixtureLibrarySetup />);
		expect(
			screen.getByRole("dialog", { name: "Export fixture" }),
		).toHaveTextContent("Revision 7");
		expect(mocks.library.exportFixturePackageFile).toHaveBeenCalledWith(
			"export-profile",
			7,
			{ root_id: "shows", path: "exports", name: "transfer.toskfixture" },
		);
		await act(async () =>
			finish({
				root_id: "shows",
				path: "exports/transfer.toskfixture",
				profile_id: "export-profile",
				revision: 7,
				bytes: 4,
				replayed: false,
				request_id: "saved",
			}),
		);
		expect(
			await screen.findByText("Saved Shows / exports/transfer.toskfixture"),
		).toBeInTheDocument();
		expect(mocks.objectUrl).not.toHaveBeenCalled();
		expect(mocks.library.exportFixturePackage).not.toHaveBeenCalled();
	});
	it("cancels without exporting or writing and keeps cancellation visible", async () => {
		openExport();
		await screen.findByRole("button", { name: "exports" });
		fireEvent.click(
			screen.getByRole("button", { name: "Cancel Export fixture" }),
		);
		expect(
			screen.queryByRole("heading", { name: "Export fixture" }),
		).not.toBeInTheDocument();
		expect(screen.getByText("Export cancelled.")).toBeInTheDocument();
		expect(mocks.library.exportFixturePackageFile).not.toHaveBeenCalled();
		expect(mocks.library.exportFixturePackage).not.toHaveBeenCalled();
	});
	it("refuses to save when the confined destination cannot be read", async () => {
		mocks.files.fileRoots.mockRejectedValue(
			new Error("Destination is unavailable"),
		);
		openExport();
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"Destination is unavailable",
		);
		expect(
			screen.getByRole("button", { name: "Save fixture package" }),
		).toBeDisabled();
		expect(mocks.library.exportFixturePackageFile).not.toHaveBeenCalled();
	});
	it("keeps a local failure in the export dialog for a deliberate retry", async () => {
		mocks.library.exportFixturePackageFile.mockRejectedValue(
			new Error("A file with that name already exists"),
		);
		openExport();
		await screen.findByRole("button", { name: "exports" });
		fireEvent.click(
			screen.getByRole("button", { name: "Save fixture package" }),
		);
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"A file with that name already exists",
		);
		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "Save fixture package" }),
			).toBeEnabled(),
		);
		expect(
			screen.getByRole("heading", { name: "Export fixture" }),
		).toBeInTheDocument();
	});
	it.each([
		"Escape",
		"backdrop",
	])("cancels via %s without writing", async (gesture) => {
		openExport();
		await screen.findByRole("button", { name: "exports" });
		if (gesture === "Escape") fireEvent.keyDown(document, { key: "Escape" });
		else
			fireEvent.pointerDown(
				screen.getByRole("dialog", { name: "Export fixture" }).parentElement!,
			);
		expect(
			screen.queryByRole("dialog", { name: "Export fixture" }),
		).not.toBeInTheDocument();
		expect(screen.getByText("Export cancelled.")).toBeInTheDocument();
		expect(mocks.library.exportFixturePackageFile).not.toHaveBeenCalled();
	});
	it("keeps the selected immutable revision when library selection refreshes before Save", async () => {
		mocks.library.exportFixturePackageFile.mockResolvedValue({
			path: "exact.toskfixture",
		});
		const rendered = openExport();
		await screen.findByRole("button", { name: "exports" });
		mocks.library.fixtureProfiles = [
			{ ...(mocks.library.fixtureProfiles[0] as object), revision: 8 },
		];
		rendered.rerender(<FixtureLibrarySetup />);
		expect(
			screen.getByRole("dialog", { name: "Export fixture" }),
		).toHaveTextContent("Revision 7");
		fireEvent.click(
			screen.getByRole("button", { name: "Save fixture package" }),
		);
		await waitFor(() =>
			expect(mocks.library.exportFixturePackageFile).toHaveBeenCalledWith(
				"export-profile",
				7,
				expect.any(Object),
			),
		);
	});
});
