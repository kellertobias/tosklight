import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { MediaPaneSurface } from "./MediaPaneSurface";
import type { MediaPaneModel } from "./mediaPaneModel";

describe("MediaPaneSurface control state", () => {
	it("does not invoke a disabled choice control", async () => {
		vi.stubGlobal(
			"ResizeObserver",
			class {
				observe() {}
				disconnect() {}
			},
		);
		const onChangeControl = vi.fn();
		const model: MediaPaneModel = {
			servers: [{ id: "server", name: "Server", statusLabel: "Online" }],
			selectedServerId: "server",
			selectedLayerId: null,
			preview: { kind: "unsupported", capability: "preview", detail: "None" },
			layers: [],
			browserMode: "media",
			maskBrowser: "hidden",
			libraryFolders: [],
			libraryFiles: [],
			draftFolderId: "1",
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
			controlSections: [
				{
					id: "playback",
					label: "Playback",
					controls: [
						{
							id: "mode",
							kind: "choice",
							label: "Play mode",
							value: "loop",
							options: [
								{ value: "loop", label: "Loop" },
								{ value: "pause", label: "Pause" },
							],
							disabled: true,
						},
					],
				},
			],
			selectedControlSectionId: "playback",
			mainSectionId: "playback",
			rightPaneVisible: false,
		};
		render(
			<MediaPaneSurface
				model={model}
				onSelectServer={vi.fn()}
				onSelectLayer={vi.fn()}
				onSelectBrowserMode={vi.fn()}
				onBrowseItem={vi.fn()}
				onSelectControlSection={vi.fn()}
				onChangeControl={onChangeControl}
				onSetRightPaneVisible={vi.fn()}
			/>,
		);
		const pause = screen.getByRole("radio", { name: "Pause" });
		expect(pause).toBeDisabled();
		await userEvent.click(pause, { pointerEventsCheck: 0 });
		expect(onChangeControl).not.toHaveBeenCalled();
	});
});
