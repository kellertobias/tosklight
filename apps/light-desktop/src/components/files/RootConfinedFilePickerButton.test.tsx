import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RootConfinedFilePickerButton } from "./RootConfinedFilePickerButton";

const mocks = vi.hoisted(() => ({
	open: vi.fn(),
	fileContent: vi.fn(),
}));

vi.mock("../../windows/FileManagerPickerHost", () => ({
	openFileManagerPicker: mocks.open,
}));
vi.mock("../../features/files/FilesContext", () => ({
	useFiles: () => ({ fileContent: mocks.fileContent }),
}));

afterEach(() => {
	cleanup();
	vi.resetAllMocks();
});

describe("RootConfinedFilePickerButton", () => {
	it("opens the ToskLight picker first and converts its confined selection into form File objects", async () => {
		mocks.open.mockResolvedValue([
			{
				rootId: "shows",
				entry: {
					name: "tour.gdtf",
					path: "imports/tour.gdtf",
					modified_millis: 123,
				},
			},
		]);
		mocks.fileContent.mockResolvedValue(
			new Blob(["fixture"], { type: "application/zip" }),
		);
		const onFiles = vi.fn();
		render(
			<RootConfinedFilePickerButton
				label="Choose GDTF file"
				allowedExtensions={["gdtf"]}
				onFiles={onFiles}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Choose GDTF file" }));
		await waitFor(() =>
			expect(mocks.open).toHaveBeenCalledWith({
				purpose: "Choose GDTF file",
				target: "files",
				multiple: false,
				allowedExtensions: ["gdtf"],
			}),
		);
		expect(mocks.fileContent).toHaveBeenCalledWith(
			"shows",
			"imports/tour.gdtf",
		);
		await waitFor(() => expect(onFiles).toHaveBeenCalledOnce());
		const file = onFiles.mock.calls[0][0][0] as File;
		expect(file.name).toBe("tour.gdtf");
		expect(file.type).toBe("application/zip");
	});

	it("passes constrained system fallback File objects through without server reads", async () => {
		const selected = new File(["show"], "tour.show");
		mocks.open.mockResolvedValue({
			source: "system",
			target: "files",
			files: [selected],
		});
		const onFiles = vi.fn();
		render(
			<RootConfinedFilePickerButton
				label="Choose show"
				allowedExtensions={["show"]}
				onFiles={onFiles}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Choose show" }));
		await waitFor(() => expect(onFiles).toHaveBeenCalledWith([selected]));
		expect(mocks.fileContent).not.toHaveBeenCalled();
	});

	it("keeps its own accessible name when embedded in a descriptive form group", () => {
		mocks.open.mockResolvedValue(null);
		render(
			<fieldset>
				<legend>Stage icon</legend>
				<RootConfinedFilePickerButton
					label="Choose stage icon"
					onFiles={vi.fn()}
				/>
				<small>PNG, SVG, or other browser image</small>
			</fieldset>,
		);

		expect(
			screen.getByRole("button", { name: "Choose stage icon" }),
		).toBeVisible();
	});
	it("reports the byte-read interval, rejects duplicate clicks and clears busy on an actionable failure", async () => {
		let fail: (error: Error) => void = () => {};
		mocks.open.mockResolvedValue([
			{ rootId: "shows", entry: { name: "tour.gdtf", path: "tour.gdtf" } },
		]);
		mocks.fileContent.mockImplementation(
			() =>
				new Promise((_resolve, reject) => {
					fail = reject;
				}),
		);
		const readBusy = vi.fn();
		const onFiles = vi.fn();
		render(
			<RootConfinedFilePickerButton
				label="Choose file"
				onFiles={onFiles}
				onReadBusyChange={readBusy}
			/>,
		);
		const button = screen.getByRole("button", { name: "Choose file" });
		fireEvent.click(button);
		fireEvent.click(button);
		await waitFor(() => expect(readBusy).toHaveBeenCalledWith(true));
		expect(mocks.open).toHaveBeenCalledOnce();
		expect(button).toBeDisabled();
		fail(new Error("Archive cannot be read"));
		await waitFor(() =>
			expect(screen.getByRole("alert")).toHaveTextContent(
				"Archive cannot be read",
			),
		);
		expect(readBusy.mock.calls.map(([busy]) => busy)).toEqual([true, false]);
		expect(onFiles).not.toHaveBeenCalled();
		expect(button).toBeEnabled();
	});
	it("does not deliver a late file to an importer that has unmounted", async () => {
		let finish: (content: Blob) => void = () => {};
		mocks.open.mockResolvedValue([
			{ rootId: "shows", entry: { name: "tour.gdtf", path: "tour.gdtf" } },
		]);
		mocks.fileContent.mockImplementation(
			() =>
				new Promise((resolve) => {
					finish = resolve;
				}),
		);
		const onFiles = vi.fn();
		const view = render(
			<RootConfinedFilePickerButton label="Choose file" onFiles={onFiles} />,
		);
		fireEvent.click(screen.getByRole("button", { name: "Choose file" }));
		await waitFor(() => expect(mocks.fileContent).toHaveBeenCalledOnce());
		view.unmount();
		finish(new Blob(["fixture"]));
		await Promise.resolve();
		await Promise.resolve();
		expect(onFiles).not.toHaveBeenCalled();
	});
});
