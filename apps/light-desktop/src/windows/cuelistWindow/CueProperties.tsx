import { Button, ModalPortal, ModalTitleBar } from "@tosklight/ui";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { Cue } from "../../api/types";
import { useControlSurfaceTarget } from "../../features/controlSurfaceInteraction/useControlSurfaceTarget";
import {
	type CueDraftActions,
	type CueKeyboardField,
	CuePropertyFields,
} from "./CuePropertyFields";
import { cueTrigger, cueTriggerKind, formatCueSeconds } from "./cueFormatting";

function useCuePropertiesLayout(active: boolean, dependencies: unknown[]) {
	const propertiesRef = useRef<HTMLElement>(null);
	const previewRef = useRef<HTMLElement>(null);
	const gridRef = useRef<HTMLDivElement>(null);
	const [fieldsFit, setFieldsFit] = useState(true);
	const [setArmed, setSetArmed] = useState(false);
	useLayoutEffect(() => {
		const aside = propertiesRef.current;
		const preview = previewRef.current;
		const fields = gridRef.current;
		if (!aside || !preview || !fields) return;
		const measure = () => {
			if (aside.clientHeight <= 0 || fields.scrollHeight <= 0) return;
			const style = getComputedStyle(aside);
			const available =
				aside.clientHeight -
				Number.parseFloat(style.paddingTop || "0") -
				Number.parseFloat(style.paddingBottom || "0");
			const gap = Number.parseFloat(style.rowGap || style.gap || "0");
			setFieldsFit(
				preview.offsetHeight + gap + fields.scrollHeight <= available + 1,
			);
		};
		if (typeof ResizeObserver === "undefined") {
			measure();
			return;
		}
		const observer = new ResizeObserver(measure);
		observer.observe(aside);
		observer.observe(preview);
		observer.observe(fields);
		measure();
		return () => observer.disconnect();
	}, dependencies);
	useEffect(() => {
		if (fieldsFit) setSetArmed(false);
	}, [fieldsFit]);
	useControlSurfaceTarget(
		!fieldsFit && active
			? {
					id: "compact-cue-properties",
					priority: 300,
					accepts: ({ type }) => type === "set",
					handle: () => setSetArmed((armed) => !armed),
				}
			: null,
	);
	return {
		propertiesRef,
		previewRef,
		gridRef,
		fieldsFit,
		setArmed,
		setSetArmed,
	};
}

function CompactCueProperties({
	actions,
	cues,
	setArmed,
	onDisarm,
	onOpenInput,
	onOpenTrigger,
}: {
	actions: CueDraftActions;
	cues: Cue[];
	setArmed: boolean;
	onDisarm: () => void;
	onOpenInput: (field: CueKeyboardField) => void;
	onOpenTrigger: () => void;
}) {
	const kind = cueTriggerKind(actions.draft);
	const triggerMillis = Number(actions.draft.trigger.delay_millis ?? 0);
	return (
		<section
			className="cue-settings-compact-fallback"
			data-set-armed={setArmed || undefined}
		>
			<p>
				{setArmed
					? "SET is active. Press an attribute value to edit it."
					: "Press SET, then press an attribute value to edit it."}
			</p>
			<div>
				<Button
					aria-label="Set Cue Title"
					active={setArmed}
					onClick={() => onOpenInput("title")}
				>
					<small>Title</small>
					<b>{actions.draft.name || "Untitled"}</b>
				</Button>
				<Button
					aria-label="Set Cue Intensity In Fade"
					active={setArmed}
					onClick={() => onOpenInput("fade")}
				>
					<small>In Fade</small>
					<b>{formatCueSeconds(actions.draft.fade_millis)}</b>
				</Button>
				<Button
					aria-label="Set Cue Intensity In Delay"
					active={setArmed}
					onClick={() => onOpenInput("delay")}
				>
					<small>In Delay</small>
					<b>{formatCueSeconds(actions.draft.delay_millis)}</b>
				</Button>
				<Button
					aria-label="Set Cue Intensity Out Fade"
					active={setArmed}
					onClick={() => onOpenInput("outFade")}
				>
					<small>Out Fade</small>
					<b>
						{formatCueSeconds(
							actions.draft.out_fade_millis ?? actions.draft.fade_millis,
						)}
					</b>
				</Button>
				<Button
					aria-label="Set Cue Intensity Out Delay"
					active={setArmed}
					onClick={() => onOpenInput("outDelay")}
				>
					<small>Out Delay</small>
					<b>
						{formatCueSeconds(
							actions.draft.out_delay_millis ?? actions.draft.delay_millis,
						)}
					</b>
				</Button>
				<Button
					aria-label="Set Cue Trigger"
					active={setArmed}
					onClick={() => {
						if (!setArmed) return;
						onDisarm();
						onOpenTrigger();
					}}
				>
					<small>Trigger</small>
					<b>{kind.toUpperCase()}</b>
				</Button>
				{kind === "link" && (
					<Button
						aria-label="Set Cue Link destination"
						active={setArmed}
						onClick={() => {
							if (!setArmed) return;
							onDisarm();
							onOpenTrigger();
						}}
					>
						<small>Link Cue</small>
						<b>
							{cues.find((cue) => cue.id === actions.draft.trigger.cue_id)
								?.number ?? "Missing"}
						</b>
					</Button>
				)}
				{(kind === "time" || kind === "link") && (
					<Button
						aria-label={
							kind === "link" ? "Set Cue Link delay" : "Set Cue Trigger time"
						}
						active={setArmed}
						onClick={() => onOpenInput("triggerTime")}
					>
						<small>{kind === "link" ? "Link delay" : "Trigger time"}</small>
						<b>{formatCueSeconds(triggerMillis)}</b>
					</Button>
				)}
			</div>
		</section>
	);
}

