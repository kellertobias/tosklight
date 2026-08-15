// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { appReducer, initialState } from "../../state/appReducer";
import { PatchWindow } from "../PatchWindow";
import { OPEN_MEDIA_PATCH_ACTION } from "./openMediaPatch";

vi.mock("../../features/patch/PatchFeatureBoundary", () => ({
	PatchFeatureBoundary: ({ children }: { children: ReactNode }) => children,
}));

vi.mock("../../components/setup/MediaServerSetup", () => ({
	MediaServerSetup: () => <div>Media Server patch</div>,
}));

vi.mock("../../components/setup/FixturePatchSetup", () => ({
	FixturePatchSetupContent: () => <div>Fixture patch</div>,
}));

vi.mock("../../platform/desktop", () => ({
	useDesktopBridge: () => ({ available: false }),
}));

describe("Media pane Open Patch navigation", () => {
	it("opens Show Patch directly in the Media Servers view", () => {
		const state = appReducer(initialState, OPEN_MEDIA_PATCH_ACTION);

		expect(state).toMatchObject({
			builtIn: "patch",
			dockMode: "builtins",
			patchBuiltInView: "media",
		});

		render(<PatchWindow patchView={state.patchBuiltInView} />);
		expect(screen.getByText("Media Server patch")).toBeInTheDocument();
		expect(screen.queryByText("Fixture patch")).toBeNull();
		expect(
			screen.getByRole("tab", { name: "Media Servers", selected: true }),
		).toBeInTheDocument();
	});
});
