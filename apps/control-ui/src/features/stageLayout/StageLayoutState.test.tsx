import { cleanup, render } from "@testing-library/react";
import { act } from "react";
import { afterEach, describe, expect, it } from "vitest";
import {
	StageLayoutStateProvider,
	useStageLayoutRevision,
	useStagePositions,
	useStagePositions3d,
} from "./StageLayoutState";
import { type StageLayoutObject, StageLayoutStore } from "./store";

function layout(
	revision: number,
	positions: Record<string, { x: number; y: number; rotation: number }>,
	positions3d: Record<string, { x: number; y: number; z: number }> = {},
): StageLayoutObject {
	return {
		kind: "stage_layout",
		id: "main",
		revision,
		updated_at: "",
		body: { version: 2, positions, positions3d },
	} as StageLayoutObject;
}

describe("scoped stage layout", () => {
	afterEach(cleanup);

	it("does not rerender a 2d reader when only 3d positions change", () => {
		const store = new StageLayoutStore();
		const positions = { "fixture-a": { x: 1, y: 2, rotation: 0 } };
		store.install(layout(1, positions));
		let renders = 0;
		function Reader() {
			renders += 1;
			useStagePositions();
			return null;
		}
		render(
			<StageLayoutStateProvider store={store}>
				<Reader />
			</StageLayoutStateProvider>,
		);
		expect(renders).toBe(1);

		// A new stored object that reuses the same positions object must not rerender the reader.
		act(() =>
			store.install(layout(2, positions, { "fixture-a": { x: 9, y: 9, z: 9 } })),
		);

		expect(renders).toBe(1);
	});

	it("rerenders a 3d reader when its own positions change", () => {
		const store = new StageLayoutStore();
		store.install(layout(1, {}));
		let renders = 0;
		const observed: { current: Record<string, unknown> } = { current: {} };
		function Reader() {
			renders += 1;
			observed.current = useStagePositions3d();
			return null;
		}
		render(
			<StageLayoutStateProvider store={store}>
				<Reader />
			</StageLayoutStateProvider>,
		);

		act(() => store.install(layout(2, {}, { "fixture-a": { x: 1, y: 2, z: 3 } })));

		expect(renders).toBe(2);
		expect(observed.current["fixture-a"]).toEqual({ x: 1, y: 2, z: 3 });
	});

	it("exposes the stored revision a write must be made against", () => {
		const store = new StageLayoutStore();
		store.install(layout(7, {}));
		const observed: { current: number } = { current: -1 };
		function Reader() {
			observed.current = useStageLayoutRevision();
			return null;
		}
		render(
			<StageLayoutStateProvider store={store}>
				<Reader />
			</StageLayoutStateProvider>,
		);

		expect(observed.current).toBe(7);
	});

	it("reports empty positions and revision zero outside a mounted boundary", () => {
		const observed: { positions: unknown; revision: number } = {
			positions: null,
			revision: -1,
		};
		function Reader() {
			observed.positions = useStagePositions();
			observed.revision = useStageLayoutRevision();
			return null;
		}
		render(<Reader />);

		expect(observed.positions).toEqual({});
		expect(observed.revision).toBe(0);
	});
});
