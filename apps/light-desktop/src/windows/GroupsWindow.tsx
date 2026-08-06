import { WindowHeader, WindowSettings } from "@tosklight/ui/window-kit";
import { useEffect, useState } from "react";
import {
	type CommandLineSurface,
	useCommandLineSurface,
} from "../components/control/commandLine/useCommandLineSurface";
import { PoolColorSettings } from "../components/shared/PoolColorSettings";
import {
	type RecordMode,
	RecordModeDialog,
} from "../components/shared/RecordModeDialog";
import { useActiveShowId } from "../features/deskSnapshot/DeskSnapshotState";
import { useGroupRecording } from "../features/groupRecording/GroupRecordingProvider";
import type { GroupRecordingTarget } from "../features/groupRecording/target";
import { useApp } from "../state/AppContext";
import { GroupPoolGrid } from "./groupsWindow/GroupPoolGrid";
import { GroupSettingsDialog } from "./groupsWindow/GroupSettingsDialog";
import { useGroupPoolModel } from "./groupsWindow/model";
import type { WindowProps } from "./windowTypes";

export function GroupPoolHeader({
	command,
	onSettings,
}: {
	command: CommandLineSurface;
	onSettings(anchor: DOMRect): void;
}) {
	const { state, dispatch } = useApp();
	return (
		<WindowHeader
			title="Group Pool"
			info={{
				primary: `${command.selected.length} fixtures selected`,
				secondary: "Ordered selection",
			}}
			actions={[
				[
					...(state.groupsReturnToStage
						? [
								{
									id: "stage",
									label: "Back to Stage",
									onClick: () => dispatch({ type: "RETURN_TO_STAGE" }),
								},
							]
						: []),
				],
				[
					{
						id: "presets",
						label: "Presets",
						onClick: () => dispatch({ type: "OPEN_BUILTIN", kind: "presets" }),
					},
				],
			]}
			settings
			onSettings={(button) => onSettings(button.getBoundingClientRect())}
		/>
	);
}

export function GroupsWindow({
	active = true,
	compact,
	paneId,
	poolColumns,
}: WindowProps) {
	const groupScope = useActiveShowId();
	const groupRecording = useGroupRecording();
	const command = useCommandLineSurface({
		selection: true,
		enabled: active,
		observeCommand: false,
	});
	const { dispatch } = useApp();
	const model = useGroupPoolModel(active);
	const [recordGroup, setRecordGroup] = useState<GroupRecordingTarget | null>(
		null,
	);
	const [settingsGroup, setSettingsGroup] = useState<string | null>(null);
	const [colorSettingsAnchor, setColorSettingsAnchor] =
		useState<DOMRect | null>(null);
	const settingsTarget = model.groups.find(
		(group) => group.id === settingsGroup,
	);

	useEffect(() => {
		setSettingsGroup(null);
		setRecordGroup(null);
	}, [groupScope, model.groupRuntimeReady]);

	useEffect(() => {
		if (!active) return;
		const openRequestedGroup = (event: Event) => {
			const id = (event as CustomEvent<string>).detail;
			if (model.groups.some((group) => group.id === id)) setSettingsGroup(id);
		};
		window.addEventListener("light:group-configuration", openRequestedGroup);
		return () =>
			window.removeEventListener(
				"light:group-configuration",
				openRequestedGroup,
			);
	}, [active, model.groups]);

	const recordGroupAction = async (
		target: GroupRecordingTarget,
		mode: RecordMode = "overwrite",
	) => {
		if (!groupRecording) return null;
		const outcome = await groupRecording.record({
			objectId: target.objectId,
			operation: mode,
			expectedObjectRevision: target.expectedObjectRevision,
		});
		if (outcome) await command.reset();
		return outcome;
	};
	const cancelRecording = () => {
		setRecordGroup(null);
		dispatch({ type: "SET_STORE_ARMED", value: false });
	};
	const recordExistingGroup = async (mode: RecordMode) => {
		if (!recordGroup) return cancelRecording();
		await recordGroupAction(recordGroup, mode);
		cancelRecording();
	};

	return (
		<div className="pool-window group-pool-window">
			{!compact && (
				<GroupPoolHeader
					command={command}
					onSettings={setColorSettingsAnchor}
				/>
			)}
			{model.groupRuntimeReady ? (
				<GroupPoolGrid
					active={active}
					command={command}
					cards={model.cards}
					capabilities={model.capabilities}
					knownFixtureIds={model.knownFixtureIds}
					onOpenSettings={setSettingsGroup}
					onOpenRecord={setRecordGroup}
					recordGroup={recordGroupAction}
					paneId={paneId}
					columns={poolColumns}
				/>
			) : (
				<p className="pool-loading" role="status">
					Group runtime loading…
				</p>
			)}
			{recordGroup && (
				<RecordModeDialog
					target={recordGroup.label}
					onChoose={recordExistingGroup}
					onCancel={cancelRecording}
				/>
			)}
			{settingsTarget && (
				<GroupSettingsDialog
					// Keyed on identity only: the dialog tracks the authoritative revision itself, and
					// remounting on every edit threw the operator back to the first tab.
					key={settingsTarget.id}
					group={settingsTarget}
					groups={model.groups}
					onClose={() => setSettingsGroup(null)}
				/>
			)}
			{colorSettingsAnchor && (
				<WindowSettings
					modal={false}
					anchor={colorSettingsAnchor}
					title="Group Pool Settings"
					onClose={() => setColorSettingsAnchor(null)}
					tabs={[
						{
							id: "colors",
							label: "Colors",
							content: <PoolColorSettings objectType="group" paneId={paneId} />,
						},
					]}
				/>
			)}
		</div>
	);
}
