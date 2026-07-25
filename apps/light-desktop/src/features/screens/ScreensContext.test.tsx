import { fireEvent, render, screen } from "@testing-library/react";
import { memo } from "react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "../../components/common";
import { ScreensProvider, useScreens } from "./ScreensContext";
import type { ScreensContextValue } from "./types";

function screensSource(setScreenPage = vi.fn()): ScreensContextValue {
	return {
		screens: null,
		bootstrap: null,
		session: null,
		saveScreen: vi.fn(),
		deleteScreen: vi.fn(),
		setScreenPage,
		updateControlDesk: vi.fn(),
		selectControlDesk: vi.fn(),
		removeClient: vi.fn(),
	};
}

describe("ScreensProvider", () => {
	it("does not publish a new context value for action identity churn", () => {
		let renders = 0;
		const first = vi.fn();
		const latest = vi.fn();
		const Consumer = memo(() => {
			renders += 1;
			const screens = useScreens();
			return (
				<Button onClick={() => void screens.setScreenPage("screen-1", 2)}>
					Save
				</Button>
			);
		});
		const source = screensSource(first);
		const view = render(
			<ScreensProvider source={source}>
				<Consumer />
			</ScreensProvider>,
		);
		view.rerender(
			<ScreensProvider source={{ ...source, setScreenPage: latest }}>
				<Consumer />
			</ScreensProvider>,
		);
		expect(renders).toBe(1);
		fireEvent.click(screen.getByRole("button", { name: "Save" }));
		expect(first).not.toHaveBeenCalled();
		expect(latest).toHaveBeenCalledWith("screen-1", 2);
	});
});
