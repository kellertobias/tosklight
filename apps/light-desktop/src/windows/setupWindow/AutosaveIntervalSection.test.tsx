import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DeskConfiguration } from "../../api/types";
import type { SetupWindowController } from "./controller";
import { ShowsRecoverySection } from "./GeneralSections";
import { OutputsSection } from "./OutputsSection";

vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useBootstrapSnapshot: () => ({
		active_show: { name: "Tour", updated_at: "Now" },
	}),
	useSessionSnapshot: () => null,
}));

vi.mock("../../features/dmxDiagnostics/DmxDiagnosticsContext", () => ({
	useDmxDiagnostics: () => null,
}));

vi.mock("../../features/shellStatus/ShellStatusState", () => ({
	useConnectionStatus: () => "connected",
}));

vi.mock("../../features/showLifecycle/ShowLifecycleContext", () => ({
	useShowLifecycle: () => ({ shows: [{ id: "tour" }, { id: "festival" }] }),
}));

const dispatch = vi.fn();
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ dispatch }),
}));

vi.mock("../../components/setup/OutputRoutesSetup", () => ({
	OutputRoutesSetup: () => <div>Output routes</div>,
}));

vi.mock("../../components/setup/ShowRecoveryFileManager", () => ({
	ShowRecoveryFileManager: () => <div>Show recovery file manager</div>,
}));

afterEach(cleanup);

describe("autosave interval setup placement", () => {
	it("edits the interval under Shows & recovery and omits it from Outputs", () => {
		const draft = {
			autosave_interval_seconds: 30,
			frame_rate_hz: 44,
			output_bind_ip: "0.0.0.0",
			backup_retention: 20,
		} as DeskConfiguration;
		const editDraft = vi.fn();
		const controller = {
			draft,
			editDraft,
		} as unknown as SetupWindowController;

		render(<ShowsRecoverySection controller={controller} />);
		expect(screen.getByText("Current show")).toBeVisible();
		expect(screen.getByText("Tour")).toBeVisible();
		expect(screen.getByText("2 library shows")).toBeVisible();
		expect(screen.getByText("Connected, autosave active")).toHaveClass(
			"is-connected",
		);
		fireEvent.click(screen.getByRole("button", { name: "Open show" }));
		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "file_manager",
		});
		const interval = screen.getByLabelText("Autosave interval");
		expect(interval).toHaveValue("30");
		fireEvent.change(interval, { target: { value: "45" } });
		expect(editDraft).toHaveBeenCalledWith({
			...draft,
			autosave_interval_seconds: 45,
		});

		cleanup();
		render(<OutputsSection controller={controller} />);
		expect(
			screen.queryByLabelText("Autosave interval"),
		).not.toBeInTheDocument();
		const frameRate = screen.getByLabelText("Frame rate");
		expect(frameRate).toBeVisible();
		expect(screen.getByText("40–60 Hz")).toBeVisible();
	});
});
