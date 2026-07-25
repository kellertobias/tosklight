import { describe, expect, it, vi } from "vitest";
import {
	attachControllableHostedPicker,
	type ControllableHostedPickerOperation,
	type ControllableHostedPickerWindow,
	controllableHostedPickerOutcome,
	decodeHostedPickerOutcome,
	decodeHostedPickerRequest,
	HOSTED_PICKER_TEST_CONTROL,
} from "./controllableHostedPicker";

describe("controllable hosted picker", () => {
	it("stays dormant when no test control was installed", () => {
		const open = vi.fn();
		const detach = attachControllableHostedPicker(
			open,
			{} as ControllableHostedPickerWindow,
		);

		detach();
		expect(open).not.toHaveBeenCalled();
	});

	it("attaches one strict request handler to a complete injected port", () => {
		let handler:
			| ((request: unknown) => ControllableHostedPickerOperation)
			| undefined;
		const detached = vi.fn();
		const port = {
			attach: vi.fn((next) => {
				handler = next;
				return detached;
			}),
			request: vi.fn(),
			dispose: vi.fn(),
		};
		const open = vi.fn(() => operation());
		const detach = attachControllableHostedPicker(open, {
			[HOSTED_PICKER_TEST_CONTROL]: port,
		} as unknown as ControllableHostedPickerWindow);

		const opened = handler?.({
			purpose: "Choose a plot",
			target: "files",
			multiple: true,
			allowedExtensions: ["pdf"],
			initialRootId: "shows",
			initialDirectory: "plots",
			selectLabel: "Use plot",
			cancelLabel: "Keep current",
			hideCancel: false,
		});

		expect(open).toHaveBeenCalledWith({
			purpose: "Choose a plot",
			target: "files",
			multiple: true,
			allowedExtensions: ["pdf"],
			initialRootId: "shows",
			initialDirectory: "plots",
			selectLabel: "Use plot",
			cancelLabel: "Keep current",
			hideCancel: false,
		});
		expect(opened).toBeDefined();
		detach();
		expect(detached).toHaveBeenCalledOnce();
	});

	it("rejects unknown fields, callbacks, and invalid target values", () => {
		expect(() =>
			decodeHostedPickerRequest({ target: "files", surprise: 1 }),
		).toThrow("Invalid hosted-picker request fields");
		expect(() => decodeHostedPickerRequest({ target: "fixtures" })).toThrow(
			"Invalid hosted-picker target",
		);
		expect(() =>
			decodeHostedPickerRequest({ onSelect: () => undefined }),
		).toThrow("Invalid hosted-picker request fields");
	});

	it("rejects incomplete ports and invalid attachment lifecycles", () => {
		expect(() =>
			attachControllableHostedPicker(vi.fn(), {
				[HOSTED_PICKER_TEST_CONTROL]: { attach: vi.fn() },
			} as unknown as ControllableHostedPickerWindow),
		).toThrow("Invalid controllable hosted-picker port");
		expect(() =>
			attachControllableHostedPicker(vi.fn(), {
				[HOSTED_PICKER_TEST_CONTROL]: {
					attach: vi.fn(() => null),
					request: vi.fn(),
					dispose: vi.fn(),
				},
			} as unknown as ControllableHostedPickerWindow),
		).toThrow("Invalid controllable hosted-picker attachment");
	});

	it("decodes every outcome and rejects extra result fields", () => {
		expect(decodeHostedPickerOutcome({ status: "cancelled" })).toEqual({
			status: "cancelled",
		});
		expect(
			decodeHostedPickerOutcome({
				status: "selected",
				selections: [
					{ rootId: "shows", name: "plot.txt", path: "plot.txt", kind: "file" },
				],
			}),
		).toEqual({
			status: "selected",
			selections: [
				{ rootId: "shows", name: "plot.txt", path: "plot.txt", kind: "file" },
			],
		});
		expect(
			decodeHostedPickerOutcome({
				status: "system_selected",
				target: "folders",
				files: [
					{
						name: "plot.pdf",
						type: "application/pdf",
						size: 4,
						lastModified: 12,
					},
				],
				directoryName: "Plots",
			}),
		).toEqual(expect.objectContaining({ status: "system_selected" }));
		expect(() =>
			decodeHostedPickerOutcome({ status: "cancelled", stale: true }),
		).toThrow("Invalid cancelled hosted-picker outcome fields");
	});

	it("projects internal selections without exposing private FileEntry state", () => {
		expect(
			controllableHostedPickerOutcome([
				{
					rootId: "shows",
					entry: {
						name: "plot.txt",
						path: "tour/plot.txt",
						kind: "file",
						size: 12,
						modified_millis: 1,
						created_millis: null,
						hidden: false,
						writable: true,
						mime: "text/plain",
					},
				},
			]),
		).toEqual({
			status: "selected",
			selections: [
				{
					rootId: "shows",
					name: "plot.txt",
					path: "tour/plot.txt",
					kind: "file",
				},
			],
		});
	});

	it("summarizes system files without retaining browser File objects", () => {
		const file = new File(["plot"], "plot.pdf", {
			type: "application/pdf",
			lastModified: 42,
		});

		expect(
			controllableHostedPickerOutcome({
				source: "system",
				target: "folders",
				files: [file],
				directoryName: "Plots",
			}),
		).toEqual({
			status: "system_selected",
			target: "folders",
			files: [
				{
					name: "plot.pdf",
					type: "application/pdf",
					size: 4,
					lastModified: 42,
				},
			],
			directoryName: "Plots",
		});
	});
});

function operation(): ControllableHostedPickerOperation {
	return {
		outcome: Promise.resolve({ status: "cancelled" }),
		cancel: vi.fn(),
	};
}
