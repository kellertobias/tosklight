import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { type CadPaperwork, CadProjectPanel } from "./CadProjectPanel";

const logo = JSON.stringify({
	mediaType: "image/jpeg",
	width: 120,
	height: 60,
	data: "/9j/4AAQ",
});

function paperwork(overrides: Partial<CadPaperwork> = {}): CadPaperwork {
	return {
		project: "Summer Tour",
		lightingDesigner: "Tobias Keller",
		venue: "Grand Hall",
		contactEmail: "ld@example.com",
		contactPhone: "+49 30 1234",
		showDate: "2026-09-15",
		showVersion: "1.2",
		companyLogo: "",
		...overrides,
	};
}

function setup(overrides: Partial<CadPaperwork> = {}) {
	const handlers = {
		onChange: vi.fn(),
		onSave: vi.fn(),
		onUploadLogo: vi.fn(),
		onRemoveLogo: vi.fn(),
		onMakeDefault: vi.fn(),
	};
	render(
		<CadProjectPanel
			paperwork={paperwork(overrides)}
			documentInfo={null}
			saving={false}
			{...handlers}
		/>,
	);
	return handlers;
}

describe("the show's project information", () => {
	it("puts the show on the left and the lighting designer on the right", () => {
		setup();
		const groups = screen.getAllByRole("group");
		expect(groups.map((group) => group.querySelector("legend")?.textContent)).toEqual([
			"Show",
			"Lighting designer",
		]);
		const [show, designer] = groups;
		const labels = (group: HTMLElement) =>
			[...group.querySelectorAll("label")].map((label) => label.firstChild?.textContent);
		expect(labels(show)).toEqual(["Project", "Venue", "Show date", "Version"]);
		expect(labels(designer)).toEqual(["Name", "Phone", "Email"]);
		expect(within(designer).getByLabelText("Lighting designer name")).toHaveValue("Tobias Keller");
		expect(within(designer).getByLabelText("Lighting designer phone")).toHaveValue("+49 30 1234");
		expect(within(designer).getByLabelText("Lighting designer email")).toHaveValue("ld@example.com");
		expect(within(show).getByLabelText("Show version")).toHaveValue("1.2");
	});

	it("uploads a company logo and makes the lighting designer this computer's default", () => {
		const handlers = setup();
		const designer = screen.getByRole("group", { name: "Lighting designer" });
		expect(within(designer).queryByRole("img", { name: "Company logo" })).toBeNull();
		fireEvent.click(within(designer).getByRole("button", { name: "Upload logo" }));
		expect(handlers.onUploadLogo).toHaveBeenCalledOnce();
		fireEvent.click(within(designer).getByRole("button", { name: "Make Default" }));
		expect(handlers.onMakeDefault).toHaveBeenCalledOnce();
		fireEvent.change(within(designer).getByLabelText("Lighting designer phone"), {
			target: { value: "+49 40 5678" },
		});
		expect(handlers.onChange).toHaveBeenCalledWith("contactPhone", "+49 40 5678");
	});

	it("shows a stored logo, which can be replaced or removed", () => {
		const handlers = setup({ companyLogo: logo });
		const designer = screen.getByRole("group", { name: "Lighting designer" });
		expect(within(designer).getByRole("img", { name: "Company logo" })).toHaveAttribute(
			"src",
			"data:image/jpeg;base64,/9j/4AAQ",
		);
		expect(within(designer).getByRole("button", { name: "Replace logo" })).toBeVisible();
		fireEvent.click(within(designer).getByRole("button", { name: "Remove logo" }));
		expect(handlers.onRemoveLogo).toHaveBeenCalledOnce();
	});
});
