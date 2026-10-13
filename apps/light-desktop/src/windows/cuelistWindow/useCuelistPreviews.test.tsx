import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Cue } from "../../api/types";
import { CueMediaPreviewContent } from "../../features/cueThumbnails/CueMediaPreviewView";
import { useCuelistPreviews } from "./useCuelistPreviews";

const mocks = vi.hoisted(() => ({
	actions: { available: true, mediaIndex: vi.fn(), mediaImage: vi.fn() },
	media: {
		mediaServers: [{ fixture_id: "server-b", status: { online: true } }],
	},
}));
vi.mock("../../features/cueThumbnails/CueThumbnailActions", () => ({
	useCueThumbnailActions: () => mocks.actions,
}));
vi.mock("../../features/mediaServers/MediaServersContext", () => ({
	useMediaServers: () => mocks.media,
}));
vi.mock("./useCueThumbnails", () => ({ useCueThumbnails: () => ({}) }));
const id = "397b6120-1ff1-4988-8b2f-5cd613e0977f";
const cues: Cue[] = [
	{
		id,
		number: "1",
		name: "Sky",
		fade_millis: 0,
		delay_millis: 0,
		trigger: { type: "manual" },
		changes: [],
	},
];
function Fixture() {
	const previews = useCuelistPreviews(cues, true);
	const [opened, setOpened] = useState<number | null>(null);
	const preview = previews.mediaPreviews[0];
	const picture = previews.pictureAt(opened);
	return (
		<>
			{preview && (
				<CueMediaPreviewContent
					cueNumber="1"
					preview={preview}
					onOpen={() => previews.openPicture(0, setOpened)}
					onRetry={previews.retry}
				/>
			)}
			{picture && <img alt="Opened Cue" src={picture.src} />}
		</>
	);
}
beforeEach(() => {
	mocks.actions.mediaIndex.mockReset().mockResolvedValue([
		{
			cueId: id,
			cueListId: "list-b",
			serverFixtureId: "server-b",
			outputId: "b",
			scope: "layer",
			layer: 0,
			layerFixtureId: "layer-b",
			previewKey: "unchanged-sky",
		},
	]);
	mocks.actions.mediaImage.mockReset().mockResolvedValue({
		kind: "picture",
		blob: new Blob(["Sky"]),
		empty: false,
	});
	let sequence = 0;
	vi.stubGlobal("URL", {
		...URL,
		createObjectURL: vi.fn(() => `blob:sky-${++sequence}`),
		revokeObjectURL: vi.fn(),
	});
});
afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});
it("literal picture tap revalidates unchanged content; offline hides the modal and Retry recovers", async () => {
	render(<Fixture />);
	const button = await screen.findByRole("button", {
		name: "Open Cue 1 Layer 1 preview",
	});
	let rejectOffline!: (value: unknown) => void;
	mocks.actions.mediaImage.mockImplementationOnce(
		() =>
			new Promise((resolve) => {
				rejectOffline = resolve;
			}),
	);
	fireEvent.click(button);
	await waitFor(() =>
		expect(mocks.actions.mediaImage).toHaveBeenCalledTimes(2),
	);
	expect(screen.queryByAltText("Opened Cue")).toBeNull();
	expect(screen.getByRole("status")).toHaveTextContent("Loading media preview");
	await act(async () => {
		rejectOffline({
			kind: "failed",
			state: "offline",
			error: "connection refused",
			retryable: true,
		});
	});
	const retry = await screen.findByRole("button", {
		name: /Media Server offline.*Retry/,
	});
	expect(retry.querySelector("img")).toBeNull();
	expect(screen.queryByAltText("Opened Cue")).toBeNull();
	expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:sky-1");
	fireEvent.click(retry);
	await waitFor(() =>
		expect(mocks.actions.mediaImage).toHaveBeenCalledTimes(3),
	);
	await screen.findByRole("button", { name: "Open Cue 1 Layer 1 preview" });
	expect(screen.getByAltText("Opened Cue")).toHaveAttribute(
		"src",
		"blob:sky-2",
	);
});
