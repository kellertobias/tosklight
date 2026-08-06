import { Button } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import type { SetupWindowController } from "./controller";

export type SetupSection =
	| "shows"
	| "outputs"
	| "timecode"
	| "network"
	| "screens"
	| "preferences-defaults"
	| "preferences-attributes"
	| "preferences-highlight"
	| "preferences-others";

const SETUP_SECTIONS: ReadonlyArray<{
	id: SetupSection;
	label: string;
	group?: "Preferences";
}> = [
	{ id: "shows", label: "Shows & recovery" },
	{ id: "outputs", label: "Outputs" },
	{ id: "timecode", label: "Timecode" },
	{ id: "network", label: "Network & Inputs" },
	{ id: "screens", label: "Screens & playback" },
	{ id: "preferences-defaults", label: "Defaults", group: "Preferences" },
	{
		id: "preferences-attributes",
		label: "Attributes & encoders",
		group: "Preferences",
	},
	{ id: "preferences-highlight", label: "Highlight", group: "Preferences" },
	{ id: "preferences-others", label: "Others", group: "Preferences" },
];

export type AttributeSettingsTab =
	| "encoder-groups"
	| "activation-groups"
	| "attributes";

export const ATTRIBUTE_SETTINGS_TABS: ReadonlyArray<{
	id: AttributeSettingsTab;
	label: string;
}> = [
	{ id: "encoder-groups", label: "Encoder groups" },
	{ id: "activation-groups", label: "Attribute activation groups" },
	{ id: "attributes", label: "Attributes" },
];

export function setupSectionLabel(section: SetupSection) {
	return (
		SETUP_SECTIONS.find((candidate) => candidate.id === section)?.label ??
		section
	);
}

export function isPreferencesSection(section: SetupSection) {
	return section.startsWith("preferences-");
}

export function SetupHeader({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const actions =
		controller.section === "screens"
			? [
					[
						{
							id: "undo",
							label: "Undo",
							disabled: !controller.screenCanUndo,
							onClick: () => controller.screenUndo.current?.(),
						},
						{
							id: "desk-lock",
							label: "Desk Lock",
							onClick: () => controller.setDeskLockSettingsOpen(true),
						},
					],
				]
			: [
					...(controller.section === "preferences-attributes"
						? [
								ATTRIBUTE_SETTINGS_TABS.map(({ id, label }) => ({
									id: `attribute-tab-${id}`,
									label,
									active: controller.attributeTab === id,
									onClick: () => controller.setAttributeTab(id),
								})),
							]
						: []),
					[
						{
							id: "save",
							label: "Save changes",
							disabled:
								!controller.draft ||
								(controller.section === "preferences-defaults" &&
									!controller.programmerSettingsLoaded),
							onClick: () => void controller.save(),
						},
					],
				];
	return (
		<WindowHeader
			title="Desk Setup"
			info={{
				primary: setupSectionLabel(controller.section),
				secondary: controller.restartRequired ? "Restart required" : undefined,
			}}
			actions={actions}
		/>
	);
}

export function SetupNavigation({
	section,
	onSelect,
}: {
	section: SetupSection;
	onSelect: (section: SetupSection) => void;
}) {
	let renderedPreferencesLabel = false;
	return (
		<nav aria-label="Desk Setup">
			{SETUP_SECTIONS.map(({ id, label, group }) => {
				const groupLabel =
					group && !renderedPreferencesLabel ? (
						<div className="setup-navigation-group" key={`${group}-label`}>
							{group}
						</div>
					) : null;
				if (group) renderedPreferencesLabel = true;
				return (
					<div
						className={group ? "setup-navigation-child" : undefined}
						key={id}
					>
						{groupLabel}
						<Button
							onClick={() => onSelect(id)}
							className={id === section ? "active" : ""}
							aria-current={id === section ? "page" : undefined}
						>
							{label}
						</Button>
					</div>
				);
			})}
		</nav>
	);
}
