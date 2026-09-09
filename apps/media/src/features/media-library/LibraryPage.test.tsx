import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ICON_CATALOG_GROUPS } from "@tosklight/ui/controls";
import { ModalProvider } from "@tosklight/ui/modals";
import { DEFAULT_POOL_COLOR_PALETTE } from "@tosklight/ui/pools";
import { afterEach, describe, expect, it, vi } from "vitest";
import { aCatalog, stubServer } from "../../testing/server";
import {
	allocateFreeAddresses,
	draggedItemIds,
	isAcceptedMediaFile,
	LibraryBrowserView,
	LibraryPage,
} from "./LibraryPage";

afterEach(() => vi.unstubAllGlobals());

describe("the CITP media library", () => {
	it("filters the media folder pool to playable and parking storage", async () => {
		stubServer();
		const { container } = render(<LibraryPage />);
		expect(
			(await screen.findByText("Looks")).closest("button"),
		).toBeInTheDocument();
		expect(container.querySelector(".media-library-folder-pool")).toHaveClass(
			"ui-button-grid",
			"square-grid",
		);
		expect(container.querySelector(".media-library-folder-pool")).toHaveStyle({
			"--grid-cell-min": "68px",
		});
		expect(
			screen.getByRole("button", { name: /199Empty folder/u }),
		).toBeInTheDocument();
		expect(
			container.querySelector('.media-library-folders [data-folder="200"]'),
		).not.toBeInTheDocument();
		expect(
			container.querySelector('.media-library-folders [data-folder="255"]'),
		).not.toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /900Parking0\/254/u }),
		).toBeInTheDocument();
		expect(screen.getByText("Blue haze").closest("button")).toBeInTheDocument();
		expect(screen.getByRole("tablist")).toHaveTextContent(
			"MediaVisualizersText",
		);
		const newMedia = screen.getByRole("button", { name: "New media" });
		const tabs = screen.getByRole("tablist");
		expect(
			newMedia.compareDocumentPosition(tabs) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
	});

	it("opens the first free exact address from the leftmost New media action", async () => {
		stubServer();
		render(<LibraryPage />);

		await userEvent.click(
			await screen.findByRole("button", { name: "New media" }),
		);

		expect(screen.getByRole("heading", { name: "Empty" })).toBeInTheDocument();
		expect(screen.getByText("001 / 003")).toBeInTheDocument();
	});

	it("opens one media item in the preview editor and persists name and BPM", async () => {
		const server = stubServer();
		render(
			<ModalProvider>
				<LibraryPage />
			</ModalProvider>,
		);
		const media = (await screen.findByText("Blue haze")).closest("button");
		if (!media) throw new Error("media card missing");
		await userEvent.click(media);
		const preview = screen.getByRole("img", { name: "Blue haze preview" });
		expect(preview).toHaveAttribute(
			"src",
			"/api/v2/library/1/1/thumbnail?revision=3",
		);
		expect(preview.parentElement).toHaveClass("media-library-item-preview");
		const editor = screen
			.getByRole("button", { name: "Save media" })
			.closest("form");
		const inputs = editor?.querySelectorAll("input");
		if (!inputs || inputs.length < 2)
			throw new Error("media editor fields missing");
		await userEvent.clear(inputs[0]);
		await userEvent.type(inputs[0], "Opening haze");
		await userEvent.clear(inputs[1]);
		await userEvent.type(inputs[1], "128");
		await userEvent.click(screen.getByRole("button", { name: "Save media" }));
		await vi.waitFor(() => expect(server.writes).toHaveLength(2));
		expect(server.writes).toEqual([
			"/library/items/asset-a/update",
			"/library/items/asset-a/update",
		]);
		const bodies = vi
			.mocked(fetch)
			.mock.calls.filter(
				([input, init]) =>
					String(input).includes("/library/items/asset-a/update") &&
					init?.method === "POST",
			)
			.map(([, init]) => JSON.parse(String(init?.body)));
		expect(bodies).toMatchObject([
			{ name: "Opening haze", swap: false },
			{ intrinsicBpm: 128, swap: false },
		]);
		expect(bodies[0]).not.toHaveProperty("intrinsicBpm");
		expect(bodies[1]).not.toHaveProperty("name");
		expect(server.catalog.folders[0].items[0]).toMatchObject({
			name: "Opening haze",
			intrinsicBpm: 128,
		});
	});

	it("disables and re-enables one media file without removing it", async () => {
		const server = stubServer();
		render(<LibraryPage />);
		const media = (await screen.findByText("Blue haze")).closest("button");
		if (!media) throw new Error("media card missing");
		await userEvent.click(media);

		const enabled = screen.getByRole("switch", { name: "Enabled" });
		expect(enabled).toBeChecked();
		await userEvent.click(enabled);
		await vi.waitFor(() =>
			expect(server.catalog.folders[0].items[0].enabled).toBe(false),
		);
		expect(screen.getByText("Disabled")).toBeInTheDocument();
		expect(screen.getByRole("switch", { name: "Enabled" })).not.toBeChecked();

		await userEvent.click(screen.getByRole("switch", { name: "Enabled" }));
		await vi.waitFor(() =>
			expect(server.catalog.folders[0].items[0].enabled).toBe(true),
		);
		expect(server.catalog.folders[0].items).toHaveLength(2);
	});

	it("retries and uploads a custom thumbnail for the selected stable media file", async () => {
		const server = stubServer();
		const { container } = render(<LibraryPage />);
		const media = (await screen.findByText("Blue haze")).closest("button");
		if (!media) throw new Error("media card missing");
		await userEvent.click(media);

		await userEvent.click(
			screen.getByRole("button", { name: "Retry thumbnail" }),
		);
		await vi.waitFor(() =>
			expect(server.writes).toContain("/library/items/asset-a/thumbnail/retry"),
		);
		expect(
			screen.getByRole("img", { name: "Blue haze preview" }),
		).toHaveAttribute("src", "/api/v2/library/1/1/thumbnail?revision=4");

		await userEvent.click(
			screen.getByRole("button", { name: "Upload custom thumbnail" }),
		);
		const picker = container.querySelector<HTMLInputElement>(
			'input[type="file"][accept="image/png,image/jpeg,image/gif,image/webp"]',
		);
		if (!picker) throw new Error("custom thumbnail picker missing");
		fireEvent.change(picker, {
			target: {
				files: [new File(["image"], "custom.png", { type: "image/png" })],
			},
		});
		await vi.waitFor(() =>
			expect(
				server.writes.some((path) =>
					path.startsWith("/library/items/asset-a/thumbnail/upload?requestId="),
				),
			).toBe(true),
		);
		await vi.waitFor(() =>
			expect(
				screen.getByRole("img", { name: "Blue haze preview" }),
			).toHaveAttribute("src", "/api/v2/library/1/1/thumbnail?revision=5"),
		);
	});

	it("requires confirmation before permanently deleting one media file", async () => {
		const server = stubServer();
		const confirm = vi
			.fn()
			.mockReturnValueOnce(false)
			.mockReturnValueOnce(true);
		vi.stubGlobal("confirm", confirm);
		render(<LibraryPage />);
		const media = (await screen.findByText("Blue haze")).closest("button");
		if (!media) throw new Error("media card missing");
		await userEvent.click(media);

		await userEvent.click(screen.getByRole("button", { name: "Delete media" }));
		expect(server.catalog.itemCount).toBe(2);
		expect(server.writes).not.toContain("/library/items/asset-a/delete");

		await userEvent.click(screen.getByRole("button", { name: "Delete media" }));
		await vi.waitFor(() => expect(server.catalog.itemCount).toBe(1));
		expect(server.catalog.folders[0].items.map((item) => item.id)).toEqual([
			"asset-b",
		]);
		expect(confirm).toHaveBeenCalledWith(
			"Delete Blue haze permanently? This cannot be undone.",
		);
	});

	it("stores one exact licence note on multiple selected media files", async () => {
		const server = stubServer();
		server.catalog.folders[0].items[0].note = "Source A";
		server.catalog.folders[0].items[1].note = "Source B";
		render(<LibraryPage />);

		const first = (await screen.findByText("Blue haze")).closest("button");
		const second = screen.getByText("Static grid").closest("button");
		if (!first || !second) throw new Error("media cards missing");
		await userEvent.click(first);
		fireEvent.click(second, { ctrlKey: true });

		expect(screen.getByText("Multiple values")).toBeInTheDocument();
		const note = screen.getByLabelText("Note for 2 media files");
		await userEvent.type(note, "Licence: CC BY 4.0\nCreator: Example");
		await userEvent.click(
			screen.getByRole("button", { name: "Save note to 2" }),
		);

		await vi.waitFor(() =>
			expect(server.catalog.folders[0].items.map((item) => item.note)).toEqual([
				"Licence: CC BY 4.0\nCreator: Example",
				"Licence: CC BY 4.0\nCreator: Example",
			]),
		);
		expect(server.writes).toContain("/library/notes/update");
	});

	it("selects occupied file ranges across gaps and adds later Shift ranges", async () => {
		const catalog = aCatalog();
		const template = catalog.folders[0].items[1];
		catalog.folders[0].items = [
			{ ...catalog.folders[0].items[0], file: 1, name: "Range one" },
			{ ...template, id: "asset-b", file: 3, name: "Range three" },
			{ ...template, id: "asset-c", file: 5, name: "Range five" },
			{ ...template, id: "asset-d", file: 8, name: "Range eight" },
			{ ...template, id: "asset-e", file: 10, name: "Range ten" },
		];
		catalog.itemCount = 5;
		render(<LibraryBrowserView catalog={catalog} />);

		fireEvent.click(screen.getByText("Range one").closest("button") as Element);
		fireEvent.click(
			screen.getByText("Range five").closest("button") as Element,
			{
				shiftKey: true,
			},
		);
		expect(screen.getByText("3 selected")).toBeInTheDocument();

		fireEvent.click(
			screen.getByText("Range eight").closest("button") as Element,
			{
				ctrlKey: true,
			},
		);
		fireEvent.click(
			screen.getByText("Range ten").closest("button") as Element,
			{
				shiftKey: true,
			},
		);

		expect(screen.getByText("5 selected")).toBeInTheDocument();
		for (const name of [
			"Range one",
			"Range three",
			"Range five",
			"Range eight",
			"Range ten",
		]) {
			expect(screen.getByText(name).closest("button")).toHaveClass("selected");
		}
	});

	it("enables, disables, and deletes one selected set through bulk intents", async () => {
		const server = stubServer();
		const confirm = vi
			.fn()
			.mockReturnValueOnce(false)
			.mockReturnValueOnce(true);
		vi.stubGlobal("confirm", confirm);
		render(<LibraryPage />);

		const first = (await screen.findByText("Blue haze")).closest("button");
		const second = screen.getByText("Static grid").closest("button");
		if (!first || !second) throw new Error("media cards missing");
		fireEvent.click(first);
		fireEvent.click(second, { ctrlKey: true });

		await userEvent.click(
			screen.getByRole("button", { name: "Disable selected" }),
		);
		await vi.waitFor(() =>
			expect(
				server.catalog.folders[0].items.map((item) => item.enabled),
			).toEqual([false, false]),
		);
		expect(server.writes.at(-1)).toBe("/library/items/update");
		expect(server.writeBodies.at(-1)).toMatchObject({
			ids: ["asset-a", "asset-b"],
			enabled: false,
		});

		await userEvent.click(
			screen.getByRole("button", { name: "Enable selected" }),
		);
		await vi.waitFor(() =>
			expect(
				server.catalog.folders[0].items.map((item) => item.enabled),
			).toEqual([true, true]),
		);

		await userEvent.click(
			screen.getByRole("button", { name: "Delete selected" }),
		);
		expect(server.catalog.itemCount).toBe(2);
		await userEvent.click(
			screen.getByRole("button", { name: "Delete selected" }),
		);
		await vi.waitFor(() => expect(server.catalog.itemCount).toBe(0));
		expect(
			server.writes.filter((path) => path === "/library/items/delete"),
		).toHaveLength(1);
		expect(confirm).toHaveBeenCalledWith(
			"Delete 2 selected media files permanently? This cannot be undone.",
		);
	});

	it("adds and clears one note across multiple media folders", async () => {
		const server = stubServer();
		render(<LibraryPage />);

		const first = (await screen.findByText("Looks")).closest("button");
		const second = screen.getByRole("button", { name: /002Empty folder/u });
		if (!first) throw new Error("folder card missing");
		await userEvent.click(first);
		fireEvent.click(second, { ctrlKey: true });

		const note = screen.getByLabelText("Note for 2 folders");
		await userEvent.type(note, "Purchased stock library licence");
		await userEvent.click(
			screen.getByRole("button", { name: "Save note to 2" }),
		);
		await vi.waitFor(() =>
			expect(
				server.catalog.folders
					.filter((folder) => folder.folder === 1 || folder.folder === 2)
					.map((folder) => folder.note),
			).toEqual([
				"Purchased stock library licence",
				"Purchased stock library licence",
			]),
		);

		await userEvent.click(screen.getByRole("button", { name: "Clear notes" }));
		await vi.waitFor(() =>
			expect(
				server.catalog.folders
					.filter((folder) => folder.folder === 1 || folder.folder === 2)
					.map((folder) => folder.note),
			).toEqual([null, null]),
		);
	});

	it("frames the larger inspector preview at the configured output ratio", async () => {
		render(
			<LibraryBrowserView
				catalog={aCatalog()}
				thumbnailUrl={() => "portrait-preview.png"}
				previewAspectRatio={4 / 3}
			/>,
		);
		await userEvent.click(
			screen.getByText("Blue haze").closest("button") as HTMLButtonElement,
		);

		expect(
			screen.getByRole("img", { name: "Blue haze preview" }).parentElement,
		).toHaveStyle({ aspectRatio: String(4 / 3) });
	});

	it("clicking a media folder opens name, icon, and upload configuration", async () => {
		const server = stubServer();
		render(
			<ModalProvider>
				<LibraryPage />
			</ModalProvider>,
		);
		const folder = (await screen.findByText("Looks")).closest("button");
		if (!folder) throw new Error("folder button missing");
		await userEvent.click(folder);
		expect(screen.getByText("Folder icon")).toBeInTheDocument();
		expect(screen.getByText("Upload media to this folder")).toBeInTheDocument();
		await userEvent.clear(screen.getByLabelText("Folder name"));
		await userEvent.type(screen.getByLabelText("Folder name"), "Act one");
		expect(
			screen.queryByRole("button", { name: "Save folder" }),
		).not.toBeInTheDocument();
		await vi.waitFor(() =>
			expect(server.catalog.folders[0].name).toBe("Act one"),
		);
		const catalogGroup = ICON_CATALOG_GROUPS.find(
			(group) => group.id !== "built-in" && group.icons.length > 0,
		);
		const catalogIcon = catalogGroup?.icons[0];
		if (!catalogGroup || !catalogIcon)
			throw new Error("full icon catalog missing");
		await userEvent.click(screen.getByRole("button", { name: "Choose icon" }));
		await userEvent.click(screen.getByRole("button", { name: "Icon group" }));
		await userEvent.click(
			screen.getByRole("option", { name: catalogGroup.label }),
		);
		await userEvent.click(
			screen.getByRole("button", { name: catalogIcon.label }),
		);
		await vi.waitFor(() =>
			expect(server.catalog.folders[0].icon).toBe(catalogIcon.value),
		);
	});

	it("persists a folder name when the operator immediately leaves the folder", async () => {
		const server = stubServer();
		render(<LibraryPage />);
		const first = (await screen.findByText("Looks")).closest("button");
		const second = screen.getByRole("button", { name: /002Empty folder/u });
		if (!first) throw new Error("folder button missing");

		await userEvent.click(first);
		await userEvent.clear(screen.getByLabelText("Folder name"));
		await userEvent.type(screen.getByLabelText("Folder name"), "Act one");
		await userEvent.click(second);

		await vi.waitFor(() =>
			expect(server.catalog.folders[0].name).toBe("Act one"),
		);
		await userEvent.click(first);
		expect(screen.getByLabelText("Folder name")).toHaveValue("Act one");
		expect(first).toHaveTextContent("Act one");
	});

	it("de-emphasizes empty playable folders without muting populated folders", async () => {
		stubServer();
		render(<LibraryPage />);
		const populated = (await screen.findByText("Looks")).closest("button");
		const empty = screen.getByRole("button", { name: /002Empty folder/u });
		if (!populated) throw new Error("populated folder button missing");

		expect(populated).not.toHaveClass("empty");
		expect(populated.style.getPropertyValue("--pool-card-color")).toBe(
			DEFAULT_POOL_COLOR_PALETTE.group,
		);
		expect(empty).toHaveClass("empty");
		expect(empty.style.getPropertyValue("--pool-card-color")).toBe("");
	});

	it("compacts the selected folder into consecutive slots", async () => {
		const catalog = aCatalog();
		catalog.folders[0].items[0].file = 3;
		catalog.folders[0].items[1].file = 8;
		const server = stubServer({ catalog });
		render(<LibraryPage />);
		const folder = (await screen.findByText("Looks")).closest("button");
		if (!folder) throw new Error("folder button missing");

		await userEvent.click(folder);
		await userEvent.click(
			screen.getByRole("button", { name: "Compact files" }),
		);

		await vi.waitFor(() =>
			expect(server.catalog.folders[0].items.map((item) => item.file)).toEqual([
				1, 2,
			]),
		);
		expect(server.writes).toContain("/library/folders/1/update");
		expect(server.writeBodies).toContainEqual(
			expect.objectContaining({ compact: true }),
		);
	});

	it("uploads, replaces, and removes a folder picture", async () => {
		const server = stubServer();
		render(<LibraryPage />);
		await userEvent.click(
			(await screen.findByText("Looks")).closest("button")!,
		);

		const pictureInput = document.querySelector(
			'.media-folder-presentation-editor input[accept="image/*"]',
		) as HTMLInputElement;
		fireEvent.change(pictureInput, {
			target: {
				files: [new File(["picture"], "folder.png", { type: "image/png" })],
			},
		});
		await vi.waitFor(() =>
			expect(server.folderPresentations.folders[0].pictureUrl).toContain(
				"/folder-presentations/1/picture",
			),
		);
		await userEvent.click(
			await screen.findByRole("button", { name: "Remove folder picture" }),
		);
		await vi.waitFor(() =>
			expect(server.folderPresentations.folders[0].pictureUrl).toBeNull(),
		);
	});

	it("uses Playback's responsive media-card grid without losing draggable slots", () => {
		const { container } = render(
			<LibraryBrowserView
				catalog={aCatalog()}
				thumbnailUrl={() => "preview.png"}
			/>,
		);
		const grid = container.querySelector(
			".media-file-pool-grid.media-library-file-pool-grid",
		) as HTMLElement | null;
		expect(grid).not.toBeNull();
		expect(grid?.style.getPropertyValue("--grid-cell-min")).toBe("112px");
		expect(grid?.style.gridTemplateColumns).toBe("");
		expect(screen.getByText("Blue haze").closest("button")).toHaveAttribute(
			"draggable",
			"true",
		);
	});

	it("opens an empty slot and uploads a named file to that exact address", async () => {
		const onUploadAt = vi.fn();
		const { container } = render(
			<LibraryBrowserView
				catalog={aCatalog()}
				onUploadAt={onUploadAt}
				thumbnailUrl={() => "preview.png"}
			/>,
		);
		const empty = container.querySelector(
			'.media-library-pool .pool-card.empty[data-pool-position="2"]',
		) as HTMLButtonElement | null;
		expect(empty).not.toBeNull();
		await userEvent.click(empty as HTMLButtonElement);
		expect(screen.getByRole("heading", { name: "Empty" })).toBeInTheDocument();
		expect(screen.getByText("001 / 003")).toBeInTheDocument();
		const nameInput = container.querySelector(
			'.media-library-inspector input[type="text"]',
		) as HTMLInputElement;
		await userEvent.type(nameInput, "New loop");
		const input = container.querySelector(
			'.media-library-inspector input[type="file"]',
		) as HTMLInputElement;
		const file = new File(["pixels"], "loop.mov", { type: "video/quicktime" });
		fireEvent.change(input, { target: { files: [file] } });
		expect(onUploadAt).toHaveBeenCalledWith(
			file,
			{ folder: 1, file: 3 },
			"New loop",
			false,
		);
	});

	it("replaces filled media without changing its address", async () => {
		const onUploadAt = vi.fn();
		const { container } = render(
			<LibraryBrowserView
				catalog={aCatalog()}
				onUploadAt={onUploadAt}
				thumbnailUrl={() => "preview.png"}
			/>,
		);
		await userEvent.click(
			screen.getByText("Blue haze").closest("button") as HTMLButtonElement,
		);
		const input = container.querySelector(
			'.media-library-inspector input[type="file"]',
		) as HTMLInputElement;
		const file = new File(["new pixels"], "replacement.mov", {
			type: "video/quicktime",
		});
		fireEvent.change(input, { target: { files: [file] } });
		expect(onUploadAt).toHaveBeenCalledWith(
			file,
			{ folder: 1, file: 1 },
			"Blue haze",
			true,
		);
	});

	it("centres the empty-folder message over an empty media grid", async () => {
		stubServer();
		render(<LibraryPage />);
		await userEvent.click(
			await screen.findByRole("button", { name: /002Empty folder/u }),
		);
		const empty = screen.getByText("This folder is empty").closest("div");
		expect(empty).toHaveClass("media-library-empty-folder");
		expect(empty).toHaveTextContent("This folder is empty");
	});

	it("right-clicking a different folder keeps that folder's editor open", async () => {
		const onRenameFolder = vi.fn();
		render(
			<LibraryBrowserView
				catalog={aCatalog()}
				onRenameFolder={onRenameFolder}
				thumbnailUrl={() => "preview.png"}
			/>,
		);
		fireEvent.contextMenu(
			screen.getByRole("button", { name: /002Empty folder/u }),
		);
		expect(
			screen.getByRole("heading", { name: "Configure folder" }),
		).toBeInTheDocument();
		await userEvent.type(screen.getByLabelText("Folder name"), "Act two");
		await vi.waitFor(() =>
			expect(onRenameFolder).toHaveBeenCalledWith(2, "Act two"),
		);
		expect(
			screen.queryByRole("button", { name: "Save folder" }),
		).not.toBeInTheDocument();
	});

	it("moves exactly the item ids serialized when a multi-selection drag starts", () => {
		const onMoveItems = vi.fn();
		render(
			<LibraryBrowserView
				catalog={aCatalog()}
				onMoveItems={onMoveItems}
				thumbnailUrl={() => "preview.png"}
			/>,
		);
		const first = screen.getByText("Blue haze").closest("button");
		const second = screen.getByText("Static grid").closest("button");
		if (!first || !second) throw new Error("media cards missing");
		fireEvent.click(first);
		fireEvent.click(second, { metaKey: true });
		const values = new Map<string, string>();
		const transfer = {
			files: [],
			types: ["application/x-tosklight-media-items"],
			effectAllowed: "none",
			setData: (type: string, value: string) => values.set(type, value),
			getData: (type: string) => values.get(type) ?? "",
		};
		fireEvent.dragStart(first, { dataTransfer: transfer });
		fireEvent.drop(screen.getByRole("button", { name: /002Empty folder/u }), {
			dataTransfer: transfer,
		});
		expect(
			draggedItemIds(values.get("application/x-tosklight-media-items") ?? ""),
		).toEqual(["asset-a", "asset-b"]);
		expect(onMoveItems).toHaveBeenCalledTimes(1);
		const moved = onMoveItems.mock.calls[0]?.[0] as Array<{ id: string }>;
		expect(moved.map((item) => item.id)).toEqual(["asset-a", "asset-b"]);
		expect(onMoveItems.mock.calls[0]?.[1]).toBe(2);
	});

	it("reorders complete folders and swaps occupied file slots", () => {
		const onSwapFolders = vi.fn();
		const onReorderItem = vi.fn();
		const { container } = render(
			<LibraryBrowserView
				catalog={aCatalog()}
				onSwapFolders={onSwapFolders}
				onReorderItem={onReorderItem}
				thumbnailUrl={() => "preview.png"}
			/>,
		);
		const values = new Map<string, string>();
		const transfer = {
			files: [],
			types: [],
			effectAllowed: "none",
			setData: (type: string, value: string) => values.set(type, value),
			getData: (type: string) => values.get(type) ?? "",
		};
		const firstFolder = screen.getByRole("button", { name: /001Looks/u });
		fireEvent.dragStart(firstFolder, { dataTransfer: transfer });
		fireEvent.drop(screen.getByRole("button", { name: /900Parking/u }), {
			dataTransfer: transfer,
		});
		expect(onSwapFolders).toHaveBeenCalledWith(1, 900);

		const first = screen.getByText("Blue haze").closest("button");
		const second = screen.getByText("Static grid").closest("button");
		if (!first || !second) throw new Error("media cards missing");
		fireEvent.dragStart(first, { dataTransfer: transfer });
		fireEvent.drop(second, { dataTransfer: transfer });
		expect(onReorderItem).toHaveBeenCalledWith(
			expect.objectContaining({ id: "asset-a" }),
			{ folder: 1, file: 2 },
		);
		onReorderItem.mockClear();
		fireEvent.dragStart(first, { dataTransfer: transfer });
		const empty = container.querySelector<HTMLButtonElement>(
			'.media-library-pool [data-pool-position="2"]',
		);
		if (!empty) throw new Error("empty file slot missing");
		fireEvent.drop(empty, { dataTransfer: transfer });
		expect(onReorderItem).toHaveBeenCalledWith(
			expect.objectContaining({ id: "asset-a" }),
			{ folder: 1, file: 3 },
		);
	});

	it("keeps search-hidden selected items in the exact multi-drag payload", async () => {
		const onMoveItems = vi.fn();
		render(
			<LibraryBrowserView
				catalog={aCatalog()}
				onMoveItems={onMoveItems}
				thumbnailUrl={() => "preview.png"}
			/>,
		);
		const first = screen.getByText("Blue haze").closest("button");
		const second = screen.getByText("Static grid").closest("button");
		if (!first || !second) throw new Error("media cards missing");
		fireEvent.click(first);
		fireEvent.click(second, { metaKey: true });
		await userEvent.type(screen.getByLabelText("Search Library"), "Blue");
		expect(screen.queryByText("Static grid")).not.toBeInTheDocument();
		const values = new Map<string, string>();
		const transfer = {
			files: [],
			types: ["application/x-tosklight-media-items"],
			effectAllowed: "none",
			setData: (type: string, value: string) => values.set(type, value),
			getData: (type: string) => values.get(type) ?? "",
		};
		fireEvent.dragStart(first, { dataTransfer: transfer });
		fireEvent.drop(screen.getByRole("button", { name: /002Empty folder/u }), {
			dataTransfer: transfer,
		});
		const moved = onMoveItems.mock.calls[0]?.[0] as Array<{ id: string }>;
		expect(moved.map((item) => item.id)).toEqual(["asset-a", "asset-b"]);
	});

	it("treats a drop back onto the items' current folder as a no-op", async () => {
		const server = stubServer();
		render(<LibraryPage />);
		const first = (await screen.findByText("Blue haze")).closest("button");
		if (!first) throw new Error("media card missing");
		fireEvent.click(first);
		const values = new Map<string, string>();
		const transfer = {
			files: [],
			types: ["application/x-tosklight-media-items"],
			effectAllowed: "none",
			setData: (type: string, value: string) => values.set(type, value),
			getData: (type: string) => values.get(type) ?? "",
		};
		fireEvent.dragStart(first, { dataTransfer: transfer });
		fireEvent.drop(screen.getByRole("button", { name: /001Looks/u }), {
			dataTransfer: transfer,
		});
		await Promise.resolve();
		expect(server.writes).toEqual([]);
		expect(server.catalog.folders[0].items[0].file).toBe(1);
	});

	it("reloads the catalog after a later item in a batch move is refused", async () => {
		const server = stubServer();
		const serverFetch = globalThis.fetch;
		let catalogReads = 0;
		let itemWrites = 0;
		vi.stubGlobal(
			"fetch",
			vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
				const path = String(input).replace("/api/v2", "");
				if (path === "/catalog" && init?.method !== "POST") catalogReads += 1;
				if (/^\/library\/items\/[^/]+\/update$/u.test(path)) {
					itemWrites += 1;
					if (itemWrites === 2) {
						return new Response(
							JSON.stringify({
								code: "move-refused",
								message: "the second item could not be moved",
							}),
							{
								status: 409,
								headers: { "content-type": "application/json" },
							},
						);
					}
				}
				return serverFetch(input, init);
			}),
		);
		render(<LibraryPage />);
		const first = (await screen.findByText("Blue haze")).closest("button");
		const second = screen.getByText("Static grid").closest("button");
		if (!first || !second) throw new Error("media cards missing");
		fireEvent.click(first);
		fireEvent.click(second, { metaKey: true });
		const values = new Map<string, string>();
		const transfer = {
			files: [],
			types: ["application/x-tosklight-media-items"],
			effectAllowed: "none",
			setData: (type: string, value: string) => values.set(type, value),
			getData: (type: string) => values.get(type) ?? "",
		};
		fireEvent.dragStart(first, { dataTransfer: transfer });
		fireEvent.drop(screen.getByRole("button", { name: /002Empty folder/u }), {
			dataTransfer: transfer,
		});
		expect(
			await screen.findByText(/the second item could not be moved/u),
		).toBeInTheDocument();
		await vi.waitFor(() => expect(catalogReads).toBeGreaterThanOrEqual(2));
		expect(
			server.catalog.folders.find((entry) => entry.folder === 2)?.items,
		).toHaveLength(1);
	});

	it("rejects unsupported and busy external folder drops", () => {
		const onUpload = vi.fn();
		const { rerender } = render(
			<LibraryBrowserView
				catalog={aCatalog()}
				onUpload={onUpload}
				thumbnailUrl={() => "preview.png"}
			/>,
		);
		const folder = screen.getByRole("button", { name: /001Looks/u });
		fireEvent.drop(folder, {
			dataTransfer: {
				files: [new File(["no"], "notes.txt", { type: "text/plain" })],
				types: ["Files"],
			},
		});
		expect(onUpload).not.toHaveBeenCalled();
		expect(
			screen.getByText(/Only video and image files can be uploaded/u),
		).toBeInTheDocument();

		rerender(
			<LibraryBrowserView
				busy
				catalog={aCatalog()}
				onUpload={onUpload}
				thumbnailUrl={() => "preview.png"}
			/>,
		);
		fireEvent.drop(screen.getByRole("button", { name: /001Looks/u }), {
			dataTransfer: {
				files: [new File(["image"], "look.png", { type: "image/png" })],
				types: ["Files"],
			},
		});
		expect(onUpload).not.toHaveBeenCalled();
	});

	it("clears the native picker so the same file can be selected again", () => {
		const onUpload = vi.fn();
		const { container } = render(
			<LibraryBrowserView
				catalog={aCatalog()}
				onUpload={onUpload}
				thumbnailUrl={() => "preview.png"}
			/>,
		);
		const picker = container.querySelector<HTMLInputElement>(
			'input[type="file"][multiple]',
		);
		if (!picker) throw new Error("file picker missing");
		const media = new File(["image"], "look.png", { type: "image/png" });
		fireEvent.change(picker, { target: { files: [media] } });
		expect(picker.value).toBe("");
		expect(onUpload).toHaveBeenCalledWith([media], 1);
	});

	it("allocates at the first free slot and rolls into the next folder", () => {
		const catalog = aCatalog();
		catalog.folders[0].items = Array.from({ length: 254 }, (_, index) => ({
			...catalog.folders[0].items[0],
			id: `item-${index + 1}`,
			file: index + 1,
		}));
		expect(allocateFreeAddresses(catalog, 1, [], 2)).toEqual([
			{ folder: 2, file: 1 },
			{ folder: 2, file: 2 },
		]);
	});

	it("fills sparse holes before rolling over", () => {
		const catalog = aCatalog();
		catalog.folders[0].items[1].file = 4;
		expect(allocateFreeAddresses(catalog, 1, [], 3)).toEqual([
			{ folder: 1, file: 2 },
			{ folder: 1, file: 3 },
			{ folder: 1, file: 5 },
		]);
	});

	it("allocates parking space only inside the 900-series", () => {
		expect(allocateFreeAddresses(aCatalog(), 900, [], 2)).toEqual([
			{ folder: 900, file: 1 },
			{ folder: 900, file: 2 },
		]);
	});

	it("treats moving items as free and stops at the writable address boundary", () => {
		const catalog = aCatalog();
		expect(allocateFreeAddresses(catalog, 1, catalog.folders[0].items)).toEqual(
			[
				{ folder: 1, file: 1 },
				{ folder: 1, file: 2 },
			],
		);
		catalog.folders = [
			{
				folder: 199,
				name: "Last",
				items: Array.from({ length: 254 }, (_, index) => ({
					...aCatalog().folders[0].items[0],
					id: `last-${index}`,
					file: index + 1,
				})),
			},
		];
		expect(allocateFreeAddresses(catalog, 199, [], 1)).toEqual([]);
	});

	it("recognizes only operator-supported external media types and safe drag payloads", () => {
		expect(isAcceptedMediaFile({ type: "video/mp4" })).toBe(true);
		expect(isAcceptedMediaFile({ type: "image/png" })).toBe(true);
		expect(isAcceptedMediaFile({ type: "text/plain" })).toBe(false);
		expect(draggedItemIds('["a","b","a",4]')).toEqual(["a", "b"]);
		expect(draggedItemIds("not json")).toEqual([]);
	});
});
