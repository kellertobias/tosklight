import { expect, it } from "vitest";
import { StagePaneCommandQueue } from "./stagePaneCommandQueue";

it("waits only for the same pane's short acknowledgements and recovers after rejection", async () => {
	const queue = new StagePaneCommandQueue();
	let release!: () => void;
	const gate = new Promise<void>((resolve) => { release = resolve; });
	const events: string[] = [];
	const first = queue.run("live", async () => { await gate; events.push("old picture"); throw new Error("closed old RPC"); });
	const rejected = expect(first).rejects.toThrow("closed old RPC");
	const capture = queue.run("live", async () => { events.push("capture"); });
	await queue.run("preload", async () => { events.push("other pane"); });
	expect(events).toEqual(["other pane"]);
	release(); await rejected; await capture;
	expect(events).toEqual(["other pane", "old picture", "capture"]);
});

it("bounds queued operations and releases their capacity after completion", async () => {
	const queue = new StagePaneCommandQueue();
	let release!: () => void;
	const gate = new Promise<void>((resolve) => { release = resolve; });
	const first = queue.run("live", () => gate);
	const accepted = Array.from({ length: 127 }, () => queue.run("live", async () => {}));
	await expect(queue.run("live", async () => {})).rejects.toThrow("queue is full");
	release(); await first; await Promise.all(accepted);
	await queue.run("live", async () => {});
});

it("coalesces only adjacent unsent Pictures, preserving input and capture barriers", async () => {
	const queue = new StagePaneCommandQueue();
	let release!: () => void;
	const gate = new Promise<void>((resolve) => { release = resolve; });
	const events: string[] = [];
	const first = queue.run("live", () => gate);
	const old = queue.picture("live", async () => { events.push("old"); });
	const current = queue.picture("live", async () => { events.push("current"); });
	const input = queue.run("live", async () => { events.push("input"); });
	const capture = queue.run("live", async () => { events.push("capture"); });
	const later = queue.picture("live", async () => { events.push("later"); });
	release(); await Promise.all([first, old, current, input, capture, later]);
	expect(events).toEqual(["current", "input", "capture", "later"]);
});

it("always admits one lifecycle close at capacity, then permits awaited reopen", async () => {
	const queue = new StagePaneCommandQueue();
	let release!: () => void;
	const gate = new Promise<void>((resolve) => { release = resolve; });
	const events: string[] = [];
	const ordinary = [queue.run("live", () => gate), ...Array.from({ length: 127 }, () => queue.run("live", async () => {}))];
	const picture = queue.picture("live", async () => { events.push("latest picture"); });
	const close = queue.close("live", async () => { events.push("close"); });
	expect(queue.close("live", async () => { events.push("duplicate close"); })).toBe(close);
	release(); await Promise.all([...ordinary, picture, close]);
	await queue.run("live", async () => { events.push("reopen"); });
	expect(events).toEqual(["latest picture", "close", "reopen"]);
});
