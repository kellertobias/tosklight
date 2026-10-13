import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type {
	CueMediaPreviewApiClient,
	CueMediaPreviewEntry,
	CueMediaPreviewImage,
} from "../../api/client/cueMediaPreviews";
import type { CueThumbnailApiClient } from "../../api/client/cueThumbnails";
import type { Cue, CueList, PlaybackDefinition } from "../../api/types";
import { PlaybackArtworkProvider } from "../../components/control/playbackFaderBank/PlaybackArtworkProvider";
import { usePlaybackArtwork } from "../../components/control/playbackFaderBank/usePlaybackArtwork";
import { useCuelistPreviews } from "../../windows/cuelistWindow/useCuelistPreviews";
import { CueMediaPreviewContent } from "./CueMediaPreviewView";
import { CueThumbnailActionsProvider } from "./CueThumbnailActions";

const media = vi.hoisted(() => ({
	mediaServers: [
		{
			fixture_id: "server-a",
			status: { online: true },
			endpoint: { http_port: 18080 },
			output_binding: "a",
		},
		{
			fixture_id: "server-b",
			status: { online: true },
			endpoint: { http_port: 18081 },
			output_binding: "b",
		},
	],
}));
vi.mock("../mediaServers/MediaServersContext", () => ({
	useMediaServers: () => media,
}));
vi.mock("../../windows/cuelistWindow/useCueThumbnails", () => ({
	useCueThumbnails: () => ({}),
}));
const image = vi.fn();
const entries: CueMediaPreviewEntry[] = ["a", "b", "peer"].map((id) => ({
	cueId: id,
	cueListId: id,
	serverFixtureId: id === "a" ? "server-a" : "server-b",
	outputId: id,
	scope: "layer",
	layer: 0,
	layerFixtureId: id,
	previewKey: "same-key",
}));
const cues = entries.map((entry) => ({
	id: entry.cueId,
	number: "1",
	name: "Sky",
	changes: [],
	trigger: { type: "manual" },
	fade_millis: 0,
	delay_millis: 0,
})) as Cue[];
const lists = cues.map((cue) => ({ cues: [cue] }) as CueList);
const assignments = cues.map(
	(cue) =>
		({
			target: { type: "cue_list", cue_list_id: cue.id },
		}) as PlaybackDefinition,
);
const mediaClient = {
	index: vi.fn(async () => entries),
	image,
} as unknown as CueMediaPreviewApiClient;
const client = {} as CueThumbnailApiClient;
function Tile({ index }: { index: number }) {
	const artwork = usePlaybackArtwork(assignments[index], lists[index]);
	return (
		<div data-testid={`tile-${cues[index].id}`}>
			{artwork?.image && (
				<img alt={`Tile ${cues[index].id}`} src={artwork.image} />
			)}
			<span>{artwork?.notice}</span>
		</div>
	);
}
function Row() {
	const previews = useCuelistPreviews([cues[1]], true);
	const [, setOpened] = useState<number | null>(null);
	const preview = previews.mediaPreviews[0];
	return (
		<div data-testid="row">
			{preview && (
				<CueMediaPreviewContent
					cueNumber="1"
					preview={preview}
					onOpen={() => previews.openPicture(0, setOpened)}
					onRetry={() => previews.retry(cues[1].id)}
				/>
			)}
		</div>
	);
}
function Fixture({ row = true }: { row?: boolean }) {
	return (
		<CueThumbnailActionsProvider
			client={client}
			mediaClient={mediaClient}
			showId="show"
			canWrite
		>
			{row && <Row />}
			<PlaybackArtworkProvider
				candidates={cues.map((_, index) => ({
					playback: assignments[index],
					cue: lists[index],
				}))}
			>
				{cues.map((cue, index) => (
					<Tile key={cue.id} index={index} />
				))}
			</PlaybackArtworkProvider>
		</CueThumbnailActionsProvider>
	);
}
const picture = (): CueMediaPreviewImage => ({
	kind: "picture",
	blob: new Blob(["Sky"]),
	empty: false,
});
beforeEach(() => {
	media.mediaServers = media.mediaServers.map((server) => ({
		...server,
		endpoint: { http_port: server.fixture_id === "server-a" ? 18080 : 18081 },
	}));
	image.mockReset().mockImplementation(async () => picture());
	let count = 0;
	vi.stubGlobal("URL", {
		...URL,
		createObjectURL: vi.fn(() => `blob:${++count}`),
		revokeObjectURL: vi.fn(),
	});
});
afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});
it.each(["missing", "offline"] as const)(
	"actual row %s response invalidates affected Playback pictures and same-key retry/reopen recovers",
	async (state) => {
		const view = render(<Fixture />);
		await screen.findByAltText("Tile b");
		await screen.findByAltText("Tile a");
		await screen.findByAltText("Tile peer");
		const a = screen.getByAltText("Tile a").getAttribute("src");
		const b = screen.getByAltText("Tile b").getAttribute("src");
		const peer = screen.getByAltText("Tile peer").getAttribute("src");
		const aCalls = image.mock.calls.filter((call) => call[1] === "a").length;
		image.mockResolvedValueOnce({
			kind: "failed",
			state,
			error: state,
			retryable: state === "offline",
		});
		fireEvent.click(
			within(screen.getByTestId("row")).getByRole("button", {
				name: /Open Cue/,
			}),
		);
		const notice =
			state === "missing" ? "Media output missing" : "Media Server offline";
		await waitFor(() =>
			expect(screen.getByTestId("tile-b")).toHaveTextContent(notice),
		);
		expect(within(screen.getByTestId("tile-b")).queryByRole("img")).toBeNull();
		expect(URL.revokeObjectURL).toHaveBeenCalledWith(b);
		expect(screen.getByAltText("Tile a")).toHaveAttribute("src", a);
		if (state === "missing")
			expect(screen.getByAltText("Tile peer")).toHaveAttribute("src", peer);
		else expect(screen.getByTestId("tile-peer")).toHaveTextContent(notice);
		if (state === "offline")
			fireEvent.click(
				within(screen.getByTestId("row")).getByRole("button", {
					name: /Retry/,
				}),
			);
		else {
			view.rerender(<Fixture row={false} />);
			view.rerender(<Fixture />);
		}
		await waitFor(() =>
			expect(screen.getByAltText("Tile b")).not.toHaveAttribute("src", b),
		);
		expect(screen.getByTestId("tile-b")).not.toHaveTextContent(notice);
		expect(image.mock.calls.filter((call) => call[1] === "a")).toHaveLength(
			aCalls,
		);
	},
);
it("a failure observed by another consumer retires an older in-flight success and all owned blobs", async () => {
	let resolveOld: ((value: CueMediaPreviewImage) => void) | undefined;
	let bCalls = 0;
	image.mockImplementation(async (_show: string, id: string) => {
		if (id === "b" && ++bCalls === 2)
			return new Promise<CueMediaPreviewImage>((resolve) => {
				resolveOld = resolve;
			});
		return picture();
	});
	const view = render(<Fixture />);
	await within(screen.getByTestId("row")).findByRole("button", {
		name: /Open Cue/,
	});
	await waitFor(() => expect(resolveOld).toBeDefined());
	image.mockResolvedValueOnce({
		kind: "failed",
		state: "missing",
		error: "missing",
		retryable: false,
	});
	fireEvent.click(
		within(screen.getByTestId("row")).getByRole("button", { name: /Open Cue/ }),
	);
	await waitFor(() =>
		expect(screen.getByTestId("tile-b")).toHaveTextContent(
			"Media output missing",
		),
	);
	const created = vi.mocked(URL.createObjectURL).mock.calls.length;
	await act(async () => resolveOld?.(picture()));
	expect(URL.createObjectURL).toHaveBeenCalledTimes(created);
	expect(within(screen.getByTestId("tile-b")).queryByRole("img")).toBeNull();
	view.unmount();
	expect(URL.revokeObjectURL).toHaveBeenCalledTimes(created);
});

