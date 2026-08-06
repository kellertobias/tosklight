import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { initialState } from "../../state/appReducer";
import { DeskSettingsModal } from "./DeskSettingsModal";

const context = vi.hoisted(() => ({
	dispatch: vi.fn(),
	state: null as unknown as typeof initialState,
}));

vi.mock("../../state/AppContext", () => ({
	useApp: () => context,
}));

function renderModal(desks = initialState.desks) {
	context.state = {
		...initialState,
		desks,
		deskSettingsOpen: true,
		deskSettingsId: desks[0].id,
	};
	return render(<DeskSettingsModal />, { wrapper: ModalProvider });
}

beforeEach(() => context.dispatch.mockReset());
afterEach(cleanup);

describe("DeskSettingsModal", () => {
	it("places Delete beside Close and keeps Clone as the large neutral action", () => {
		renderModal();
		const dialog = screen.getByRole("dialog", { name: "Desktop settings" });
		const remove = screen.getByRole("button", { name: "Delete desktop" });
		const close = screen.getByRole("button", {
			name: "Close Desktop settings",
		});
		expect(
			remove.compareDocumentPosition(close) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		const clone = screen.getByRole("button", {
			name: "Clone current desktop",
		});
		expect(clone.classList.contains("large-action")).toBe(true);
		expect(clone.classList.contains("danger")).toBe(false);
		expect(dialog.contains(clone)).toBe(true);
	});

	it("confirms deletion and preserves the existing clone actions", () => {
		renderModal();
		fireEvent.click(screen.getByRole("button", { name: "Delete desktop" }));
		const prompt = screen.getByText("Delete desktop “Programming”?", {
			selector: "b",
		});
		const confirmDialog = screen.getByRole("alertdialog", {
			name: "Delete desktop",
		});
		expect(confirmDialog.contains(prompt)).toBe(true);
		expect(
			screen
				.getByRole("dialog", { name: "Desktop settings" })
				.contains(confirmDialog),
		).toBe(false);
		fireEvent.click(screen.getByRole("button", { name: "Confirm delete" }));
		expect(context.dispatch).toHaveBeenCalledWith({
			type: "DELETE_DESK",
			id: initialState.desks[0].id,
		});

		context.dispatch.mockReset();
		cleanup();
		renderModal();
		fireEvent.click(
			screen.getByRole("button", { name: "Clone current desktop" }),
		);
		expect(context.dispatch.mock.calls.map(([action]) => action.type)).toEqual([
			"START_SAVE_DESK",
			"NEW_DESK",
			"OPEN_DESK_SETTINGS",
		]);
	});

	it("does not offer deletion when only one Desktop remains", () => {
		renderModal([initialState.desks[0]]);
		expect(
			(
				screen.getByRole("button", {
					name: "Delete desktop",
				}) as HTMLButtonElement
			).disabled,
		).toBe(true);
	});
});
