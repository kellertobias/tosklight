import { capturePicture } from "./windows/stageWindow/stageCaptureTestFixture";
// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ModalProvider } from "@tosklight/ui/modals";
import { DesktopProvider } from "./platform/desktop";
import { browserDesktopBridge } from "./platform/desktop/browserDesktopBridge";
import type { DesktopBridge, StageStillEvent, StageStillPhase, StageStillStatus, StagePaneBenchmarkSample, PackagedStageBenchmarkConfig } from "./platform/desktop/types";
import type { NativeStageReadyEvidence } from "./nativePackagedStageReadiness";
import { PackagedStageCaptureExercise } from "./nativePackagedStageCapture";
import { StageCaptureControls } from "./windows/stageWindow/StageCaptureControls";
import { useStageCapture } from "./windows/stageWindow/useStageCapture";

function Controls() {
    const capture = useStageCapture("live-pane", true, capturePicture());
    return <div data-visualization-lane="live"><div data-stage-renderer="native">
        <StageCaptureControls capture={capture} paneId="live-pane" />
    </div></div>;
}
function setup(onChoice?: (emit: (phase: StageStillPhase, patch?: Partial<StageStillStatus>) => void) => void, ready = true) {
    const listeners = new Set<(event: StageStillEvent) => void>();
    let requestId = "";
    const emit = (phase: StageStillPhase, patch: Partial<StageStillStatus> = {}) => act(() => {
        const status: StageStillStatus = { paneId: "live-pane", requestId, jobId: "owned-job", followPreload: false,
            sceneRevision: 8, valuesFrame: 20, timeSeconds: 2.5, width: 3840, height: 2160,
            framing: "frozen-camera", engine: null, notice: null, phase, ...patch };
        for (const listener of listeners) listener({ paneId: "live-pane", requestId: status.requestId!, status, error: null });
    });
    const start = vi.fn(async (_pane: string, request: string) => {
        requestId = request;
        emit("queued");
        emit({ needsChoice: { blenderVersion: "actual-path-test", ultimateReasons: [], ultraExplanation: "Native", ultimateExplanation: "Cycles" } });
    });
    const choose = vi.fn(async () => {
        if (onChoice) onChoice(emit);
        else emit({ saved: { path: "/isolated/Stage-owned-job.png" } }, { engine: "ultra" });
    });
    const cancel = vi.fn(async () => { emit("cancelled"); });
    const open = vi.fn(async () => {});
    const desktop: DesktopBridge = { ...browserDesktopBridge, available: true, startStagePaneCapture: start,
        chooseStagePaneCapture: choose, cancelStagePaneCapture: cancel, openStagePaneCapture: open,
        onStageStill: async (listener) => { listeners.add(listener); return () => { listeners.delete(listener); }; } };
    const exercise = new PackagedStageCaptureExercise(desktop, 500);
    const frame = { paneId: "main:live-pane", attachmentId: "owned-attachment", followPreload: false,
        width: 1920, height: 540, quality: "draft", mode: "lines_3d", degraded: false } as StagePaneBenchmarkSample;
    const evidence = { lanes: [{ frame }] } as unknown as NativeStageReadyEvidence;
    const config = { baselineQuality: "draft", baselineView: "lines_3d" } as PackagedStageBenchmarkConfig;
    if (ready) exercise.acceptReady(evidence, config);
    render(<DesktopProvider bridge={exercise.bridge}><ModalProvider><Controls /></ModalProvider></DesktopProvider>);
    return { exercise, start, choose, cancel, open, emit, listeners, evidence, config };
}
afterEach(cleanup);

it("clicks actual capture controls once, follows native choice and visible saved result without opening it", async () => {
    const harness = setup();
    const run = harness.exercise.run();
    expect(harness.exercise.run()).toBe(run);
    const result = await run;
    expect(result.passed).toBe(true);
    expect(result.ui).toEqual({ buttonClicked: true, chooserClicked: true, savedVisible: true, openedOrRevealed: false });
    expect(harness.start).toHaveBeenCalledTimes(1);
    expect(harness.choose).toHaveBeenCalledWith("live-pane", "owned-job", "ultra");
    expect(harness.open).not.toHaveBeenCalled();
    expect(harness.listeners.size).toBe(1); // Only the real control's subscription remains.
});

it("rejects changed frozen identity and cancels only its acknowledged job", async () => {
    const harness = setup((emit) => { emit({ rendering: { detail: "render" } }, { timeSeconds: 3 }); });
    const result = await harness.exercise.run();
    expect(result.passed).toBe(false);
    expect(String(result.failures)).toContain("Frozen capture identity changed");
    expect(harness.cancel).toHaveBeenCalledWith("live-pane", "owned-job");
});

it("times out boundedly and waits for owned cancellation", async () => {
    const harness = setup((emit) => { emit({ rendering: { detail: "render" } }, { engine: "ultra" }); });
    const result = await harness.exercise.run();
    expect(result.passed).toBe(false);
    expect(String(result.failures)).toContain("timed out");
    expect(harness.cancel).toHaveBeenCalledTimes(1);
});

it("never counts automatic fallback as quality chooser coverage", async () => {
    const harness = setup();
    harness.start.mockImplementation(async (_pane, requestId) => {
        harness.emit({ saved: { path: "/fallback.png" } }, { engine: "ultra", requestId });
    });
    const result = await harness.exercise.run();
    expect(result.passed).toBe(false);
    expect(harness.choose).not.toHaveBeenCalled();
});

it("stops an in-flight exercise through owned cancellation", async () => {
    const harness = setup((emit) => { emit({ rendering: { detail: "render" } }, { engine: "ultra" }); });
    const run = harness.exercise.run();
    await waitFor(() => expect(harness.choose).toHaveBeenCalledTimes(1));
    harness.exercise.stop();
    expect((await run).passed).toBe(false);
    expect(harness.cancel).toHaveBeenCalledWith("live-pane", "owned-job");
});

it("waits for actual native readiness even when the DOM control is already enabled", async () => {
    const harness = setup(undefined, false);
    const run = harness.exercise.run();
    await new Promise((resolve) => setTimeout(resolve, 75));
    expect(harness.start).not.toHaveBeenCalled();
    harness.exercise.acceptReady(harness.evidence, harness.config);
    expect((await run).passed).toBe(true);
});

it("rejects a readiness witness for the wrong requested picture before starting any job", async () => {
    const harness = setup(undefined, false);
    harness.exercise.acceptReady(harness.evidence, { ...harness.config, baselineView: "full_3d" });
    expect((await harness.exercise.run()).passed).toBe(false);
    expect(harness.start).not.toHaveBeenCalled();
    expect(harness.cancel).not.toHaveBeenCalled();
});
