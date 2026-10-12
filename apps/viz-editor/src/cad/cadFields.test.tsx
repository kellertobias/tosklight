import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useLayoutEffect } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { CommitNumber, CommitText, CommitTextArea } from "./cadFields";

afterEach(cleanup);

it("preserves a native edit made after the field mounts but before passive synchronization", () => {
	const commit = vi.fn();
	function EarlyEdit() {
		useLayoutEffect(() => {
			fireEvent.change(screen.getByLabelText("Position X"), {
				target: { value: "1 THRU 2" },
			});
		}, []);
		return <CommitText label="Position X" value="-3" onCommit={commit} />;
	}
	render(<EarlyEdit />);
	const input = screen.getByLabelText("Position X");
	expect(input).toHaveValue("1 THRU 2");
	expect(commit).not.toHaveBeenCalled();
	fireEvent.keyDown(input, { key: "Enter" });
	expect(commit).toHaveBeenCalledExactlyOnceWith("1 THRU 2");
});

it("keeps an unfinished edit on same-value synchronization and Escape restores the current source", () => {
	const commit = vi.fn();
	const { rerender } = render(<CommitText label="Position X" value="-3" onCommit={commit} />);
	const input = screen.getByLabelText("Position X");
	fireEvent.change(input, { target: { value: "1 THRU" } });
	rerender(<CommitText label="Position X" value="-3" onCommit={(draft) => commit(draft)} />);
	expect(input).toHaveValue("1 THRU");
	expect(commit).not.toHaveBeenCalled();
	fireEvent.keyDown(input, { key: "Escape" });
	expect(input).toHaveValue("-3");
	expect(commit).not.toHaveBeenCalled();
	fireEvent.change(input, { target: { value: "9 THRU" } });
	rerender(<CommitText label="Position X" value="-5" onCommit={commit} />);
	expect(input).toHaveValue("-5");
	fireEvent.keyDown(input, { key: "Enter" });
	expect(commit).not.toHaveBeenCalled();
});

it("keeps numeric drafts on incidental synchronization and resets to changed authoritative values", () => {
	const commit = vi.fn();
	const { rerender } = render(<CommitNumber label="Position X" value={-3} onCommit={commit} />);
	const input = screen.getByLabelText("Position X");
	fireEvent.change(input, { target: { value: "-2." } });
	rerender(<CommitNumber label="Position X" value={-3} onCommit={(value) => commit(value)} />);
	expect(input).toHaveValue("-2.");
	expect(commit).not.toHaveBeenCalled();
	fireEvent.keyDown(input, { key: "Escape" });
	expect(input).toHaveValue("-3");
	fireEvent.change(input, { target: { value: "-9." } });
	rerender(<CommitNumber label="Position X" value={-5} onCommit={commit} />);
	expect(input).toHaveValue("-5");
	fireEvent.keyDown(input, { key: "Enter" });
	fireEvent.blur(input);
	expect(commit).not.toHaveBeenCalled();
});

it("keeps multiline drafts on incidental synchronization and Escape uses the latest source", () => {
	const commit = vi.fn();
	const { rerender } = render(<CommitTextArea label="Notes" value="Original" onCommit={commit} />);
	const input = screen.getByLabelText("Notes");
	fireEvent.change(input, { target: { value: "Unfinished\nnotes" } });
	rerender(<CommitTextArea label="Notes" value="Original" onCommit={(value) => commit(value)} />);
	expect(input).toHaveValue("Unfinished\nnotes");
	expect(commit).not.toHaveBeenCalled();
	fireEvent.keyDown(input, { key: "Escape" });
	expect(input).toHaveValue("Original");
	fireEvent.change(input, { target: { value: "Another draft" } });
	rerender(<CommitTextArea label="Notes" value="New target" onCommit={commit} />);
	expect(input).toHaveValue("New target");
	fireEvent.keyDown(input, { key: "Escape" });
	fireEvent.blur(input);
	expect(commit).not.toHaveBeenCalled();
});
