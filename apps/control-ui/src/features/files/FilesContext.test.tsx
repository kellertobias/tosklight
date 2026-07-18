import { fireEvent, render, screen } from "@testing-library/react";
import { memo } from "react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "../../components/common";
import { FilesProvider, useFiles } from "./FilesContext";
import type { FilesContextValue } from "./types";

function filesSource(fileContent = vi.fn()): FilesContextValue {
	return {
		status: "connected",
		commandLine: "FIXTURE",
		resetCommandLine: vi.fn(),
		systemPickerFallback: false,
		fileRoots: vi.fn(),
		fileEntries: vi.fn(),
		fileMetadata: vi.fn(),
		readFileNote: vi.fn(),
		saveFileNote: vi.fn(),
		readTextFile: vi.fn(),
		saveTextFile: vi.fn(),
		fileOperation: vi.fn(),
		fileContent,
		fileStreamUrl: vi.fn(),
		fileThumbnail: vi.fn(),
		claimFileInput: vi.fn(),
		releaseFileInput: vi.fn(),
	};
}

describe("FilesProvider", () => {
	it("keeps consumers stable while delegating to the latest file actions", () => {
		let renders = 0;
		const first = vi.fn();
		const latest = vi.fn();
		const Consumer = memo(() => {
			renders += 1;
			const files = useFiles();
			return (
				<Button onClick={() => void files.fileContent("shows", "plot.png")}>
					Read
				</Button>
			);
		});
		const source = filesSource(first);
		const view = render(
			<FilesProvider source={source}>
				<Consumer />
			</FilesProvider>,
		);
		view.rerender(
			<FilesProvider source={{ ...source, fileContent: latest }}>
				<Consumer />
			</FilesProvider>,
		);
		expect(renders).toBe(1);
		fireEvent.click(screen.getByRole("button", { name: "Read" }));
		expect(first).not.toHaveBeenCalled();
		expect(latest).toHaveBeenCalledWith("shows", "plot.png");
	});
});
