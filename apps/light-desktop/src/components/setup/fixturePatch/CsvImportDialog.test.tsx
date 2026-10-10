import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FixtureDefinition } from "../../../api/types";
import { CsvImportDialog } from "./CsvImportDialog";
import type { PatchController } from "./controller";

const context = vi.hoisted(() => ({
	controller: null as PatchController | null,
}));
vi.mock("./controller", () => ({
	usePatchController: () => context.controller,
}));
afterEach(cleanup);
const definition: FixtureDefinition = {
	schema_version: 2,
	id: "dimmer-mode",
	revision: 1,
	manufacturer: "Generic",
	device_type: "spot",
	name: "Dimmer",
	model: "Dimmer",
	mode: "8 bit",
	footprint: 1,
	heads: [],
	color_calibration: null,
	physical: {},
	model_asset: null,
	icon_asset: null,
	hazardous: false,
	direct_control_protocols: [],
	signal_loss_policy: { type: "hold_last" },
	safe_values: {},
	profile_id: "dimmer",
	mode_id: "dimmer-mode",
	profile_snapshot: null,
};
function setup(request: ReturnType<typeof vi.fn>) {
	const close = vi.fn();
	const status = vi.fn();
	function Workflow() {
		const [open, setOpen] = useState(true);
		context.controller = {
			data: {
				availableDefinitions: [definition],
				all: [],
				layers: [{ id: "default", name: "Default" }],
			},
			ui: {
				csvImportOpen: open,
				activeLayer: "all",
				setCsvImportOpen: (value: boolean) => {
					close(value);
					setOpen(value);
				},
				setStatus: status,
			},
			patch: { patchFixtures: request },
		} as unknown as PatchController;
		return <CsvImportDialog />;
	}
	render(<Workflow />);
	return { close, status };
}
async function review() {
	fireEvent.change(screen.getByLabelText("CSV file"), {
		target: {
			files: [
				new File(
					[
						"Fixture ID,Fixture Name,Manufacturer,Fixture Type,Mode,Patch\n901,CSV acceptance,Generic,Dimmer,8 bit,50.201",
					],
					"acceptance.csv",
				),
			],
		},
	});
	await screen.findByRole("list", { name: "Column assignments" });
	fireEvent.click(screen.getByRole("button", { name: "Next: fixture types" }));
	await screen.findByRole("dialog", { name: "Import CSV · Review" });
}
describe("CSV atomic import lifecycle", () => {
	it("blocks title close, Escape, backdrop and duplicate submissions until authoritative success", async () => {
		let finish: (value: unknown[]) => void = () => {};
		const request = vi.fn(
			() =>
				new Promise((resolve) => {
					finish = resolve;
				}),
		);
		const { close, status } = setup(request);
		await review();
		const submit = screen.getByRole("button", { name: "Import 1 fixture" });
		fireEvent.click(submit);
		fireEvent.click(submit);
		expect(request).toHaveBeenCalledOnce();
		const progress = screen.getByRole("status", { name: "Applying CSV patch" });
		expect(progress).toHaveFocus();
		expect(
			screen.getByRole("button", { name: "Close Import CSV", hidden: true }),
		).toBeDisabled();
		fireEvent.keyDown(progress, { key: "Escape" });
		const backdrop = progress.closest(".stacked-modal-layer");
		if (!backdrop) throw new Error("CSV backdrop missing");
		fireEvent.pointerDown(backdrop);
		expect(
			screen.queryByRole("dialog", { name: "Close Import CSV?" }),
		).toBeNull();
		expect(close).not.toHaveBeenCalled();
		await act(async () => {
			finish([{ uuid: "new-fixture" }]);
		});
		expect(close).toHaveBeenCalledExactlyOnceWith(false);
		expect(status).toHaveBeenCalledWith(
			"Imported 1 fixture from acceptance.csv.",
		);
		expect(screen.queryByRole("dialog", { name: /Import CSV/ })).toBeNull();
	});
	it("retains the review after failure and retries the same atomic plan without duplicate apply", async () => {
		let finish: (value: unknown[]) => void = () => {};
		const request = vi
			.fn()
			.mockRejectedValueOnce(
				new Error("Patch revision changed. Review and retry."),
			)
			.mockImplementationOnce(
				() =>
					new Promise((resolve) => {
						finish = resolve;
					}),
			);
		const { close } = setup(request);
		await review();
		fireEvent.click(screen.getByRole("button", { name: "Import 1 fixture" }));
		await screen.findByRole("alert");
		expect(screen.getByRole("alert")).toHaveTextContent(
			"Patch revision changed. Review and retry.",
		);
		expect(
			screen.queryByRole("status", { name: "Applying CSV patch" }),
		).toBeNull();
		expect(close).not.toHaveBeenCalled();
		expect(
			screen.getByRole("table", { name: "Fixtures to import" }),
		).toHaveTextContent("CSV acceptance");
		const retry = screen.getByRole("button", { name: "Retry import" });
		fireEvent.click(retry);
		fireEvent.click(retry);
		await waitFor(() => expect(request).toHaveBeenCalledTimes(2));
		const first = request.mock.calls[0][0][0];
		const retryCandidate = request.mock.calls[1][0][0];
		expect(retryCandidate.input.fixtureId).toBe(
			retryCandidate.fixture.fixture_id,
		);
		expect({
			...retryCandidate,
			fixture: {
				...retryCandidate.fixture,
				fixture_id: first.fixture.fixture_id,
			},
			input: { ...retryCandidate.input, fixtureId: first.input.fixtureId },
		}).toEqual(first); // Existing candidate construction allocates a new identity on retry.
		expect(screen.queryByRole("alert")).toBeNull();
		await act(async () => {
			finish([{ uuid: "new-fixture" }]);
		});
		expect(close).toHaveBeenCalledExactlyOnceWith(false);
	});
	it("keeps pre-apply cancellation and Stay without mutating the patch", async () => {
		const request = vi.fn();
		const { close } = setup(request);
		await review();
		fireEvent.click(screen.getByRole("button", { name: "Close Import CSV" }));
		fireEvent.click(screen.getByRole("button", { name: "Stay in Import CSV" }));
		expect(
			screen.getByRole("dialog", { name: "Import CSV · Review" }),
		).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Close Import CSV" }));
		fireEvent.click(screen.getByRole("button", { name: "Yes, close" }));
		expect(request).not.toHaveBeenCalled();
		expect(close).toHaveBeenCalledExactlyOnceWith(false);
	});
});
