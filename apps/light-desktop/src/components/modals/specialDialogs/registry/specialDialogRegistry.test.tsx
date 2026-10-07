import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SemanticSpecialDialogPlaceholder } from "./SemanticSpecialDialogPlaceholder";
import {
	hasSpecialDialog,
	LEGACY_SPECIAL_DIALOGS,
	registerSemanticSpecialDialog,
	resolveSpecialDialog,
	SEMANTIC_SPECIAL_DIALOGS,
} from "./specialDialogRegistry";

describe("special dialog registry", () => {
	it("resolves every family to its legacy dialog under contract 0", () => {
		for (const family of ["Position", "Color", "Shapers", "Media", "Control"])
			expect(resolveSpecialDialog(family, false)).toBe(
				LEGACY_SPECIAL_DIALOGS[family as keyof typeof LEGACY_SPECIAL_DIALOGS],
			);
		expect(resolveSpecialDialog("Focus", false)).toBeNull();
		expect(resolveSpecialDialog("Beam", false)).toBeNull();
		expect(LEGACY_SPECIAL_DIALOGS.Position?.cardClassName).toBe(
			"position-special-dialog",
		);
		expect(LEGACY_SPECIAL_DIALOGS.Shapers?.cardClassName).toBe(
			"shapers-special-dialog-card",
		);
	});

	it("resolves Position, Color and Focus to semantic entries and keeps the rest legacy", () => {
		for (const family of ["Position", "Color", "Focus"] as const)
			expect(resolveSpecialDialog(family, true)).toMatchObject({
				mode: "semantic",
				family,
			});
		for (const family of ["Shapers", "Media", "Control"])
			expect(resolveSpecialDialog(family, true)?.mode).toBe("legacy");
	});

	it("offers a Focus Special Dialog only under the semantic contract", () => {
		expect(hasSpecialDialog("Focus", false)).toBe(false);
		expect(hasSpecialDialog("Focus", true)).toBe(true);
		expect(hasSpecialDialog("Position", false)).toBe(true);
		expect(hasSpecialDialog("Beam", true)).toBe(false);
		expect(hasSpecialDialog("Intensity", false)).toBe(false);
	});

	it("lets a family agent install its production dialog", () => {
		const original = SEMANTIC_SPECIAL_DIALOGS.Focus;
		const FocusDialog = () => null;
		registerSemanticSpecialDialog("Focus", FocusDialog);
		expect(resolveSpecialDialog("Focus", true)).toMatchObject({
			mode: "semantic",
			Component: FocusDialog,
		});
		if (original) SEMANTIC_SPECIAL_DIALOGS.Focus = original;
	});

	it("renders a placeholder modal that closes without sending anything", () => {
		const close = vi.fn();
		render(
			<SemanticSpecialDialogPlaceholder
				family="Focus"
				selectedFixtureIds={["a", "b"]}
				close={close}
			/>,
		);
		expect(screen.getByTestId("semantic-special-dialog-placeholder").textContent).toBe(
			"2 fixtures selected",
		);
		fireEvent.keyDown(document, { key: "Escape" });
		fireEvent.click(screen.getByRole("button", { name: /close/i }));
		expect(close).toHaveBeenCalled();
	});
});
