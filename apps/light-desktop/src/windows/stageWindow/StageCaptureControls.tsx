import { ModalFrame, useModalKeyboardOwnership } from "@tosklight/ui/modals";
import { isStageCaptureShortcut } from "./stageCaptureShortcut";
import type { useStageCapture } from "./useStageCapture";
import type { StageStillPhase } from "../../platform/desktop/types";
import "./stageCapture.css";

type Capture = ReturnType<typeof useStageCapture>;
function progress(phase: StageStillPhase | undefined) {
	if (!phase) return "Freezing current scene…";
	if (phase === "queued") return "4K render queued";
	if (phase === "checkingBlender") return "Checking installed Blender…";
	if (phase === "writing") return "Writing 3840 × 2160 image…";
	if (phase === "cancelled") return "4K render cancelled";
	if (typeof phase === "object") {
		if ("rendering" in phase) return phase.rendering.detail;
		if ("cancelling" in phase) return phase.cancelling.detail;
		if ("failed" in phase) return phase.failed.detail;
		if ("saved" in phase) return "4K image saved";
		if ("needsChoice" in phase) return "Frozen scene ready · choose quality";
	}
	return "Rendering 4K image…";
}
export function StageCaptureControls({ capture, paneId, followPreload }: {
	capture: Capture; paneId: string; followPreload?: boolean;
}) {
	const modalOwnsKeyboard = useModalKeyboardOwnership();
	const phase = capture.status?.phase;
	const choice = capture.busy && typeof phase === "object" && "needsChoice" in phase ? phase.needsChoice : null;
	const saved = typeof phase === "object" && "saved" in phase ? phase.saved.path : null;
	const failed = typeof phase === "object" && "failed" in phase;
	return <>
		<div className="stage-capture-controls" role="toolbar" aria-label="Stage image rendering" onKeyDown={(event) => {
			if (!(event.target instanceof HTMLButtonElement) || !isStageCaptureShortcut(event, modalOwnsKeyboard())) return;
			event.preventDefault(); event.stopPropagation(); void capture.start();
		}}>
			<button type="button" disabled={!capture.ready || capture.busy} onClick={() => { void capture.start(); }}
				aria-label={`Render 4K ${followPreload ? "Follow Preload" : "Live"} image`}>Render 4K</button>
			{!capture.available && <span>4K rendering requires native Stage.</span>}
			{(capture.busy || capture.status) && <span role={failed ? "alert" : "status"} aria-live="polite">{progress(phase)}</span>}
			{capture.busy && <button type="button" disabled={!capture.canCancel} onClick={() => { void capture.cancel(); }}>Cancel render</button>}
			{capture.status?.notice && <span role="status">{capture.status.notice}</span>}
			{capture.error && <span role="alert">{capture.error}</span>}
			{saved && <><code className="stage-capture-path">{saved}</code>
				{capture.canOpenSaved && <><button type="button" onClick={() => { void capture.openSaved(false); }}>Open image</button>
					<button type="button" onClick={() => { void capture.openSaved(true); }}>Reveal image</button></>}
			</>}
		</div>
		{choice && <ModalFrame id={`stage-capture-${paneId}`} ariaLabel="Render 4K quality" title="Render 4K · Choose quality"
			dialogClassName="stage-capture-dialog" closeLabel="Cancel render" closeDisabled={capture.commandPending}
			policy={{ backdrop: false, escape: !capture.commandPending, explicit: !capture.commandPending }} onClose={() => { void capture.cancel(); }}>
			<div className="stage-capture-dialog-content">
				<p>{capture.status?.followPreload ? "Follow Preload" : "Live"} scene frozen · 3840 × 2160. Live quality remains unchanged.</p>
				<p>Installed Blender: {choice.blenderVersion}</p>
				<p><strong>Ultra quality · ToskLight</strong><br />{choice.ultraExplanation}</p>
				<p><strong>Ultimate quality · Blender</strong><br />{choice.ultimateExplanation}</p>
				{choice.ultimateReasons.length > 0 && <div role="status"><strong>Ultimate is unavailable for this frozen scene:</strong>
					<ul>{choice.ultimateReasons.map((reason, index) => <li key={`${index}-${reason}`}>{reason}</li>)}</ul></div>}
				{capture.error && <p role="alert">{capture.error}</p>}
				<div className="stage-capture-dialog-actions">
					<button type="button" data-modal-initial-focus disabled={capture.commandPending} onClick={() => { void capture.choose("ultra"); }}>Ultra · ToskLight</button>
					<button type="button" disabled={capture.commandPending || choice.ultimateReasons.length > 0} onClick={() => { void capture.choose("ultimate"); }}>Ultimate · Blender</button>
					<button type="button" disabled={capture.commandPending} onClick={() => { void capture.cancel(); }}>Cancel render</button>
				</div>
			</div>
		</ModalFrame>}
	</>;
}
