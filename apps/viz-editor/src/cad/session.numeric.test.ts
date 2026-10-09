import { expect, it, vi } from "vitest";
import { cadSession } from "./session";
const invoke = vi.hoisted(() =>
	vi.fn().mockResolvedValue({ sceneRevision: 10 }),
);
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
it("sends exact numeric poses and expected revision to the registered native command", async () => {
	const transforms = [
		{
			id: "physical-copy",
			positionMillimetres: [1, 2, 3] as [number, number, number],
			rotationDegrees: [0, 0, 30] as [number, number, number],
		},
	];
	expect(await cadSession.setNumericTransforms(9, transforms)).toEqual({
		sceneRevision: 10,
	});
	expect(invoke).toHaveBeenCalledWith("cad_set_numeric_transforms", {
		expectedSceneRevision: 9,
		transforms,
	});
});
