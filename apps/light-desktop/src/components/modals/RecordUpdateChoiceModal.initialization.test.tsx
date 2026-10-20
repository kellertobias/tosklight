import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
} from "@testing-library/react";
import { useLayoutEffect } from "react";
import { afterEach, expect, it, vi } from "vitest";
import type { RecordUpdateOption } from "../../api/types";
import { RecordUpdateChoiceModal } from "./RecordUpdateChoiceModal";

afterEach(cleanup);

it("retains Merge when chosen on the commit that enables Record after delayed defaults", async () => {
	const confirm = vi.fn();
	function DelayedDefault({
		storedDefault,
	}: {
		storedDefault: RecordUpdateOption | null;
	}) {
		// A real click at the enabled commit precedes passive effects. This reproduces the
		// initialization race deterministically instead of relying on CI scheduling/load.
		useLayoutEffect(() => {
			if (storedDefault) screen.getByRole("radio", { name: "Merge" }).click();
		}, [storedDefault]);
		return (
			<RecordUpdateChoiceModal
				kind="record"
				storedDefault={storedDefault}
				busy={false}
				error={null}
				onConfirm={confirm}
				onCancel={() => {}}
			/>
		);
	}
	const { rerender } = render(<DelayedDefault storedDefault={null} />);
	expect(screen.getByRole("button", { name: "Record" })).toBeDisabled();
	await act(async () => {
		rerender(<DelayedDefault storedDefault="smart" />);
	});
	expect(screen.getByRole("radio", { name: "Merge" })).toHaveAttribute(
		"aria-checked",
		"true",
	);
	fireEvent.click(screen.getByRole("button", { name: "Record" }));
	expect(confirm).toHaveBeenCalledExactlyOnceWith("merge", false);
});

it("follows delayed defaults until the operator chooses, without replacing that choice later", () => {
	const confirm = vi.fn();
	const props = {
		kind: "record" as const,
		busy: false,
		error: null,
		onConfirm: confirm,
		onCancel: () => {},
	};
	const { rerender } = render(
		<RecordUpdateChoiceModal
			{...props}
			storedDefault={null}
			initialOption="add_existing"
		/>,
	);
	rerender(
		<RecordUpdateChoiceModal
			{...props}
			storedDefault="smart"
			initialOption="add_existing"
		/>,
	);
	expect(screen.getByRole("radio", { name: "Add Existing" })).toHaveAttribute(
		"aria-checked",
		"true",
	);
	fireEvent.click(screen.getByRole("radio", { name: "Merge" }));
	rerender(
		<RecordUpdateChoiceModal
			{...props}
			storedDefault="add_cue"
			initialOption="smart"
		/>,
	);
	expect(screen.getByRole("radio", { name: "Merge" })).toHaveAttribute(
		"aria-checked",
		"true",
	);
	fireEvent.click(screen.getByRole("button", { name: "Record" }));
	expect(confirm).toHaveBeenCalledExactlyOnceWith("merge", false);
});

it("starts a fresh choice and default switch when changing between Record and Update", () => {
	const props = {
		busy: false,
		error: null,
		onConfirm: vi.fn(),
		onCancel: () => {},
	};
	const { rerender } = render(
		<RecordUpdateChoiceModal {...props} kind="record" storedDefault="smart" />,
	);
	fireEvent.click(screen.getByRole("radio", { name: "Merge" }));
	fireEvent.click(screen.getByRole("switch", { name: "Set as default" }));
	rerender(
		<RecordUpdateChoiceModal
			{...props}
			kind="update"
			storedDefault="add_cue"
		/>,
	);
	expect(screen.getByRole("radio", { name: "Add Cue" })).toHaveAttribute(
		"aria-checked",
		"true",
	);
	expect(
		screen.getByRole("switch", { name: "Set as default" }),
	).not.toBeChecked();
	rerender(
		<RecordUpdateChoiceModal {...props} kind="record" storedDefault="smart" />,
	);
	expect(screen.getByRole("radio", { name: "Smart" })).toHaveAttribute(
		"aria-checked",
		"true",
	);
});

it("uses the new armed option when the modal is closed and reopened", () => {
	const props = {
		kind: "record" as const,
		busy: false,
		error: null,
		onConfirm: vi.fn(),
		onCancel: () => {},
	};
	const { rerender } = render(
		<RecordUpdateChoiceModal {...props} storedDefault="smart" />,
	);
	fireEvent.click(screen.getByRole("radio", { name: "Merge" }));
	rerender(<></>);
	rerender(
		<RecordUpdateChoiceModal
			{...props}
			storedDefault="smart"
			initialOption="add_existing"
		/>,
	);
	expect(screen.getByRole("radio", { name: "Add Existing" })).toHaveAttribute(
		"aria-checked",
		"true",
	);
	expect(
		screen.getByRole("switch", { name: "Set as default" }),
	).not.toBeChecked();
});
