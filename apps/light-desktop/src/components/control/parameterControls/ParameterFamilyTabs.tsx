import { Button } from "@tosklight/ui";
import { EncoderGroupTabs } from "@tosklight/ui/encoders";
import type { DynamicDefinitionProjection } from "../../../api/types";
import type { DynamicEditorTask } from "../../../features/dynamics/DynamicEditorSessionContext";
import { useDynamicEditorSession } from "../../../features/dynamics/DynamicEditorSessionContext";
import { dynamicSpatialDraft } from "../../../features/dynamics/dynamicSpatialDraft";
import { useLowerSectionSwitch } from "../../../features/screens/LowerSectionSwitch";
import { useDynamics } from "../../../features/showObjects/ShowObjectsState";
import { projectionKind } from "../../../features/spatialMapping/projectionKinds";
import {
	alignModes,
	compactFamilyLabels,
	type ParameterFamily,
	parameterFamilies,
	type SpecialParameterFamily,
	specialParameterFamilies,
} from "./model";
import type { ParameterController } from "./useParameterController";
import { useVisibleEncoderCount } from "./VisibleEncoderCount";

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
	const label = controller.state.shiftArmed ? "Off" : alignLabel(controller.alignMode);
	const setMode = async (mode: ParameterController["alignMode"]) => {
		if (!controller.programmerActions) return;
		try {
			await controller.programmerActions.alignSelection(mode ?? "off");
			controller.setAlignMode(mode);
		} catch {
			// The server error is already projected by the programming action owner.
		}
	};
	return (
		<Button
			aria-label={`Align ${label}`}
			className={`align-cycle ${controller.alignMode ? "align-active" : "align-off"}`}
			onClick={(event) => {
				if (event.shiftKey || controller.state.shiftArmed) {
					void setMode(null);
					return;
				}
				const nextIndex =
					controller.alignMode == null
						? 0
						: alignModes.indexOf(controller.alignMode) + 1;
				void setMode(
					nextIndex >= alignModes.length ? null : alignModes[nextIndex],
				);
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
	const sectionSwitch = useLowerSectionSwitch();
	if (editor.session) return <DynamicEditorTaskTabs />;
	return (
		<EncoderGroupTabs
			className="family-tabs"
			groups={(Object.keys(parameterFamilies) as ParameterFamily[]).map(
				(name) => ({
					id: name,
					label: name,
					compactLabel: compactFamilyLabels[name],
					pageCount:
						controller.encoderGroups.find(
							(group) => group.id === name.toLowerCase(),
						)?.pages.length ?? 1,
				}),
			)}
			activeGroup={controller.family}
			page={controller.encoderPage}
			pageFormat="of"
			onChange={controller.selectEncoderGroup}
			trailing={
				<>
					<AlignmentControl controller={controller} />
					<SpecialDialogButton controller={controller} />
					<Button
						aria-label="Dynamics"
						className={`dynamics-family ${controller.dynamicsMode ? "active" : ""}`}
						onClick={() => controller.setDynamicsMode(!controller.dynamicsMode)}
					>
						<FamilyLabel full="Dynamics" compact="Dyn" />
					</Button>
					{sectionSwitch}
				</>
			}
		/>
	);
}

export function DynamicEditorTaskTabs({
	task,
	onTask,
	dynamic: controlledDynamic,
	laneId: controlledLaneId,
	onLane,
	page: controlledPage,
	onPage,
	pageCount: configuredPageCount,
}: {
	task?: DynamicEditorTask;
	onTask?(task: DynamicEditorTask): void;
	dynamic?: DynamicDefinitionProjection;
	laneId?: string | null;
	onLane?(id: string): void;
	page?: number;
	onPage?(page: number): void;
	pageCount?: number;
} = {}) {
	const editor = useDynamicEditorSession();
	const visibleEncoderCount = useVisibleEncoderCount();
	const pageCount = configuredPageCount ?? (visibleEncoderCount === 4 ? 2 : 1);
	const activeTask = task ?? editor.session?.task;
	if (!activeTask) return null;
	const page = controlledPage ?? editor.session?.encoderPage ?? 1;
	const changePage = (next: number) =>
		onPage ? onPage(next) : editor.update({ encoderPage: next });
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
				onPage={changePage}
				pageCount={pageCount}
			/>
		);
	return (
		<ConnectedDynamicEditorTaskTabs
			activeTask={activeTask}
			onTask={(next) => (onTask ? onTask(next) : editor.update({ task: next }))}
			page={page}
			onPage={changePage}
			pageCount={pageCount}
		/>
	);
}

function ConnectedDynamicEditorTaskTabs({
	activeTask,
	onTask,
	page,
	onPage,
	pageCount,
}: {
	activeTask: DynamicEditorTask;
	onTask(task: DynamicEditorTask): void;
	page: number;
	onPage(page: number): void;
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
			onPage={onPage}
			pageCount={pageCount}
		/>
	);
}

function DynamicEditorTaskTabsView({
	activeTask,
	onTask,
	dynamic,
	page,
	onPage,
	pageCount,
}: {
	activeTask: DynamicEditorTask;
	onTask(task: DynamicEditorTask): void;
	dynamic?: DynamicDefinitionProjection;
	laneId?: string | null;
	onLane(id: string): void;
	page: number;
	onPage(page: number): void;
	pageCount: number;
}) {
	return (
		<EncoderGroupTabs
			className="family-tabs dynamics-editor-family-tabs"
			groups={[
				{
					id: "curves",
					label: "Lanes",
					compactLabel: "Lanes",
					pageCount,
				},
				{
					id: "projection",
					label: "Projection",
					compactLabel: "Proj",
					// Planar fits on one page; the angular kinds place on page one and
					// orient on page two, whatever the encoder width.
					pageCount: projectionPageCount(dynamic),
				},
				{
					id: "phase",
					label: "Phase",
					compactLabel: "Phase",
					pageCount,
				},
				{ id: "speed", label: "Speed", pageCount },
			]}
			activeGroup={activeTask}
			page={page}
			onChange={(nextTask, nextPage) => {
				onTask(nextTask);
				onPage(nextPage);
			}}
		/>
	);
}

/** Two pages once a projection has an orientation to configure, one otherwise. */
function projectionPageCount(dynamic?: DynamicDefinitionProjection) {
	const stage = dynamicSpatialDraft(dynamic?.spatial_mapping).projection;
	const kind =
		stage.type === "replace" ? projectionKind(stage.value) : "planar";
	return kind === "planar" ? 1 : 2;
}
