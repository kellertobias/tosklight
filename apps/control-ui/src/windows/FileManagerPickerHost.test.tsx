import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Button } from "../components/common/controls";
import { FileManagerPickerHost, openFileManagerPicker } from "./FileManagerPickerHost";
import {
	HOSTED_PICKER_TEST_CONTROL,
	type ControllableHostedPickerOperation,
} from "./fileManagerWindow/controllableHostedPicker";

const mocks = vi.hoisted(() => ({
	configuration: { file_manager_system_picker_fallback: false },
	activePicker: null as null | {
		onSelect: (selection: unknown[]) => void;
		onCancel: () => void;
	},
}));

vi.mock("../features/files/FilesContext", () => ({
	useFiles: () => ({
		get systemPickerFallback() {
			return mocks.configuration.file_manager_system_picker_fallback;
		},
	}),
}));

vi.mock("./FileManagerWindow", () => ({
  extension: (name: string) => name.includes(".") ? name.split(".").pop()?.toLowerCase() ?? "" : "",
	FileManager: ({ picker }: { picker: { target?: string; multiple?: boolean; allowedExtensions?: string[]; initialRootId?: string; initialDirectory?: string; onSelect: (selection: unknown[]) => void; onCancel: () => void } }) => {
		mocks.activePicker = picker;
		return <section aria-label="Mock picker">
			<output>{JSON.stringify({ target: picker.target, multiple: picker.multiple, allowedExtensions: picker.allowedExtensions, initialRootId: picker.initialRootId, initialDirectory: picker.initialDirectory })}</output>
			<Button onClick={() => picker.onSelect([mockSelection()])}>Select mock</Button>
			<Button onClick={picker.onCancel}>Cancel mock</Button>
		</section>;
	},
}));

afterEach(() => {
	cleanup();
	delete (window as unknown as Record<string, unknown>)[
		HOSTED_PICKER_TEST_CONTROL
	];
	mocks.activePicker = null;
});

