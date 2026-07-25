import { cleanup, render } from "@testing-library/react";
import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DeskLockState } from "../../api/types";
import { DeskLockStateProvider, useDeskLock, useDeskLocked } from "./DeskLockState";
import { DeskLockStore } from "./store";

function lockState(overrides: Partial<DeskLockState> = {}): DeskLockState {
	return {
		locked: false,
		message: "Desk locked",
		wallpaper: null,
		unlock_mode: "button",
		...overrides,
	};
}

describe("scoped desk lock", () => {
	afterEach(cleanup);

	it("publishes nothing for an unchanged poll result", () => {
		const store = new DeskLockStore();
		store.install(lockState());
		const listener = vi.fn();
		store.subscribe(listener);

		// The desk lock is polled twice per second; an equivalent result must not wake subscribers.
		store.install(lockState());
		store.install(lockState());

		expect(listener).not.toHaveBeenCalled();
	});

	it("does not rerender a reader across equivalent polls", () => {
		const store = new DeskLockStore();
		store.install(lockState());
		let renders = 0;
		function Reader() {
			renders += 1;
			useDeskLock();
			return null;
		}
		render(
			<DeskLockStateProvider store={store}>
				<Reader />
			</DeskLockStateProvider>,
		);
		expect(renders).toBe(1);

		act(() => store.install(lockState()));

		expect(renders).toBe(1);
	});

	it("rerenders a reader when the lock genuinely changes", () => {
		const store = new DeskLockStore();
		store.install(lockState());
		let renders = 0;
		const observed: { current: boolean } = { current: false };
		function Reader() {
			renders += 1;
			observed.current = useDeskLocked();
			return null;
		}
		render(
			<DeskLockStateProvider store={store}>
				<Reader />
			</DeskLockStateProvider>,
		);

		act(() => store.install(lockState({ locked: true })));

		expect(renders).toBe(2);
		expect(observed.current).toBe(true);
	});

	it("reports an unknown desk lock outside a mounted boundary", () => {
		const observed: { lock: DeskLockState | null; locked: boolean } = {
			lock: lockState(),
			locked: true,
		};
		function Reader() {
			observed.lock = useDeskLock();
			observed.locked = useDeskLocked();
			return null;
		}
		render(<Reader />);

		expect(observed.lock).toBeNull();
		expect(observed.locked).toBe(false);
	});
});
