import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { DeskLoadingStateProvider } from "../../features/deskLoading/DeskLoadingState";
import { DeskLoadingOverlay } from "./DeskLoadingOverlay";

describe("DeskLoadingOverlay", () => {
	it("names the show operation and exposes it as a busy status", () => {
		render(
			<DeskLoadingStateProvider
				loading={{
					operationId: 4,
					title: "Loading show Festival…",
					detail: "Installing the show engine snapshot",
				}}
			>
				<DeskLoadingOverlay />
			</DeskLoadingStateProvider>,
		);

		const status = screen.getByRole("status");
		expect(status).toHaveAttribute("aria-busy", "true");
		expect(status).toHaveTextContent("Loading show Festival…");
		expect(status).toHaveTextContent(
			"Ready desk controls remain available while show capabilities reconnect.",
		);
	});
});
