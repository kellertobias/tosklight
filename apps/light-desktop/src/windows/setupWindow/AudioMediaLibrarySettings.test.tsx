import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DeskConfiguration } from "../../api/types";
import { TimecodeActionsProvider } from "../../features/timecode/TimecodeActionsContext";
import { DesktopProvider } from "../../platform/desktop";
import { browserDesktopBridge } from "../../platform/desktop/browserDesktopBridge";
import type { SetupWindowController } from "./controller";
import { ShowsRecoverySection } from "./GeneralSections";

vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useBootstrapSnapshot: () => ({ active_show: null }),
	useSessionSnapshot: () => null,
}));
vi.mock("../../features/shellStatus/ShellStatusState", () => ({
	useConnectionStatus: () => "connected",
}));
vi.mock("../../features/showLifecycle/ShowLifecycleContext", () => ({
	useShowLifecycle: () => ({ shows: [] }),
}));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ dispatch: vi.fn() }) }));
vi.mock("../../components/setup/ShowRecoveryFileManager", () => ({
	ShowRecoveryFileManager: () => null,
}));

afterEach(cleanup);

describe("Audio Player media library settings", () => {
	it("selects and replaces the desk-local default library", async () => {
		const draft = {
			autosave_interval_seconds: 30,
			internal_audio_library_roots: { archive: "/Volumes/Archive" },
		} as unknown as DeskConfiguration;
		const editDraft = vi.fn();
		const selectFolder = vi.fn().mockResolvedValue("/Volumes/Show/Audio");
		const controller = { draft, editDraft } as unknown as SetupWindowController;
		const api = {
			internalAudioStatus: vi.fn().mockResolvedValue({ players: [], libraries: [] }),
		};
		render(
			<DesktopProvider bridge={{ ...browserDesktopBridge, available: true, selectFolder }}>
				<TimecodeActionsProvider api={api as never}>
					<ShowsRecoverySection controller={controller} />
				</TimecodeActionsProvider>
			</DesktopProvider>,
		);

		expect(screen.getByText("Select the media library for the Audio Player.")).toBeVisible();
		expect(screen.getByText(/Not configured/)).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Select media library" }));
		await waitFor(() => expect(selectFolder).toHaveBeenCalledOnce());
		expect(editDraft).toHaveBeenCalledWith({
			...draft,
			internal_audio_library_roots: {
				archive: "/Volumes/Archive",
				default: "/Volumes/Show/Audio",
			},
		});
	});

	it("reports an unavailable configured root and leaves cancellation unchanged", async () => {
		const draft = {
			autosave_interval_seconds: 30,
			internal_audio_library_roots: { default: "/Missing/Audio" },
		} as unknown as DeskConfiguration;
		const editDraft = vi.fn();
		const controller = { draft, editDraft } as unknown as SetupWindowController;
		const api = {
			internalAudioStatus: vi.fn().mockResolvedValue({
				players: [],
				libraries: [
					{
						binding: "default",
						entries: 0,
						diagnostics: [
							"audio library /Missing/Audio is unavailable: not found",
						],
					},
				],
			}),
		};
		render(
			<DesktopProvider
				bridge={{ ...browserDesktopBridge, available: true, selectFolder: async () => null }}
			>
				<TimecodeActionsProvider api={api as never}>
					<ShowsRecoverySection controller={controller} />
				</TimecodeActionsProvider>
			</DesktopProvider>,
		);

		expect(screen.getByText(/\/Missing\/Audio/)).toBeVisible();
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"Library unavailable: audio library /Missing/Audio is unavailable: not found",
		);
		fireEvent.click(screen.getByRole("button", { name: "Select media library" }));
		await waitFor(() => expect(editDraft).not.toHaveBeenCalled());
	});
});