function CueTriggerModal({
	actions,
	cues,
	close,
}: {
	actions: CueDraftActions;
	cues: Cue[];
	close: () => void;
}) {
	const kind = cueTriggerKind(actions.draft);
	const triggerMillis = Number(actions.draft.trigger.delay_millis ?? 0);
	const linkCandidates = cues.filter(
		(cue) => cue.id && cue.id !== actions.draft.id,
	);
	const choose = (value: "go" | "follow" | "time" | "timecode" | "link") => {
		const next = {
			...actions.draft,
			trigger: cueTrigger(
				value,
				triggerMillis,
				String(actions.draft.trigger.cue_id ?? linkCandidates[0]?.id ?? ""),
				Number(actions.draft.trigger.frame ?? 0),
			),
		};
		actions.setDraft(next);
		close();
		void actions.save(next);
	};
	const chooseDestination = (cueId: string) => {
		const next = {
			...actions.draft,
			trigger: cueTrigger("link", triggerMillis, cueId),
		};
		actions.setDraft(next);
		close();
		void actions.save(next);
	};
	return (
		<ModalPortal onClose={close}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && close()
				}
			>
				<section
					className="nested-modal cue-trigger-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Cue Trigger"
				>
					<ModalTitleBar
						title="Cue Trigger"
						closeLabel="Close Cue Trigger"
						onClose={close}
					/>
					<div className="cue-trigger-options">
						{(
							[
								"go",
								"follow",
								"time",
								"timecode",
								...(linkCandidates.length > 0 ? (["link"] as const) : []),
							] as const
						).map((value) => (
							<Button
								key={value}
								active={kind === value}
								onClick={() => choose(value)}
							>
								{value.toUpperCase()}
							</Button>
						))}
					</div>
					{kind === "link" && (
						<div
							className="cue-trigger-options"
							role="group"
							aria-label="Link destination"
						>
							{linkCandidates.map((cue) => (
								<Button
									key={cue.id}
									active={cue.id === actions.draft.trigger.cue_id}
									onClick={() => chooseDestination(cue.id as string)}
								>
									{`Cue ${cue.number}${cue.name ? ` · ${cue.name}` : ""}`}
								</Button>
							))}
						</div>
					)}
				</section>
			</div>
		</ModalPortal>
	);
}

export function CueProperties({
	actions,
	cues,
	thumbnail,
	editError,
	active,
	layoutDependencies,
}: {
	actions: CueDraftActions;
	cues: Cue[];
	thumbnail: string | undefined;
	editError: string;
	active: boolean;
	layoutDependencies: unknown[];
}) {
	const refs = {
		title: useRef<HTMLInputElement>(null),
		fade: useRef<HTMLInputElement>(null),
		delay: useRef<HTMLInputElement>(null),
		outFade: useRef<HTMLInputElement>(null),
		outDelay: useRef<HTMLInputElement>(null),
		triggerTime: useRef<HTMLInputElement>(null),
		triggerPicker: useRef<HTMLDivElement>(null),
		grid: useRef<HTMLDivElement>(null),
	};
	const layout = useCuePropertiesLayout(active, layoutDependencies);
	refs.grid = layout.gridRef;
	const [triggerModalOpen, setTriggerModalOpen] = useState(false);
	const [keyboardRequests, setKeyboardRequests] = useState<
		Partial<Record<CueKeyboardField, number>>
	>({});
	const openInput = (field: CueKeyboardField) => {
		if (!layout.setArmed) return;
		layout.setSetArmed(false);
		setKeyboardRequests((requests) => ({
			...requests,
			[field]: (requests[field] ?? 0) + 1,
		}));
	};
	return (
		<>
			<aside
				ref={layout.propertiesRef}
				className={`sequence-actions cue-properties ${layout.fieldsFit ? "" : "compact-cue-settings"}`.trim()}
			>
				<section ref={layout.previewRef} className="cue-selected-preview">
					{thumbnail && (
						<img
							className="cue-selected-thumbnail"
							src={thumbnail}
							alt={`3D preview for Cue ${actions.draft.number}`}
						/>
					)}
					<b className="cue-selected-label">
						Selected Cue · {actions.draft.number}
					</b>
				</section>
				<CuePropertyFields
					actions={actions}
					cues={cues}
					refs={refs}
					keyboardRequests={keyboardRequests}
				/>
				{!layout.fieldsFit && (
					<CompactCueProperties
						actions={actions}
						cues={cues}
						setArmed={layout.setArmed}
						onDisarm={() => layout.setSetArmed(false)}
						onOpenInput={openInput}
						onOpenTrigger={() => setTriggerModalOpen(true)}
					/>
				)}
				{editError && (
					<p className="ui-field-error" role="alert">
						{editError}
					</p>
				)}
			</aside>
			{triggerModalOpen && (
				<CueTriggerModal
					actions={actions}
					cues={cues}
					close={() => setTriggerModalOpen(false)}
				/>
			)}
		</>
	);
}
