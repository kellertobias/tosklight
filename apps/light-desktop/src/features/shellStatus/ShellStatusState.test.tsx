import { cleanup, render } from "@testing-library/react";
import { act } from "react";
import { afterEach, describe, expect, it } from "vitest";
import {
	ShellStatusStateProvider,
	useConnectionStatus,
	useServerError,
} from "./ShellStatusState";
import { ShellStatusStore } from "./store";

describe("scoped shell status", () => {
	afterEach(cleanup);

	it("does not rerender a status reader when only the error changes", () => {
		const store = new ShellStatusStore();
		store.install("connected", null);
		let renders = 0;
		function Reader() {
			renders += 1;
			useConnectionStatus();
			return null;
		}
		render(
			<ShellStatusStateProvider store={store}>
				<Reader />
			</ShellStatusStateProvider>,
		);
		expect(renders).toBe(1);

		act(() => store.install("connected", "Patch write failed"));

		expect(renders).toBe(1);
	});

	it("does not rerender an error reader when only the status changes", () => {
		const store = new ShellStatusStore();
		store.install("connected", "Patch write failed");
		let renders = 0;
		function Reader() {
			renders += 1;
			useServerError();
			return null;
		}
		render(
			<ShellStatusStateProvider store={store}>
				<Reader />
			</ShellStatusStateProvider>,
		);
		expect(renders).toBe(1);

		act(() => store.install("connecting", "Patch write failed"));

		expect(renders).toBe(1);
	});

	it("rerenders each reader for its own change", () => {
		const store = new ShellStatusStore();
		store.install("connected", null);
		const observed: { status: string; error: string | null } = {
			status: "",
			error: null,
		};
		function Reader() {
			observed.status = useConnectionStatus();
			observed.error = useServerError();
			return null;
		}
		render(
			<ShellStatusStateProvider store={store}>
				<Reader />
			</ShellStatusStateProvider>,
		);

		act(() => store.install("offline", "Connection lost"));

		expect(observed).toEqual({
			status: "offline",
			error: "Connection lost",
		});
	});

	it("publishes nothing when an equivalent status and error are installed", () => {
		const store = new ShellStatusStore();
		store.install("connected", "Same");
		let renders = 0;
		function Reader() {
			renders += 1;
			useConnectionStatus();
			useServerError();
			return null;
		}
		render(
			<ShellStatusStateProvider store={store}>
				<Reader />
			</ShellStatusStateProvider>,
		);

		act(() => store.install("connected", "Same"));

		expect(renders).toBe(1);
	});

	it("reports the initial status outside a mounted boundary", () => {
		const observed: { status: string; error: string | null } = {
			status: "",
			error: "x",
		};
		function Reader() {
			observed.status = useConnectionStatus();
			observed.error = useServerError();
			return null;
		}
		render(<Reader />);

		expect(observed).toEqual({ status: "connecting", error: null });
	});
});
