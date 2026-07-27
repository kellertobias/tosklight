import { Button } from "@tosklight/ui";
import {
	alignModes,
	compactFamilyLabels,
	type ParameterFamily,
	parameterFamilies,
	type SpecialParameterFamily,
	specialParameterFamilies,
} from "./model";
import type { ParameterController } from "./useParameterController";
import { useDynamicEditorSession } from "../../../features/dynamics/DynamicEditorSessionContext";
import type { DynamicEditorTask } from "../../../features/dynamics/DynamicEditorSessionContext";

function FamilyLabel({ full, compact }: { full: string; compact: string }) {
	return (
		<>
			<span className="family-label-full" aria-hidden="true">
				{full}
			</span>
			<span className="family-label-compact" aria-hidden="true">
				{compact}
			</span>
		</>
	);
}

function alignLabel(mode: ParameterController["alignMode"]) {
	return mode ? mode[0].toUpperCase() + mode.slice(1) : "Off";
}

function AlignmentControl({ controller }: { controller: ParameterController }) {
	if (controller.family !== "Position") return null;
	const label = alignLabel(controller.alignMode);
	return (
		<Button
			aria-label={`Align ${label}`}
			className={`align-cycle ${controller.alignMode ? "align-active" : "align-off"}`}
			onClick={(event) => {
				if (event.shiftKey || controller.state.shiftArmed) {
					controller.setAlignMode(null);
					if (controller.state.shiftArmed)
						controller.dispatch({ type: "SET_SHIFT_ARMED", value: false });
					return;
				}
				const next =
					alignModes[
						(controller.alignMode == null
							? 0
							: alignModes.indexOf(controller.alignMode) + 1) %
							alignModes.length
					];
				void controller.programmerActions?.alignSelection("pan", next);
				controller.setAlignMode(next);
			}}
		>
			<span className="align-label-full">
				<span>Align</span>
				<span>{label}</span>
			</span>
			<span className="align-label-compact">
				<span>Align</span>
				<span>{label}</span>
			</span>
		</Button>
	);
}

function SpecialDialogButton({
	controller,
}: {
	controller: ParameterController;
}) {
	if (
		!specialParameterFamilies.has(controller.family as SpecialParameterFamily)
	)
		return null;
	return (
		<Button
			className="special-dialogs"
			aria-label="Special Dialog"
			onClick={() =>
				controller.dispatch({
					type: "OPEN_SPECIAL_DIALOG",
					family: controller.family as SpecialParameterFamily,
				})
			}
		>
			<span className="special-dialog-label-full">
				<span>Special</span>
				<span>Dialog</span>
			</span>
			<span className="special-dialog-label-compact">Spcl</span>
		</Button>
	);
}

export function ParameterFamilyTabs({
	controller,
}: {
	controller: ParameterController;
}) {
	const editor = useDynamicEditorSession();
	if (editor.session) return <DynamicEditorTaskTabs />;
	return (
		<div className="family-tabs">
			{(Object.keys(parameterFamilies) as ParameterFamily[]).map((name) => (
				<Button
					key={name}
					aria-label={name}
					className={`attribute-family ${controller.family === name ? "active" : ""}`}
					onClick={() => controller.setFamily(name)}
				>
					<FamilyLabel full={name} compact={compactFamilyLabels[name]} />
				</Button>
			))}
			<span className="family-spacer" />
			<AlignmentControl controller={controller} />
			<SpecialDialogButton controller={controller} />
			<Button
				aria-label="Dynamics"
				className={`dynamics-family ${controller.dynamicsMode ? "active" : ""}`}
				onClick={() => controller.setDynamicsMode(!controller.dynamicsMode)}
			>
				<FamilyLabel full="Dynamics" compact="Dyn" />
			</Button>
		</div>
	);
}

export function DynamicEditorTaskTabs({
	task,
	onTask,
}: {
	task?: DynamicEditorTask;
	onTask?(task: DynamicEditorTask): void;
} = {}) {
	const editor = useDynamicEditorSession();
	const activeTask = task ?? editor.session?.task;
	if (!activeTask) return null;
	return (
		<div className="family-tabs dynamics-editor-family-tabs">
			{(["curves", "phase", "speed"] as const).map((task) => {
				const label =
					task === "phase"
						? "Phase Spread"
						: task[0].toUpperCase() + task.slice(1);
				return (
					<Button
						key={task}
						aria-label={label}
						className={activeTask === task ? "active" : ""}
						onClick={() => (onTask ? onTask(task) : editor.update({ task }))}
					>
						<FamilyLabel
							full={label}
							compact={
								task === "phase"
									? "Phase"
									: task === "curves"
										? "Curves"
										: "Speed"
							}
						/>
					</Button>
				);
			})}
		</div>
	);
}
