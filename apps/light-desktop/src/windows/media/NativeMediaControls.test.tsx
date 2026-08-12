import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { NativeMediaControls } from "./NativeMediaControls";

describe("Native Media controls", () => {
	it("shows native status and saves static text through the fixture-scoped action", async () => {
		const load = vi.fn().mockResolvedValue({
			endpoint: "http://10.0.0.8:8080",
			status: "ok",
			instance: "media-a",
			outputs: 2,
			catalogRevision: 7,
			catalogItems: 42,
			textSlots: [
				{
					folder: 200,
					file: 3,
					name: "Interval",
					enabled: true,
					kind: "static",
					text: "House open",
				},
			],
			effectControlsAvailable: false,
		});
		const updateText = vi.fn().mockResolvedValue({
			folder: 200,
			file: 3,
			name: "Interval",
			enabled: true,
			kind: "static",
			text: "Stand by",
		});

		render(
			<NativeMediaControls
				fixtureId="fixture-1"
				load={load}
				updateText={updateText}
			/>,
		);

		expect(await screen.findByText("Media Server online")).toBeInTheDocument();
		expect(screen.getByText("http://10.0.0.8:8080")).toBeInTheDocument();
		expect(screen.getByText("Effect controls unavailable")).toBeInTheDocument();
		fireEvent.change(screen.getByLabelText("200.3 · Interval"), {
			target: { value: "Stand by" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Save text" }));
		await waitFor(() =>
			expect(updateText).toHaveBeenCalledWith(
				"fixture-1",
				200,
				3,
				"Stand by",
			),
		);
	});

	it("keeps native failures actionable", async () => {
		render(
			<NativeMediaControls
				fixtureId="fixture-1"
				load={vi.fn().mockRejectedValue(new Error("Media Server is offline"))}
				updateText={vi.fn()}
			/>,
		);
		expect(await screen.findByText("Native controls unavailable")).toBeInTheDocument();
		expect(screen.getByText("Media Server is offline")).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
	});
});
