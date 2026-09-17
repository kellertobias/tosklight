import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MediaServerCacheSettings } from "./MediaServerCacheSettings";

const media = vi.hoisted(() => ({
	server: null as null | { clearMediaThumbnailCache: () => Promise<number> },
	clear: vi.fn<() => Promise<number>>(),
}));

vi.mock("../../features/mediaServers/MediaServersContext", () => ({
	useMediaServers: () => media.server,
}));

beforeEach(() => {
	media.clear.mockReset();
	media.server = { clearMediaThumbnailCache: media.clear };
});

afterEach(cleanup);

describe("Show Patch Settings › Clear Thumbnail Cache", () => {
	it("clears the desk cache and says how many thumbnails went", async () => {
		media.clear.mockResolvedValue(12);
		render(<MediaServerCacheSettings />);
		fireEvent.click(
			screen.getByRole("button", { name: "Clear Thumbnail Cache" }),
		);

		expect(await screen.findByRole("status")).toHaveTextContent(
			"Cleared 12 cached thumbnails. Refresh Thumbnails on a server row to fetch them again.",
		);
		expect(media.clear).toHaveBeenCalledOnce();
		expect(
			screen.getByRole("button", { name: "Clear Thumbnail Cache" }),
		).toBeEnabled();
	});

	it("turns a refusal into an action the operator can take", async () => {
		media.clear.mockRejectedValue(new Error("The desk is not reachable."));
		render(<MediaServerCacheSettings />);
		fireEvent.click(
			screen.getByRole("button", { name: "Clear Thumbnail Cache" }),
		);

		expect(await screen.findByRole("alert")).toHaveTextContent(
			"The desk is not reachable. Check the desk connection, then Clear Thumbnail Cache again.",
		);
	});

	it("explains a missing desk connection instead of doing nothing", () => {
		media.server = null;
		render(<MediaServerCacheSettings />);
		fireEvent.click(
			screen.getByRole("button", { name: "Clear Thumbnail Cache" }),
		);

		expect(screen.getByRole("alert")).toHaveTextContent(
			"The desk connection is unavailable.",
		);
	});
});
