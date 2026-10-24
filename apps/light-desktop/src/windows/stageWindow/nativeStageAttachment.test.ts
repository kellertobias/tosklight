import { describe, expect, it, vi } from "vitest";
import { NativeStageAttachment } from "./nativeStageAttachment";
const geometry = { x: 0, y: 0, width: 800, height: 600, scale: 2, surfaceWidth: 1600, surfaceHeight: 1200 };
function serial() {
	let tail = Promise.resolve();
	return (work: () => Promise<void>) => {
		const next = tail.then(work);
		tail = next.catch(() => undefined);
		return next;
	};
}
function bridge() {
	return {
		openStagePane: vi.fn(async (_id: string, _live3d: boolean, _geometry: unknown) => {}),
		closeStagePane: vi.fn(async (_id: string) => {}),
		setStagePane: vi.fn(async (_id: string, _geometry: unknown) => {}),
		setStagePaneVisibility: vi.fn(async (_id: string, _visible: boolean) => {}),
	};
}

describe("serialized native Stage attachment", () => {
	it("coalesces visibility while a transition is pending and applies geometry before reveal", async () => {
		const desktop = bridge();
		const calls: string[] = [];
		let hidden!: () => void;
		desktop.setStagePaneVisibility.mockImplementation(async (_id, visible) => {
			calls.push(`visibility:${visible}`);
			if (!visible) await new Promise<void>((resolve) => { hidden = resolve; });
		});
		desktop.setStagePane.mockImplementation(async () => { calls.push("geometry"); });
		const attachment = new NativeStageAttachment(desktop, "pane", true, serial(), vi.fn(), vi.fn());
		attachment.update(geometry, true);
		await vi.waitFor(() => expect(calls).toEqual(["visibility:true"]));
		attachment.update(geometry, false);
		await vi.waitFor(() => expect(calls).toEqual(["visibility:true", "visibility:false"]));
		attachment.update({ ...geometry, width: 900 }, false);
		attachment.update({ ...geometry, width: 950 }, true);
		attachment.update({ ...geometry, width: 950 }, true);
		hidden();
		await vi.waitFor(() => expect(calls).toEqual(["visibility:true", "visibility:false", "geometry", "visibility:true"]));
		expect(desktop.setStagePane).toHaveBeenCalledOnce();
		expect(desktop.setStagePane).toHaveBeenCalledWith("pane", { ...geometry, width: 950 });
		attachment.dispose();
	});

	it("finishes an old generation's close before opening its replacement", async () => {
		const desktop = bridge();
		const calls: string[] = [];
		let finishFirst!: () => void;
		desktop.openStagePane.mockImplementation(async () => {
			calls.push("open");
			if (calls.length === 1) await new Promise<void>((resolve) => { finishFirst = resolve; });
		});
		desktop.closeStagePane.mockImplementation(async () => { calls.push("close"); });
		const serialize = serial();
		const staleAttached = vi.fn();
		const old = new NativeStageAttachment(desktop, "same-pane", true, serialize, staleAttached, vi.fn());
		old.update(geometry, true);
		await vi.waitFor(() => expect(calls).toEqual(["open"]));
		old.dispose();
		const currentAttached = vi.fn();
		const next = new NativeStageAttachment(desktop, "same-pane", true, serialize, currentAttached, vi.fn());
		next.update(geometry, false);
		finishFirst();
		await vi.waitFor(() => expect(currentAttached).toHaveBeenCalledOnce());
		expect(calls).toEqual(["open", "close", "open"]);
		expect(staleAttached).not.toHaveBeenCalled();
		expect(desktop.setStagePaneVisibility).toHaveBeenCalledOnce();
		expect(desktop.setStagePaneVisibility).toHaveBeenCalledWith("same-pane", false);
		next.dispose();
	});

	it("does not allocate collapsed geometry or resize an attached picture to zero", async () => {
		const desktop = bridge();
		const attachment = new NativeStageAttachment(desktop, "pane", true, serial(), vi.fn(), vi.fn());
		attachment.update(null, false);
		await Promise.resolve();
		expect(desktop.openStagePane).not.toHaveBeenCalled();
		attachment.update(geometry, true);
		await vi.waitFor(() => expect(desktop.setStagePaneVisibility).toHaveBeenCalledWith("pane", true));
		attachment.update(null, false);
		await vi.waitFor(() => expect(desktop.setStagePaneVisibility).toHaveBeenLastCalledWith("pane", false));
		expect(desktop.setStagePane).not.toHaveBeenCalled();
		expect(desktop.closeStagePane).not.toHaveBeenCalled();
		attachment.update(geometry, true);
		await vi.waitFor(() => expect(desktop.setStagePaneVisibility).toHaveBeenLastCalledWith("pane", true));
		expect(desktop.openStagePane).toHaveBeenCalledOnce();
		attachment.dispose();
	});

	it("reports a failed visibility update without a recurring retry loop", async () => {
		const desktop = bridge();
		desktop.setStagePaneVisibility.mockRejectedValue(new Error("native visibility failed"));
		const failed = vi.fn();
		const attachment = new NativeStageAttachment(desktop, "pane", true, serial(), vi.fn(), failed);
		attachment.update(geometry, true);
		await vi.waitFor(() => expect(failed).toHaveBeenCalledOnce());
		attachment.update(geometry, true);
		await Promise.resolve(); await Promise.resolve();
		expect(desktop.setStagePaneVisibility).toHaveBeenCalledOnce();
		attachment.dispose();
	});
});
