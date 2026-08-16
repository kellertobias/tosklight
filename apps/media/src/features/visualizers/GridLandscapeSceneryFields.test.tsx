import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { GridLandscapeSceneryFields } from "./GridLandscapeSceneryFields";

it("chooses left and right roadside scenery independently", async () => {
	const left = vi.fn();
	const right = vi.fn();
	render(
		<GridLandscapeSceneryFields
			left={1}
			right={2}
			onLeftChange={left}
			onRightChange={right}
		/>,
	);

	await choose("Left scenery", "Palm trees");
	expect(left).toHaveBeenCalledWith(2);
	expect(right).not.toHaveBeenCalled();

	await choose("Right scenery", "Off");
	expect(right).toHaveBeenCalledWith(0);
});

async function choose(labelText: string, option: string) {
	const label = screen.getByText(labelText, { selector: "label" });
	const trigger = label.parentElement?.querySelector<HTMLButtonElement>(
		'button[aria-haspopup="listbox"]',
	);
	expect(trigger).toBeTruthy();
	await userEvent.click(trigger as HTMLButtonElement);
	await userEvent.click(screen.getByRole("option", { name: option }));
}
