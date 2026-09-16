import {
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FixtureProfile } from "../../api/types";
import { FixtureProfileEditor } from "./FixtureProfileEditor";
import { blankFixtureProfile } from "./fixtureProfileModel";

vi.mock("../files/RootConfinedFilePickerButton", () => ({
	RootConfinedFilePickerButton: ({ label }: { label: string }) => (
		<span>{label}</span>
	),
}));

afterEach(cleanup);

function validProfile(): FixtureProfile {
	const profile = blankFixtureProfile();
	profile.manufacturer = "Acme";
	profile.name = "Orbit";
	return profile;
}

function openSimulation(
	profile: FixtureProfile,
	save = vi.fn(async (draft: FixtureProfile) => draft),
) {
	rtlRender(
		<FixtureProfileEditor
			initialProfile={profile}
			manufacturers={[]}
			onSave={save}
			onClose={vi.fn()}
		/>,
		{ wrapper: ModalProvider },
	);
	fireEvent.click(screen.getByRole("tab", { name: "Simulation" }));
	return save;
}

function type(label: string, value: string) {
	fireEvent.change(screen.getByLabelText(label), { target: { value } });
}

function fieldError(label: string) {
	const field = screen.getByLabelText(label).closest(".ui-form-field");
	if (!(field instanceof HTMLElement)) throw new Error(`No field ${label}`);
	return within(field).queryByRole("alert")?.textContent ?? null;
}

function saveErrors() {
	return within(
		screen.getByText("Fixture profile needs attention").closest("section") ??
			document.body,
	)
		.getAllByRole("listitem")
		.map((item) => item.textContent);
}

