import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiFailure } from "./client";
import { resetResources, useResource, writeResource } from "./resource";

afterEach(() => {
	resetResources();
	vi.useRealTimers();
});

function Panel({ loader, pollMs }: { loader: () => Promise<string>; pollMs?: number }) {
	const resource = useResource("subject", loader, { pollMs });
	return (
		<div>
			<span data-testid="data">{resource.data ?? "—"}</span>
			<span data-testid="failure">{resource.failure?.code ?? "—"}</span>
			<span data-testid="stale">{String(resource.stale)}</span>
		</div>
	);
}

describe("the resource cache", () => {
	it("loads once and publishes the value", async () => {
		render(<Panel loader={async () => "loaded"} />);
		await waitFor(() => expect(screen.getByTestId("data")).toHaveTextContent("loaded"));
	});

	it("keeps the last good value on screen when a later read fails", async () => {
		let answer: () => Promise<string> = async () => "first";
		render(<Panel loader={() => answer()} pollMs={20} />);
		await waitFor(() => expect(screen.getByTestId("data")).toHaveTextContent("first"));

		answer = async () => {
			throw new ApiFailure("unreachable", "gone", 0);
		};

		await waitFor(() => expect(screen.getByTestId("failure")).toHaveTextContent("unreachable"));
		// A panel that blanks out during a hiccup is worse than one that says it is stale.
		expect(screen.getByTestId("data")).toHaveTextContent("first");
		expect(screen.getByTestId("stale")).toHaveTextContent("true");
	});

	it("takes an optimistic write without a round trip", async () => {
		render(<Panel loader={async () => "server"} />);
		await waitFor(() => expect(screen.getByTestId("data")).toHaveTextContent("server"));

		writeResource("subject", "optimistic");
		await waitFor(() => expect(screen.getByTestId("data")).toHaveTextContent("optimistic"));
	});

	it("turns an unexpected throw into a typed failure rather than losing it", async () => {
		render(
			<Panel
				loader={async () => {
					throw new Error("boom");
				}}
			/>,
		);
		await waitFor(() =>
			expect(screen.getByTestId("failure")).toHaveTextContent("unexpected-error"),
		);
	});
});
