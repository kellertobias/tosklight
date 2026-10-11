import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, it, expect, vi } from "vitest";
import type {
	PreloadScopeTransport,
	PreloadScopeObserver,
} from "../../features/playbackPreloadScope/contracts";
import { PreloadScopeProvider } from "../../features/playbackPreloadScope/PreloadScopeContext";
import { PlaybackPreloadScopeSettings } from "./PlaybackPreloadScopeSettings";
const SHOW = "11111111-1111-4111-8111-111111111111",
	OTHER = "22222222-2222-4222-8222-222222222222";
const scope = { position_ranges: [{ start: 1, end: 1000 }], pages: null };
function setup() {
	let observer!: PreloadScopeObserver;
	const close = vi.fn();
	const transport: PreloadScopeTransport = {
		loadSnapshot: vi.fn(async (authority) => ({
			show_id: authority.showId,
			revision: 4,
			scope,
		})),
		save: vi.fn(async (authority, revision, value, id) => ({
			show_id: authority.showId,
			revision: revision + 1,
			scope: value,
			request_id: id,
			changed: true,
			replayed: false,
		})),
		subscribe: vi.fn((_, o) => {
			observer = o;
			return { close };
		}),
	};
	const tree = (showId = SHOW) => (
		<PreloadScopeProvider
			authority={{ authorityId: "session", scope: { showId } }}
			transport={transport}
		>
			<PlaybackPreloadScopeSettings />
		</PreloadScopeProvider>
	);
	const view = render(tree());
	return { transport, view, tree, close, getObserver: () => observer };
}
afterEach(cleanup);
describe("Preload eligibility operator settings", () => {
	it("loads all-page default and saves union position ranges against exact revision", async () => {
		const model = setup();
		await waitFor(() =>
			expect(screen.getByLabelText("Playback position ranges")).toBeEnabled(),
		);
		expect(screen.getByLabelText("All pages")).toBeChecked();
		fireEvent.change(screen.getByLabelText("Playback position ranges"), {
			target: { value: "1-15,10-20,401-680" },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "Save playback eligibility" }),
		);
		await waitFor(() =>
			expect(model.transport.save).toHaveBeenCalledWith(
				{ showId: SHOW },
				4,
				{
					position_ranges: [
						{ start: 1, end: 20 },
						{ start: 401, end: 680 },
					],
					pages: null,
				},
				expect.any(String),
				expect.any(AbortSignal),
			),
		);
	});
	it("requires valid explicit optional pages and keeps operator input on rejection", async () => {
		const model = setup();
		await waitFor(() =>
			expect(screen.getByLabelText("Playback position ranges")).toBeEnabled(),
		);
		fireEvent.click(screen.getByLabelText("All pages"));
		fireEvent.change(screen.getByLabelText("Eligible pages"), {
			target: { value: "100" },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "Save playback eligibility" }),
		);
		expect(await screen.findByRole("alert")).toHaveTextContent("99");
		expect(model.transport.save).not.toHaveBeenCalled();
		expect(screen.getByLabelText("Eligible pages")).toHaveValue("100");
	});
	it("repairs subscribed changes without overwriting an unsaved range", async () => {
		const model = setup();
		await waitFor(() =>
			expect(screen.getByLabelText("Playback position ranges")).toBeEnabled(),
		);
		fireEvent.change(screen.getByLabelText("Playback position ranges"), {
			target: { value: "301-305" },
		});
		vi.mocked(model.transport.loadSnapshot).mockResolvedValueOnce({
			show_id: SHOW,
			revision: 5,
			scope: { position_ranges: [{ start: 401, end: 680 }], pages: [2] },
		});
		act(() => model.getObserver().changed({ showId: SHOW, revision: 5 }));
		await waitFor(() =>
			expect(model.transport.loadSnapshot).toHaveBeenCalledTimes(2),
		);
		expect(screen.getByLabelText("Playback position ranges")).toHaveValue(
			"301-305",
		);
	});
	it("drops prior show writes and closes event lease on Show replacement", async () => {
		const model = setup();
		await waitFor(() =>
			expect(screen.getByLabelText("Playback position ranges")).toBeEnabled(),
		);
		fireEvent.change(screen.getByLabelText("Playback position ranges"), {
			target: { value: "301-305" },
		});
		model.view.rerender(model.tree(OTHER));
		await waitFor(() =>
			expect(screen.getByLabelText("Playback position ranges")).toHaveValue(
				"1-1000",
			),
		);
		expect(model.close).toHaveBeenCalledOnce();
		expect(
			screen.getByRole("button", { name: "Save playback eligibility" }),
		).toBeDisabled();
	});
});
