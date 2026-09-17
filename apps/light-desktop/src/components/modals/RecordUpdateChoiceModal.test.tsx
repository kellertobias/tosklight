import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RecordUpdateChoiceModal } from "./RecordUpdateChoiceModal";

afterEach(cleanup);

function renderModal(
	props: Partial<Parameters<typeof RecordUpdateChoiceModal>[0]> = {},
) {
	const onConfirm = vi.fn();
	const onCancel = vi.fn();
	render(
		<RecordUpdateChoiceModal
			kind="record"
			storedDefault="smart"
			busy={false}
			error={null}
			onConfirm={onConfirm}
			onCancel={onCancel}
			{...props}
		/>,
	);
	return { onConfirm, onCancel };
}

describe("Record and Update choice modal", () => {
	it("titles Record with the four choices, an off default toggle, and Record in the title", () => {
		const { onConfirm } = renderModal();
		const dialog = screen.getByRole("dialog", { name: "Record" });
		const modes = within(dialog).getByRole("radiogroup", {
			name: "Record mode",
		});
		expect(
			within(modes)
				.getAllByRole("radio")
				.map((radio) => radio.textContent),
		).toEqual(["Smart", "Merge", "Add Existing", "Add Cue"]);
		const toggle = within(dialog).getByRole("switch", {
			name: "Set as default",
		});
		expect(toggle).not.toBeChecked();
		expect(within(dialog).getByText("Current default:")).toBeInTheDocument();
		const header = dialog.querySelector(".modal-title-bar, header") ?? dialog;
		fireEvent.click(within(header as HTMLElement).getByRole("button", { name: "Record" }));
		expect(onConfirm).toHaveBeenCalledWith("smart", false);
	});

	it("records a one-off choice or stores it as the default", () => {
		const { onConfirm } = renderModal({ storedDefault: "merge" });
		expect(screen.getByRole("radio", { name: "Merge" })).toHaveAttribute(
			"aria-checked",
			"true",
		);
		expect(screen.getByText(/Smart with Set as default returns/)).toBeInTheDocument();
		fireEvent.click(screen.getByRole("radio", { name: "Add Cue" }));
		expect(
			screen.getByText("Always stores the programmer as a new Cue at the end."),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Record" }));
		expect(onConfirm).toHaveBeenLastCalledWith("add_cue", false);
		fireEvent.click(screen.getByRole("radio", { name: "Smart" }));
		fireEvent.click(screen.getByRole("switch", { name: "Set as default" }));
		fireEvent.click(screen.getByRole("button", { name: "Record" }));
		expect(onConfirm).toHaveBeenLastCalledWith("smart", true);
	});

	it("starts from the one-off option the armed line names", () => {
		renderModal({ storedDefault: "smart", initialOption: "add_existing" });
		expect(screen.getByRole("radio", { name: "Add Existing" })).toHaveAttribute(
			"aria-checked",
			"true",
		);
	});

	it("uses the same layout titled Update, with Update and Targets actions", () => {
		const onTargets = vi.fn();
		const { onConfirm, onCancel } = renderModal({
			kind: "update",
			onTargets,
		});
		const dialog = screen.getByRole("dialog", { name: "Update" });
		expect(
			within(dialog).getByRole("radiogroup", { name: "Update mode" }),
		).toBeInTheDocument();
		fireEvent.click(within(dialog).getByRole("radio", { name: "Merge" }));
		expect(within(dialog).getByText(/Update All/)).toBeInTheDocument();
		fireEvent.click(within(dialog).getByRole("button", { name: "Update" }));
		expect(onConfirm).toHaveBeenCalledWith("merge", false);
		fireEvent.click(within(dialog).getByRole("button", { name: "Targets" }));
		expect(onTargets).toHaveBeenCalledOnce();
		fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
		expect(onCancel).toHaveBeenCalled();
	});

	it("keeps Record disabled until the stored default has loaded", () => {
		const { onConfirm } = renderModal({ storedDefault: null });
		fireEvent.click(screen.getByRole("button", { name: "Record" }));
		expect(onConfirm).not.toHaveBeenCalled();
	});
});
