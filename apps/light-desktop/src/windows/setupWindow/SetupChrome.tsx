import { Button } from "@tosklight/ui";
import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
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
	{ id: "preferences-highlight", label: "Highlight", group: "Preferences" },
	{
		id: "preferences-attributes",
		label: "Attributes & encoders",
		group: "Preferences",
	},
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

export type NetworkSettingsTab = "control-server" | "sound" | "bridges";
export type DefaultsSettingsTab = "record-update" | "playback" | "pools";
export type OutputsSettingsTab = "engine" | "routes" | "audio";

export const OUTPUTS_SETTINGS_TABS: ReadonlyArray<{
	id: OutputsSettingsTab;
	label: string;
}> = [
	{ id: "engine", label: "Output Engine" },
	{ id: "routes", label: "Routes" },
	{ id: "audio", label: "Audio Output" },
];

const NETWORK_SETTINGS_TABS: ReadonlyArray<{
	id: NetworkSettingsTab;
	label: string;
}> = [
	{ id: "control-server", label: "Control & server" },
	{ id: "sound", label: "Sound" },
	{ id: "bridges", label: "Bridges" },
];

const DEFAULTS_SETTINGS_TABS: ReadonlyArray<{
	id: DefaultsSettingsTab;
	label: string;
}> = [
	{ id: "record-update", label: "Record & Update" },
	{ id: "playback", label: "Playback" },
	{ id: "pools", label: "Pool colors" },
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
	const groups =
		controller.section === "screens"
			? [
					{
						id: "screen-configuration",
						actions: [
							{
								id: "undo",
								label: "Undo",
								disabled: !controller.screenCanUndo,
								onPress: () => controller.screenUndo.current?.(),
							},
							{
								id: "encoder-placement",
								label: "Configure encoder placement",
								onPress: () => controller.setEncoderPlacementOpen(true),
							},
							{
								id: "desk-lock",
								label: "Configure desk lock",
								onPress: () => controller.setDeskLockSettingsOpen(true),
							},
						],
					},
				]
			: [
					...(controller.section === "outputs"
						? [
								{
									id: "output-settings",
									kind: "tabs" as const,
									activeId: controller.outputsTab,
									onActiveChange: (id: string) =>
										controller.setOutputsTab(
											id as typeof controller.outputsTab,
										),
									actions: OUTPUTS_SETTINGS_TABS.map(({ id, label }) => ({
										id,
										label,
									})),
								},
							]
						: []),
					...(controller.section === "network"
						? [
								{
									id: "network-settings",
									kind: "tabs" as const,
									activeId: controller.networkTab,
									onActiveChange: (id: string) =>
										controller.setNetworkTab(
											id as typeof controller.networkTab,
										),
									actions: NETWORK_SETTINGS_TABS.map(({ id, label }) => ({
										id,
										label,
									})),
								},
							]
						: []),
					...(controller.section === "preferences-defaults"
						? [
								{
									id: "defaults-settings",
									kind: "tabs" as const,
									activeId: controller.defaultsTab,
									onActiveChange: (id: string) =>
										controller.setDefaultsTab(
											id as typeof controller.defaultsTab,
										),
									actions: DEFAULTS_SETTINGS_TABS.map(({ id, label }) => ({
										id,
										label,
									})),
								},
							]
						: []),
					...(controller.section === "preferences-attributes"
						? [
								{
									id: "attribute-settings",
									kind: "tabs" as const,
									activeId: controller.attributeTab,
									onActiveChange: (id: string) =>
										controller.setAttributeTab(
											id as typeof controller.attributeTab,
										),
									actions: ATTRIBUTE_SETTINGS_TABS.map(({ id, label }) => ({
										id,
										label,
									})),
								},
							]
						: []),
				];
	return (
		<WindowHeader
			title="Desk Setup"
			info={{
				primary: setupSectionLabel(controller.section),
				secondary: controller.restartRequired ? "Restart required" : undefined,
			}}
			groups={groups}
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
		<WindowScrollArea className="setup-navigation-scroll">
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
		</WindowScrollArea>
	);
}