describe("FileManagerPickerHost", () => {
  it("hosts the reusable picker configuration and resolves only after explicit selection", async () => {
    mocks.configuration.file_manager_system_picker_fallback = false;
    render(<FileManagerPickerHost />);
    let result!: Promise<unknown>;
    act(() => {
      result = openFileManagerPicker({
        target: "files",
        multiple: true,
        allowedExtensions: ["txt", "md"],
        initialRootId: "shows",
        initialDirectory: "run",
      });
    });

    expect(screen.getByRole("dialog", { name: "Choose files or folders" })).toBeVisible();
    expect(screen.getByRole("heading", { name: "File Manager" })).toBeVisible();
    expect(screen.getByText("Select files")).toBeVisible();
    expect(screen.getByRole("button", { name: "Close File Manager" })).toBeVisible();
    expect(screen.queryByRole("button", { name: "Open system file picker" })).not.toBeInTheDocument();
    expect(screen.getByText(/"target":"files"/)).toHaveTextContent('"multiple":true');
    fireEvent.click(screen.getByRole("button", { name: "Select mock" }));
		await expect(result).resolves.toEqual([
			expect.objectContaining({
				rootId: "shows",
				entry: expect.objectContaining({ path: "notes.txt" }),
			}),
		]);
    expect(screen.queryByRole("dialog", { name: "Choose files or folders" })).not.toBeInTheDocument();
  });

  it("resolves cancellation as null", async () => {
    mocks.configuration.file_manager_system_picker_fallback = false;
    render(<><section aria-label="Calling setup">Caller remains open</section><FileManagerPickerHost /></>);
    let result!: Promise<unknown>;
    act(() => { result = openFileManagerPicker({ target: "folders" }); });
    fireEvent.click(screen.getByRole("button", { name: "Close File Manager" }));
    await expect(result).resolves.toBeNull();
    expect(screen.getByRole("region", { name: "Calling setup" })).toBeVisible();
    expect(screen.queryByRole("dialog", { name: "Choose files or folders" })).not.toBeInTheDocument();
  });

  it("keeps the system picker constrained when the disabled-by-default fallback is enabled", async () => {
    mocks.configuration.file_manager_system_picker_fallback = true;
    const view = render(<FileManagerPickerHost />);
    let result!: Promise<unknown>;
    act(() => { result = openFileManagerPicker({ target: "files", multiple: true, allowedExtensions: [".gdtf"] }); });

    expect(screen.getByRole("button", { name: "Open system file picker" })).toBeVisible();
    const input = view.container.querySelector<HTMLInputElement>('input[type="file"]')!;
    expect(input).toHaveAttribute("accept", ".gdtf");
    expect(input).toHaveAttribute("multiple");
    fireEvent.change(input, { target: { files: [new File(["fixture"], "tour.gdtf", { type: "application/zip" })] } });
    await expect(result).resolves.toEqual(expect.objectContaining({
      source: "system",
      target: "files",
      files: [expect.objectContaining({ name: "tour.gdtf" })],
    }));
  });

  it("rejects a system-picked file outside the calling form's extension filter", async () => {
    mocks.configuration.file_manager_system_picker_fallback = true;
    const view = render(<FileManagerPickerHost />);
    let result!: Promise<unknown>;
    act(() => { result = openFileManagerPicker({ target: "files", allowedExtensions: ["gdtf"] }); });

    const input = view.container.querySelector<HTMLInputElement>('input[type="file"]')!;
    fireEvent.change(input, { target: { files: [new File(["image"], "wrong.png")] } });
    expect(screen.getByRole("alert")).toHaveTextContent("Choose only .gdtf files");
    fireEvent.click(screen.getByRole("button", { name: "Cancel mock" }));
    await expect(result).resolves.toBeNull();
  });

  it("configures the system fallback as a directory chooser for folder targets", () => {
    mocks.configuration.file_manager_system_picker_fallback = true;
    const view = render(<FileManagerPickerHost />);
    act(() => { void openFileManagerPicker({ target: "folders", multiple: false }); });

    const input = view.container.querySelector<HTMLInputElement>('input[type="file"]')!;
    expect(input).toHaveAttribute("webkitdirectory");
    expect(input).toHaveAttribute("multiple");
    expect(input).not.toHaveAttribute("accept");
  });

	it("accepts a typed controlled request and returns only the public selection", async () => {
		mocks.configuration.file_manager_system_picker_fallback = false;
		const control = installControllablePicker();
		render(<FileManagerPickerHost />);
		let operation!: ControllableHostedPickerOperation;
		act(() => {
			operation = control.request({
				target: "files",
				allowedExtensions: ["txt"],
			});
		});

		expect(screen.getByText(/"target":"files"/)).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Select mock" }));
		await expect(operation.outcome).resolves.toEqual({
			status: "selected",
			selections: [
				{
					rootId: "shows",
					name: "notes.txt",
					path: "notes.txt",
					kind: "file",
				},
			],
		});
	});

	it("cancels a replaced operation and ignores its late cancellation", async () => {
		const control = installControllablePicker();
		render(<FileManagerPickerHost />);
		let first!: ControllableHostedPickerOperation;
		let second!: ControllableHostedPickerOperation;
		act(() => {
			first = control.request({ target: "files" });
		});
		const lateFirstSelection = mocks.activePicker?.onSelect;
		expect(lateFirstSelection).toBeTypeOf("function");
		act(() => {
			second = control.request({ target: "folders" });
			lateFirstSelection?.([mockSelection()]);
			first.cancel();
		});

		await expect(first.outcome).resolves.toEqual({ status: "cancelled" });
		expect(screen.getByText(/"target":"folders"/)).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Select mock" }));
		await expect(second.outcome).resolves.toEqual(
			expect.objectContaining({ status: "selected" }),
		);
	});

	it("cancels the active operation when the driver requests disposal", async () => {
		const control = installControllablePicker();
		render(<FileManagerPickerHost />);
		let operation!: ControllableHostedPickerOperation;
		act(() => {
			operation = control.request({ target: "either" });
			operation.cancel();
		});

		await expect(operation.outcome).resolves.toEqual({ status: "cancelled" });
		expect(
			screen.queryByRole("dialog", { name: "Choose files or folders" }),
		).not.toBeInTheDocument();
	});

	it("detaches and settles an active request when the host unmounts", async () => {
		const control = installControllablePicker();
		const view = render(<FileManagerPickerHost />);
		let operation!: ControllableHostedPickerOperation;
		act(() => {
			operation = control.request({ target: "folders" });
		});

		const settled = vi.fn();
		void operation.outcome.then(settled);
		view.unmount();
		operation.cancel();

		await expect(operation.outcome).resolves.toEqual({ status: "cancelled" });
		await Promise.resolve();
		expect(settled).toHaveBeenCalledOnce();
		expect(control.detach).toHaveBeenCalledOnce();
	});
});

function installControllablePicker() {
	let handler:
		| ((request: unknown) => ControllableHostedPickerOperation)
		| undefined;
	const detach = vi.fn();
	const port = {
		attach: vi.fn(
			(next: (request: unknown) => ControllableHostedPickerOperation) => {
				handler = next;
				return detach;
			},
		),
		request: vi.fn(),
		dispose: vi.fn(),
	};
	Object.defineProperty(window, HOSTED_PICKER_TEST_CONTROL, {
		configurable: true,
		value: port,
	});
	return {
		detach,
		request(value: unknown) {
			if (!handler) throw new Error("Hosted-picker handler is not attached");
			return handler(value);
		},
	};
}

function mockSelection() {
	return {
		rootId: "shows",
		entry: {
			name: "notes.txt",
			path: "notes.txt",
			kind: "file",
			size: 5,
			modified_millis: 1,
			created_millis: null,
			hidden: false,
			writable: true,
		},
	};
}
