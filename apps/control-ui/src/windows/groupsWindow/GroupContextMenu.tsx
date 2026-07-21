import { Button, Input } from "../../components/common";
import type { GroupManagementOperation } from "../../features/groupManagement/contracts";
import { useGroupManagement } from "../../features/groupManagement/GroupManagementProvider";
import {
	captureGroupRecordingTarget,
	type GroupRecordingTarget,
} from "../../features/groupRecording/target";
import type { Group } from "./model";

function orderedMembers(group: Group, fixtureNames: Map<string, string>) {
	if (!group.body.fixtures.length) return "empty";
	return group.body.fixtures
		.map(
			(fixture, index) =>
				`${index + 1}. ${fixtureNames.get(fixture) ?? fixture}`,
		)
		.join(" · ");
}

export function GroupContextMenu({
	fixtureNames,
	group,
	onClose,
	recordGroup,
	runCommand,
	setGroupMaster,
	canWriteMaster,
}: {
	fixtureNames: Map<string, string>;
	group: Group;
	onClose: () => void;
	recordGroup: (target: GroupRecordingTarget) => Promise<unknown>;
	runCommand: (command: string) => Promise<unknown>;
	setGroupMaster: (groupId: string, value: number) => Promise<unknown>;
	canWriteMaster: boolean;
}) {
	const groupManagement = useGroupManagement();
	const name = group.body.name ?? `Group ${group.id}`;
	const runAndClose = (command: string) => {
		void runCommand(command);
		onClose();
	};
	const manageAndClose = (operation: GroupManagementOperation) => {
		void groupManagement?.manage({
			objectId: group.id,
			expectedObjectRevision: group.revision,
			operation,
		});
		onClose();
	};
	const replaceMembership = () => {
		const count = Object.keys(group.body.programming ?? {}).length;
		if (
			!count ||
			window.confirm(
				`Replace membership and apply ${count} stored attributes to the new members?`,
			)
		) {
			void recordGroup(captureGroupRecordingTarget(group));
		}
		onClose();
	};

	return (
		<div className="group-context-menu">
			<h3>{name}</h3>
			<small className="group-order">
				Ordered members: {orderedMembers(group, fixtureNames)}
			</small>
			<div className="group-context-master">
				Master
				<strong>{Math.round((group.body.master ?? 1) * 100)}%</strong>
				<Input
					aria-label={`${name} master`}
					type="range"
					min="0"
					max="100"
					disabled={!canWriteMaster}
					value={(group.body.master ?? 1) * 100}
					onChange={(event) =>
						void setGroupMaster(group.id, Number(event.target.value) / 100)
					}
				/>
			</div>
			<Button onClick={() => runAndClose(`GROUP ${group.id}`)}>
				Select live group
			</Button>
			<Button onClick={() => runAndClose(`GROUP GROUP ${group.id}`)}>
				Select frozen group
			</Button>
			{group.body.frozen_from && (
				<Button
					onClick={() =>
						manageAndClose({
							type: "refresh_frozen",
							expectedSource: {
								sourceGroupId: group.body.frozen_from?.source_group_id ?? "",
								expectedSourceRevision: null,
							},
						})
					}
				>
					Refresh frozen snapshot
				</Button>
			)}
			{group.body.derived_from ? (
				<Button
					onClick={() =>
						manageAndClose({
							type: "detach_derived",
							expectedSource: {
								sourceGroupId: group.body.derived_from?.source_group_id ?? "",
								expectedSourceRevision: null,
							},
						})
					}
				>
					Detach derived group
				</Button>
			) : (
				<Button onClick={replaceMembership}>
					Replace membership with selection
				</Button>
			)}
			<Button onClick={() => manageAndClose({ type: "undo" })}>
				Undo membership/programming change
			</Button>
			<Button onClick={onClose}>Cancel</Button>
		</div>
	);
}
