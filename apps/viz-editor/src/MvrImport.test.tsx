import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { MvrImport } from "./MvrImport";
import type { MvrPreview } from "./document/session";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const CLEAN = {
	uuid: "11111111-1111-4111-8111-111111111111",
	name: "Wash 1",
	gdtfSpec: "Acme Wash",
	gdtfMode: "Default",
	universe: 1,
	address: 1,
	matched: true,
	conflicted: false,
};

const UNMATCHED = {
	uuid: "22222222-2222-4222-8222-222222222222",
	name: "Mystery 2",
	gdtfSpec: "Unknown Thing",
	gdtfMode: "Mode 1",
	universe: 1,
	address: 20,
	matched: false,
	conflicted: false,
};

const CONFLICTED = {
	uuid: "33333333-3333-4333-8333-333333333333",
	name: "Spot 3",
	gdtfSpec: "Acme Spot",
	gdtfMode: "Default",
	universe: 1,
	address: 1,
	matched: true,
	conflicted: true,
};

function preview(fixtures = [CLEAN, UNMATCHED, CONFLICTED]): MvrPreview {
	return {
		fixtures,
		scenery: 2,
		missingProfiles: fixtures.some((fixture) => !fixture.matched)
			? ["Unknown Thing · Mode 1"]
			: [],
		addressConflicts: fixtures.some((fixture) => fixture.conflicted)
			? ["Spot 3 conflicts at universe 1 address 1-4"]
			: [],
	};
}

function renderImport(source = preview()) {
	const imported = vi.fn();
	render(
		<MvrImport
			path="/tmp/rig.mvr"
			preview={source}
			onImported={imported}
			onCancel={vi.fn()}
			onError={vi.fn()}
		/>,
	);
	return imported;
}

beforeEach(() => {
	invoke.mockReset();
	invoke.mockResolvedValue({
		importedFixtures: 3,
		unresolvedFixtures: 0,
		warnings: [],
	});
});

describe("importing an MVR rig", () => {
	it("asks only about the fixtures that need a decision", async () => {
		renderImport();
		expect(
			screen.getByRole("combobox", { name: "Resolution for Mystery 2" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("combobox", { name: "Resolution for Spot 3" }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("combobox", { name: "Resolution for Wash 1" }),
			"a fixture that patches as it stands is not a question",
		).not.toBeInTheDocument();
	});

	it("sends the operator's decisions with the import", async () => {
		const imported = renderImport();
		await userEvent.selectOptions(
			screen.getByRole("combobox", { name: "Resolution for Spot 3" }),
			"skip",
		);
		await userEvent.click(screen.getByRole("button", { name: "Import" }));

		await waitFor(() => expect(invoke).toHaveBeenCalled());
		const [command, payload] = invoke.mock.calls[0];
		expect(command).toBe("import_mvr");
		expect(payload.path).toBe("/tmp/rig.mvr");
		expect(payload.resolutions[CONFLICTED.uuid].action).toBe("skip");
		// Nothing is wrong with this one, so no decision is invented for it.
		expect(payload.resolutions[CLEAN.uuid]).toBeUndefined();
		// An unrecognised GDTF is kept rather than dropped.
		expect(payload.resolutions[UNMATCHED.uuid].action).toBe("import_unpatched");
		await waitFor(() =>
			expect(imported).toHaveBeenCalledWith("Imported 3 fixtures"),
		);
	});

	it("carries the chosen address through", async () => {
		renderImport();
		await userEvent.selectOptions(
			screen.getByRole("combobox", { name: "Resolution for Spot 3" }),
			"address",
		);
		const address = screen.getByRole("spinbutton", {
			name: "Address for Spot 3",
		});
		await userEvent.clear(address);
		await userEvent.type(address, "57");
		await userEvent.click(screen.getByRole("button", { name: "Import" }));

		await waitFor(() => expect(invoke).toHaveBeenCalled());
		const [, payload] = invoke.mock.calls[0];
		expect(payload.resolutions[CONFLICTED.uuid]).toMatchObject({
			action: "address",
			universe: 1,
			address: 57,
		});
	});

	it("says so when there is nothing to decide", () => {
		renderImport(preview([CLEAN]));
		expect(
			screen.getByText("Every fixture patches as it stands."),
		).toBeInTheDocument();
	});
});
