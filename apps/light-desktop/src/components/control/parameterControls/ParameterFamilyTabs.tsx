import { Button } from "@tosklight/ui";
import { useState } from "react";
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
import { useDynamics } from "../../../features/showObjects/ShowObjectsState";
import type { DynamicDefinitionProjection } from "../../../api/generated/light-wire";

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
	dynamic: controlledDynamic,
	laneId: controlledLaneId,
	onLane,
	page = 1,
	pageCount = 1,
}: {
	task?: DynamicEditorTask;
	onTask?(task: DynamicEditorTask): void;
	dynamic?: DynamicDefinitionProjection;
	laneId?: string | null;
	onLane?(id: string): void;
	page?: number;
	pageCount?: number;
} = {}) {
	const editor = useDynamicEditorSession();
	const activeTask = task ?? editor.session?.task;
	if (!activeTask) return null;
	if (controlledDynamic)
		return (
			<DynamicEditorTaskTabsView
				activeTask={activeTask}
				onTask={(next) =>
					onTask ? onTask(next) : editor.update({ task: next })
				}
				dynamic={controlledDynamic}
				laneId={controlledLaneId}
				onLane={(id) =>
					onLane ? onLane(id) : editor.update({ primaryLaneId: id })
				}
				page={page}
				pageCount={pageCount}
			/>
		);
	return (
		<ConnectedDynamicEditorTaskTabs
			activeTask={activeTask}
			onTask={(next) => (onTask ? onTask(next) : editor.update({ task: next }))}
			page={page}
			pageCount={pageCount}
		/>
	);
}

function ConnectedDynamicEditorTaskTabs({
	activeTask,
	onTask,
	page,
	pageCount,
}: {
	activeTask: DynamicEditorTask;
	onTask(task: DynamicEditorTask): void;
	page: number;
	pageCount: number;
}) {
	const editor = useDynamicEditorSession();
	const dynamics = useDynamics();
	const dynamic = dynamics.find(
		(candidate) => candidate.id === editor.session?.dynamicId,
	)?.body;
	return (
		<DynamicEditorTaskTabsView
			activeTask={activeTask}
			onTask={onTask}
			dynamic={dynamic}
			laneId={editor.session?.primaryLaneId}
			onLane={(id) => editor.update({ primaryLaneId: id })}
			page={page}
			pageCount={pageCount}
		/>
	);
}

function DynamicEditorTaskTabsView({
	activeTask,
	onTask,
	dynamic,
	laneId,
	onLane,
	page,
	pageCount,
}: {
	activeTask: DynamicEditorTask;
	onTask(task: DynamicEditorTask): void;
	dynamic?: DynamicDefinitionProjection;
	laneId?: string | null;
	onLane(id: string): void;
	page: number;
	pageCount: number;
}) {
	const [laneMenuOpen, setLaneMenuOpen] = useState(false);
	const lane =
		dynamic?.lanes.find((candidate) => candidate.id === laneId) ??
		dynamic?.lanes[0];
	return (
		<div className="family-tabs dynamics-editor-family-tabs">
			{(["curves", "phase", "speed"] as const).map((task) => {
				const baseLabel =
					task === "phase"
						? "Phase Spread"
						: task[0].toUpperCase() + task.slice(1);
				const label =
					task === activeTask && pageCount > 1
						? `${baseLabel} (${page}/${pageCount})`
						: baseLabel;
				return (
					<Button
						key={task}
						aria-label={label}
						className={activeTask === task ? "active" : ""}
						onClick={() => onTask(task)}
					>
						<FamilyLabel
							full={label}
							compact={
								task === "phase"
									? "Phase"
									: task === "curves"
										? pageCount > 1
											? `Curves (${page}/${pageCount})`
											: "Curves"
										: "Speed"
							}
						/>
					</Button>
				);
			})}
			<span className="family-spacer" />
			{dynamic && lane && (
				<div className="dynamic-editor-lane-picker">
					<Button
						aria-haspopup="menu"
						aria-expanded={laneMenuOpen}
						onClick={() => setLaneMenuOpen((current) => !current)}
					>
						{lane.attribute} ▾
					</Button>
					{laneMenuOpen && (
						<div role="menu" aria-label="Dynamic lane">
							{dynamic.lanes.map((candidate) => (
								<Button
									key={candidate.id}
									role="menuitemradio"
									aria-checked={candidate.id === lane.id}
									className={candidate.id === lane.id ? "active" : ""}
									onClick={() => {
										onLane(candidate.id);
										setLaneMenuOpen(false);
									}}
								>
									{candidate.attribute}
								</Button>
							))}
						</div>
					)}
				</div>
			)}
		</div>
	);
}