it("same-key endpoint changes refresh only affected pictures and cancel old-connection completion", async () => {
	const view = render(<Fixture />);
	await screen.findByAltText("Tile b");
	await screen.findByAltText("Tile a");
	const a = screen.getByAltText("Tile a").getAttribute("src");
	const b = screen.getByAltText("Tile b").getAttribute("src");
	let old: ((value: CueMediaPreviewImage) => void) | undefined;
	image.mockImplementationOnce(
		() =>
			new Promise<CueMediaPreviewImage>((resolve) => {
				old = resolve;
			}),
	);
	fireEvent.click(
		within(screen.getByTestId("row")).getByRole("button", { name: /Open Cue/ }),
	);
	await waitFor(() => expect(old).toBeDefined());
	media.mediaServers = media.mediaServers.map((server) =>
		server.fixture_id === "server-b"
			? { ...server, endpoint: { http_port: 18082 } }
			: server,
	);
	view.rerender(<Fixture />);
	await waitFor(() =>
		expect(screen.getByAltText("Tile b")).not.toHaveAttribute("src", b),
	);
	const created = vi.mocked(URL.createObjectURL).mock.calls.length;
	await act(async () =>
		old?.({
			kind: "failed",
			state: "missing",
			error: "old endpoint",
			retryable: false,
		}),
	);
	expect(URL.createObjectURL).toHaveBeenCalledTimes(created);
	expect(screen.getByAltText("Tile a")).toHaveAttribute("src", a);
	expect(screen.getByTestId("tile-b")).not.toHaveTextContent(
		"Media output missing",
	);
	expect(URL.revokeObjectURL).toHaveBeenCalledWith(b);
});