describe("FixtureProfileEditor physical units and precision", () => {
	it("stores whole millimetres and watts, kilograms to two places, and percentages to one", async () => {
		const save = openSimulation(validProfile());
		expect(screen.getByLabelText("Width (mm)")).toBeInTheDocument();
		expect(screen.getByLabelText("Height (mm)")).toBeInTheDocument();
		expect(screen.getByLabelText("Depth (mm)")).toBeInTheDocument();
		expect(screen.getByLabelText("Weight (kg)")).toBeInTheDocument();
		expect(screen.getByLabelText("Power consumption (W)")).toBeInTheDocument();
		type("Width (mm)", "1");
		type("Height (mm)", "99999");
		type("Depth (mm)", "310");
		type("Weight (kg)", "0.01");
		type("Power consumption (W)", "1500");
		type("Sharpness (%)", "28.9");
		type("Uniformity (%)", "100");
		for (const label of [
			"Width (mm)",
			"Height (mm)",
			"Depth (mm)",
			"Weight (kg)",
			"Power consumption (W)",
			"Sharpness (%)",
			"Uniformity (%)",
		])
			expect(fieldError(label)).toBeNull();
		fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));

		await waitFor(() => expect(save).toHaveBeenCalledOnce());
		const saved = save.mock.calls[0][0];
		expect(saved.physical).toMatchObject({
			width_millimetres: 1,
			height_millimetres: 99999,
			depth_millimetres: 310,
			weight_kilograms: 0.01,
			power_watts: 1500,
		});
		// Snapped to its precision, not the binary 0.28900000000000003.
		expect(saved.optics).toMatchObject({ sharpness: 0.289, uniformity: 1 });
	});

	it("names a decimal dimension or power and more than two kilogram places, and refuses to save", () => {
		const save = openSimulation(validProfile());
		type("Width (mm)", "420.5");
		type("Height (mm)", "12.");
		type("Depth (mm)", "0.1");
		type("Weight (kg)", "24.555");
		type("Power consumption (W)", "720.4");

		// The typed decimal stays visible instead of being folded into another whole number.
		expect(screen.getByLabelText("Width (mm)")).toHaveValue("420.5");
		expect(screen.getByLabelText("Width (mm)")).toHaveAttribute(
			"aria-invalid",
			"true",
		);
		expect(fieldError("Width (mm)")).toBe(
			"Width must be a whole number of millimetres",
		);
		// A trailing point is still the whole number 12.
		expect(fieldError("Height (mm)")).toBeNull();
		expect(fieldError("Depth (mm)")).toBe(
			"Depth must be a whole number of millimetres",
		);
		expect(fieldError("Weight (kg)")).toBe(
			"Weight allows at most 2 decimal places (kg)",
		);
		expect(fieldError("Power consumption (W)")).toBe(
			"Power consumption must be a whole number of watts",
		);

		fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
		expect(save).not.toHaveBeenCalled();
		expect(saveErrors()).toEqual([
			"Width must be a whole number of millimetres",
			"Depth must be a whole number of millimetres",
			"Weight allows at most 2 decimal places (kg)",
			"Power consumption must be a whole number of watts",
		]);
	});

	it("keeps sharpness and uniformity to one decimal place within 0 to 100", async () => {
		const save = openSimulation(validProfile());
		type("Sharpness (%)", "85.55");
		expect(fieldError("Sharpness (%)")).toBe(
			"Sharpness must be a percentage with one decimal place",
		);
		fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
		expect(save).not.toHaveBeenCalled();

		type("Sharpness (%)", "150");
		type("Uniformity (%)", "-4");
		expect(fieldError("Sharpness (%)")).toBeNull();
		fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
		await waitFor(() => expect(save).toHaveBeenCalledOnce());
		expect(save.mock.calls[0][0].optics).toMatchObject({
			sharpness: 1,
			uniformity: 0,
		});
	});

	it("reloads a saved profile with its units and precision intact", async () => {
		const save = openSimulation(validProfile());
		type("Width (mm)", "432");
		type("Weight (kg)", "1.98");
		type("Power consumption (W)", "85");
		type("Sharpness (%)", "85");
		type("Uniformity (%)", "62.5");
		fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
		await waitFor(() => expect(save).toHaveBeenCalledOnce());
		// What the server stores and hands back is plain JSON.
		const reloaded = JSON.parse(
			JSON.stringify(save.mock.calls[0][0]),
		) as FixtureProfile;
		expect(reloaded.physical).toMatchObject({
			width_millimetres: 432,
			weight_kilograms: 1.98,
			power_watts: 85,
		});
		expect(reloaded.optics).toMatchObject({
			sharpness: 0.85,
			uniformity: 0.625,
		});
		cleanup();

		const resave = openSimulation(reloaded);
		expect(screen.getByLabelText("Width (mm)")).toHaveValue("432");
		expect(screen.getByLabelText("Weight (kg)")).toHaveValue("1.98");
		expect(screen.getByLabelText("Power consumption (W)")).toHaveValue("85");
		// Always shown with exactly one decimal place.
		expect(screen.getByLabelText("Sharpness (%)")).toHaveValue("85.0");
		expect(screen.getByLabelText("Uniformity (%)")).toHaveValue("62.5");
		fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
		await waitFor(() => expect(resave).toHaveBeenCalledOnce());
		expect(resave.mock.calls[0][0].physical).toEqual(reloaded.physical);
		expect(resave.mock.calls[0][0].optics).toEqual(reloaded.optics);
	});

	it("names a legacy profile's off-precision figures instead of silently rounding them", async () => {
		const legacy = validProfile();
		legacy.physical.height_millimetres = 498.2;
		legacy.physical.weight_kilograms = 1.975;
		const save = openSimulation(legacy);
		expect(screen.getByLabelText("Height (mm)")).toHaveValue("498.2");
		expect(fieldError("Height (mm)")).toBe(
			"Height must be a whole number of millimetres",
		);
		expect(fieldError("Weight (kg)")).toBe(
			"Weight allows at most 2 decimal places (kg)",
		);
		fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
		expect(save).not.toHaveBeenCalled();
		type("Height (mm)", "498");
		type("Weight (kg)", "1.98");
		fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
		await waitFor(() => expect(save).toHaveBeenCalledOnce());
	});
});
