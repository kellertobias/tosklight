// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { MediaPaneSurface } from "./MediaPaneSurface";
import type { MediaPaneModel } from "./mediaPaneModel";

const model = {
	hasPatchedServer: false,
	hasCitpEndpoint: false,
	servers: [
		{
			id: "",
			name: "No media server is patched",
			statusLabel: "Missing patch",
			disabled: true,
		},
	],
	selectedServerId: "",
	selectedLayerId: null,
	preview: { kind: "unsupported", capability: "preview", detail: "None" },
	layers: [],
	browserMode: "media",
	showSourceFilters: false,
	maskBrowser: "hidden",
	libraryFolders: [],
	libraryFiles: [],
	draftFolderId: "",
	draftFileId: null,
	liveSelection: {
		folderId: null,
		fileId: null,
		maskFolderId: null,
		maskFileId: null,
	},
	draftSelection: {
		folderId: null,
		fileId: null,
		maskFolderId: null,
		maskFileId: null,
	},
	liveSelectionLabel: "None",
	draftSelectionLabel: "None",
	controlSections: [],
	selectedControlSectionId: "content",
	mainSectionId: "content",
	rightPaneVisible: false,
} as MediaPaneModel;

beforeAll(() => {
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
});

describe("Media pane empty patch action", () => {
	it("shows a window-style Open Patch action only in the empty state", () => {
		const onOpenPatch = vi.fn();
		const { rerender } = render(
			<MediaPaneSurface
				model={model}
				onOpenPatch={onOpenPatch}
				onSelectServer={vi.fn()}
				onSelectLayer={vi.fn()}
				onSelectBrowserMode={vi.fn()}
				onBrowseItem={vi.fn()}
				onSelectControlSection={vi.fn()}
				onChangeControl={vi.fn()}
				onSetRightPaneVisible={vi.fn()}
			/>,
		);

		const openPatch = screen.getByRole("button", { name: "Open Patch" });
		expect(openPatch.parentElement).toHaveClass("ui-window-action-group");
		fireEvent.click(openPatch);
		expect(onOpenPatch).toHaveBeenCalledOnce();

		rerender(
			<MediaPaneSurface
				model={{
					...model,
					servers: [{ id: "server-1", name: "Server", statusLabel: "Online" }],
					selectedServerId: "server-1",
				}}
				onOpenPatch={onOpenPatch}
				onSelectServer={vi.fn()}
				onSelectLayer={vi.fn()}
				onSelectBrowserMode={vi.fn()}
				onBrowseItem={vi.fn()}
				onSelectControlSection={vi.fn()}
				onChangeControl={vi.fn()}
				onSetRightPaneVisible={vi.fn()}
			/>,
		);
		expect(screen.queryByRole("button", { name: "Open Patch" })).toBeNull();
		expect(screen.getByLabelText("Media layers")).toBeInTheDocument();
	});
});
