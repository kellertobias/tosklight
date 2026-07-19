import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Button, ModalPortal, ModalTitleBar } from "../../components/common";
import { type CueDraftActions, CuePropertyFields } from "./CuePropertyFields";
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
	useEffect(() => {
		if (fieldsFit || !active) return;
		const handleSet = (event: Event) => {
			if ((event as CustomEvent<string>).detail !== "set") return;
			setSetArmed((armed) => !armed);
		};
		window.addEventListener("light:desk-action", handleSet);
		return () => window.removeEventListener("light:desk-action", handleSet);
	}, [active, fieldsFit]);
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
	setArmed,
	onDisarm,
	onOpenInput,
	onOpenTrigger,
}: {
	actions: CueDraftActions;
	setArmed: boolean;
	onDisarm: () => void;
	onOpenInput: (field: "title" | "fade" | "delay" | "triggerTime") => void;
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
					aria-label="Set Cue Fade"
					active={setArmed}
					onClick={() => onOpenInput("fade")}
				>
					<small>Fade</small>
					<b>{formatCueSeconds(actions.draft.fade_millis)}</b>
				</Button>
				<Button
					aria-label="Set Cue Delay"
					active={setArmed}
					onClick={() => onOpenInput("delay")}
				>
					<small>Delay</small>
					<b>{formatCueSeconds(actions.draft.delay_millis)}</b>
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
				{kind === "time" && (
					<Button
						aria-label="Set Cue Trigger time"
						active={setArmed}
						onClick={() => onOpenInput("triggerTime")}
					>
						<small>Trigger time</small>
						<b>{formatCueSeconds(triggerMillis)}</b>
					</Button>
				)}
			</div>
		</section>
	);
}

function CueTriggerModal({
	actions,
	close,
}: {
	actions: CueDraftActions;
	close: () => void;
}) {
	const kind = cueTriggerKind(actions.draft);
	const triggerMillis = Number(actions.draft.trigger.delay_millis ?? 0);
	const choose = (value: "go" | "follow" | "time") => {
		const next = {
			...actions.draft,
			trigger: cueTrigger(value, triggerMillis),
		};
		actions.setDraft(next);
		close();
		void actions.save(next);
	};
	return (
		<ModalPortal>
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
						{(["go", "follow", "time"] as const).map((value) => (
							<Button
								key={value}
								active={kind === value}
								onClick={() => choose(value)}
							>
								{value.toUpperCase()}
							</Button>
						))}
					</div>
				</section>
			</div>
		</ModalPortal>
	);
}

export function CueProperties({
	actions,
	thumbnail,
	editError,
	active,
	layoutDependencies,
}: {
	actions: CueDraftActions;
	thumbnail: string | undefined;
	editError: string;
	active: boolean;
	layoutDependencies: unknown[];
}) {
	const refs = {
		title: useRef<HTMLInputElement>(null),
		fade: useRef<HTMLInputElement>(null),
		delay: useRef<HTMLInputElement>(null),
		triggerTime: useRef<HTMLInputElement>(null),
		triggerPicker: useRef<HTMLDivElement>(null),
		grid: useRef<HTMLDivElement>(null),
	};
	const layout = useCuePropertiesLayout(active, layoutDependencies);
	refs.grid = layout.gridRef;
	const [triggerModalOpen, setTriggerModalOpen] = useState(false);
	const openInput = (field: "title" | "fade" | "delay" | "triggerTime") => {
		if (!layout.setArmed) return;
		layout.setSetArmed(false);
		const input = refs[field].current;
		const buttonName = field === "title" ? "Open keyboard" : "Open number pad";
		input
			?.closest(".ui-form-field")
			?.querySelector<HTMLButtonElement>(`button[aria-label="${buttonName}"]`)
			?.click();
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
				<CuePropertyFields actions={actions} refs={refs} />
				{!layout.fieldsFit && (
					<CompactCueProperties
						actions={actions}
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
					close={() => setTriggerModalOpen(false)}
				/>
			)}
		</>
	);
}
