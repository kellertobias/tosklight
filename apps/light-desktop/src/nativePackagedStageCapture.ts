import { useEffect, useRef } from "react";
import type { DesktopBridge, StageStillEvent, StageStillStatus, PackagedStageBenchmarkConfig } from "./platform/desktop/types";

import type { NativeStageReadyEvidence } from "./nativePackagedStageReadiness";

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
const terminal = (status: StageStillStatus | null) => Boolean(status && (status.phase === "cancelled" ||
    (typeof status.phase === "object" && ("saved" in status.phase || "failed" in status.phase))));

/** One explicit packaged verification run. All actions still originate in the actual DOM controls. */
export class PackagedStageCaptureExercise {
    readonly bridge: DesktopBridge;
    private request: { paneId: string; requestId: string } | null = null;
    private latest: StageStillStatus | null = null;
    private frozenIdentity: string | null = null;
    private fault: string | null = null;
    private observations: StageStillStatus[] = [];
    private ready: NativeStageReadyEvidence | null = null;
    private choices = 0;
    private stopped = false;
    private running: Promise<Record<string, unknown>> | null = null;

    constructor(private readonly desktop: DesktopBridge, private readonly timeoutMs = 180_000) {
        this.bridge = { ...desktop,
            startStagePaneCapture: async (paneId, requestId) => {
                if (this.ready?.lanes[0].frame.paneId !== `main:${paneId}`) throw new Error("Capture DOM pane does not own the ready Live frame");
                if (this.request) throw new Error("Capture exercise attempted more than one start");
                this.request = { paneId, requestId };
                try { await desktop.startStagePaneCapture!(paneId, requestId); }
                catch (error) { this.request = null; this.fault = String(error); throw error; }
            },
            chooseStagePaneCapture: async (paneId, jobId, engine) => {
                if (paneId !== this.request?.paneId || jobId !== this.latest?.jobId || engine !== "ultra" || this.choices++)
                    throw new Error("Capture exercise received an unexpected engine choice");
                try { await desktop.chooseStagePaneCapture!(paneId, jobId, engine); }
                catch (error) { this.fault = String(error); throw error; }
            },
        };
    }

    acceptReady(evidence: NativeStageReadyEvidence, config: PackagedStageBenchmarkConfig) {
        const frame = evidence.lanes[0].frame;
        if (frame.followPreload || frame.degraded || frame.width !== 1920 || frame.height !== 540 ||
            frame.quality !== (config.baselineQuality ?? "draft") || frame.mode !== (config.baselineView ?? "lines_3d")) {
            this.fault = "Ready Live frame does not match the explicit capture benchmark picture"; return;
        }
        this.ready = evidence;
    }

    /** Idempotent across React effect replay; no second job can be started. */
    run(): Promise<Record<string, unknown>> {
        return this.running ??= this.execute();
    }
    stop() { this.stopped = true; }

    private observe(event: StageStillEvent) {
        const request = this.request;
        if (!request || event.paneId !== request.paneId || event.requestId !== request.requestId) return;
        if (event.error) { this.fault = event.error; return; }
        const next = event.status;
        if (!next || next.paneId !== request.paneId || next.requestId !== request.requestId || next.followPreload ||
            next.width !== 3840 || next.height !== 2160 || (this.latest && this.latest.jobId !== next.jobId)) {
            this.fault = "Capture status has foreign or invalid identity"; return;
        }
        const identity = JSON.stringify([next.sceneRevision, next.valuesFrame, next.timeSeconds, next.framing]);
        if (this.frozenIdentity !== null && this.frozenIdentity !== identity) {
            this.fault = "Frozen capture identity changed during the job"; return;
        }
        this.frozenIdentity = identity;
        this.latest = next;
        if (this.observations.length >= 64) { this.fault = "Capture observation limit exceeded"; return; }
        this.observations.push(next);
    }

    private async wait<T>(read: () => T | null | undefined | false, deadline: number): Promise<T> {
        for (;;) {
            if (this.fault || this.stopped) throw new Error(this.fault ?? "Capture exercise stopped");
            const value = read();
            if (value) return value;
            if (Date.now() >= deadline) throw new Error("Packaged capture exercise timed out");
            await sleep(50);
        }
    }

