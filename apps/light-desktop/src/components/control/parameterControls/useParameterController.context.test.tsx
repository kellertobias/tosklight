// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useParameterController } from "./useParameterController";
import type { EncoderOwnerContext } from "./useEncoderNavigation";

const mocks = vi.hoisted(() => ({
	desk: "controller-context-1",
	editor: { session: null as object | null },
	bindings: [] as boolean[],
	projection: { encoderGroups: [], encoderPage: 1, encoderSlots: [], encoderPushTurnSlots: [], programmerValues: [], groupProgrammerValues: [], dynamicProgrammerValues: [], selectedFixtureIds: [], selectedFixtures: [], normalized: new Map(), normalizedByFixture: new Map(), discrete: new Map(), discreteByFixture: new Map(), programmerActions: null },
}));
vi.mock("../../../state/AppContext", () => ({ useApp: () => ({ dispatch: vi.fn() }) }));
vi.mock("../../../features/deskSnapshot/DeskSnapshotState", () => ({ useSessionSnapshot: () => ({ desk: { id: mocks.desk } }) }));
vi.mock("../../../features/dynamics/DynamicEditorSessionContext", () => ({ useDynamicEditorSession: () => mocks.editor }));
vi.mock("../../../features/programmingInteraction/ProgrammingInteractionView", () => ({ useProgrammingAlignmentView: () => null }));
vi.mock("./useParameterProjection", () => ({ useParameterProjection: (_family: string, _page: number, active: boolean) => ({ ...mocks.projection, active }) }));
vi.mock("./useParameterValueActions", () => ({ useParameterValueActions: () => ({ canWriteValues: true }) }));
vi.mock("./familyEncoders/useFamilyEncoderBinding", () => ({ useFamilyEncoderBinding: () => ({ overrides: {}, active: false, selectPage: vi.fn(), detent: vi.fn() }) }));
vi.mock("./useHardwareParameterEncoders", () => ({ useHardwareParameterEncoders: (projection: { active: boolean }) => mocks.bindings.push(projection.active) }));

beforeEach(() => { mocks.desk = `controller-context-${Math.random()}`; mocks.editor.session = null; mocks.bindings = []; });
afterEach(cleanup);

describe("shared encoder owner context", () => {
	it("shares Dynamics with a nonowner and exits it from the mirror family controls", () => {
		const owner = renderHook(() => useParameterController(true));
		const mirror = renderHook(() => useParameterController(true, { bindHardware: false }));
		act(() => owner.result.current.setDynamicsMode(true));
		expect(mirror.result.current.dynamicsMode).toBe(true);
		act(() => mirror.result.current.selectEncoderGroup("Color", 1));
		expect(owner.result.current.dynamicsMode).toBe(false);
		expect(owner.result.current.family).toBe("Color");
	});

	it("uses the owner's special context even when the mirror workspace has a different context", () => {
		const owner = renderHook(({ context }) => useParameterController(context === "programmer", { ownerContext: context }), { initialProps: { context: "stage" as EncoderOwnerContext } });
		const mirror = renderHook(() => useParameterController(true, { bindHardware: false, ownerContext: "patch" }));
		expect(mirror.result.current.encoderContext).toBe("stage");
		owner.rerender({ context: "programmer" });
		expect(mirror.result.current.encoderContext).toBe("programmer");
	});

	it("suppresses normal hardware detents during a live Dynamics editor and publishes its context", () => {
		const owner = renderHook(() => useParameterController(true));
		const mirror = renderHook(() => useParameterController(true, { bindHardware: false }));
		mocks.editor.session = { dynamicId: "dynamic-1" };
		mocks.bindings = [];
		owner.rerender();
		expect(owner.result.current.encoderContext).toBe("dynamic-editor");
		expect(mirror.result.current.encoderContext).toBe("dynamic-editor");
		expect(mocks.bindings.length).toBeGreaterThan(0);
		expect(mocks.bindings.every((active) => active === false)).toBe(true);
		mocks.editor.session = null;
		owner.rerender();
		expect(mirror.result.current.encoderContext).toBe("programmer");
	});
});