    private async cancelOwned() {
        // A start can time out before its first acknowledgement; wait briefly for its owned job.
        const deadline = Date.now() + 35_000;
        while (this.request && !this.latest && Date.now() < deadline) await sleep(50);
        if (!this.request || !this.latest || terminal(this.latest)) return;
        await this.desktop.cancelStagePaneCapture?.(this.request.paneId, this.latest.jobId);
        while (!terminal(this.latest) && Date.now() < deadline) await sleep(50);
        if (!terminal(this.latest)) throw new Error("Owned capture cancellation did not reach a terminal phase");
    }

    private async execute(): Promise<Record<string, unknown>> {
        const failures: string[] = [];
        let unsubscribe: (() => void) | undefined;
        let buttonClicked = false, chooserClicked = false, savedVisible = false;
        try {
            if (!this.desktop.onStageStill || !this.desktop.startStagePaneCapture ||
                !this.desktop.chooseStagePaneCapture || !this.desktop.cancelStagePaneCapture)
                throw new Error("Native capture bridge is incomplete");
            unsubscribe = await this.desktop.onStageStill((event) => this.observe(event));
            const deadline = Date.now() + this.timeoutMs;
            await this.wait(() => this.ready, deadline);
            const button = await this.wait(() => {
                const candidates = document.querySelectorAll<HTMLButtonElement>('button[aria-label="Render 4K Live image"]');
                return candidates.length === 1 && !candidates[0].disabled &&
                    candidates[0].closest('[data-stage-renderer="native"]') ? candidates[0] : null;
            }, deadline);
            button.click(); buttonClicked = true;
            await this.wait(() => {
                if (terminal(this.latest)) throw new Error("Capture completed without the required quality chooser");
                const dialog = document.querySelector<HTMLElement>('[role="dialog"][aria-label="Render 4K quality"]');
                const buttons = [...(dialog?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
                return this.latest && typeof this.latest.phase === "object" && "needsChoice" in this.latest.phase &&
                    buttons.find((candidate) => candidate.textContent?.trim() === "Ultra · ToskLight" && !candidate.disabled);
            }, deadline).then((choice) => { choice.click(); chooserClicked = true; });
            const saved = await this.wait(() => terminal(this.latest) ? this.latest : null, deadline);
            if (typeof saved.phase !== "object" || !("saved" in saved.phase) || saved.engine !== "ultra" || this.choices !== 1)
                throw new Error(`Capture did not save the selected Ultra job: ${JSON.stringify(saved.phase)}`);
            const path = saved.phase.saved.path;
            await this.wait(() => {
                const lane = document.querySelector('[data-visualization-lane="live"]');
                const texts = [...(lane?.querySelectorAll("button") ?? [])].map((candidate) => candidate.textContent?.trim());
                return lane?.textContent?.includes("4K image saved") && lane.textContent.includes(path) &&
                    texts.includes("Open image") && texts.includes("Reveal image");
            }, deadline);
            savedVisible = true;
        } catch (error) {
            failures.push(String(error));
            try { await this.cancelOwned(); } catch (cancelError) { failures.push(String(cancelError)); }
        } finally { unsubscribe?.(); }
        return { schemaVersion: 1, kind: "packaged-stage-capture-complete", passed: failures.length === 0,
            failures, request: this.request, status: this.latest, observations: this.observations, ready: this.ready,
            ui: { buttonClicked, chooserClicked, savedVisible, openedOrRevealed: false },
            boundary: "Programmatic real DOM controls through native capture; no physical pointer/shortcut, fidelity or displayed-FPS acceptance" };
    }
}

export function usePackagedStageCaptureExercise(exercise: PackagedStageCaptureExercise | null,
    enabled: boolean, desktop: DesktopBridge, finish: () => void) {
    const delayedStop = useRef<ReturnType<typeof setTimeout> | null>(null);
    const finished = useRef(finish);
    finished.current = finish;
    useEffect(() => {
        if (delayedStop.current !== null) clearTimeout(delayedStop.current);
        if (!enabled || !exercise) return;
        let disposed = false;
        void exercise.run().then(async (result) => {
            if (disposed) return;
            await desktop.appendPackagedStageBenchmarkSample(result);
            finished.current();
        }).catch((error) => { console.error("Packaged capture evidence failed", error); });
        return () => {
            disposed = true;
            // React StrictMode immediately reattaches; a genuine unmount cancels owned work.
            delayedStop.current = setTimeout(() => exercise.stop(), 0);
        };
    }, [exercise, desktop, enabled]);
}
