import type {
	AttributeDescriptor,
	FixtureMode,
	GeometryGraph,
} from "../wire";
import {
	ModalRegistration,
	ModalTitleBar,
	type TitleActionGroup,
} from "@tosklight/ui";
import { uuid } from "../sheet/fixtureProfileModel";
import {
	EditorBreadcrumbs,
	EditorTrailProvider,
	useEditorTrail,
} from "./breadcrumbs";
import { addChannel } from "./channelOperations";
import { ChannelsEditor } from "./channels";
import { ColorEditor } from "./colorEditor";
import { ControlActionsEditor } from "./controlActions";
import { EmitterBindings } from "./emitterBindings";
import { HeadsEditor } from "./heads";
import { MotionBindings } from "./motionBindings";

export type ModeEditorTab =
	| "heads"
	| "channels"
	| "control"
	| "color"
	| "emitters";

const MODE_TABS: readonly ModeEditorTab[] = [
	"heads",
	"channels",
	"control",
	"color",
	"emitters",
];

const MODE_TAB_LABELS: Record<ModeEditorTab, string> = {
	heads: "Heads",
	channels: "Channels",
	control: "Control actions",
	color: "Color",
	emitters: "Emitters & Motion",
};

/**
 * What the open tab can add, as title-bar buttons left of the tabs.
 *
 * Sitting left of the tabs keeps the tabs where they are when a tab brings its own buttons, and
 * the title bar draws the divider between the two groups.
 */
function tabActions(
	tab: ModeEditorTab,
	mode: FixtureMode,
	openSplit: number,
	onOpenSplit: (split: number) => void,
	onChange: (mode: FixtureMode) => void,
): TitleActionGroup[] {
	if (tab === "channels") {
		const activeSplit = mode.splits.some((split) => split.number === openSplit)
			? openSplit
			: (mode.splits[0]?.number ?? 1);
		return [
			{
				id: "channel-actions",
				actions: [
					{
						id: "add-split",
						label: "Add split",
						onPress: () => {
							const number =
								Math.max(0, ...mode.splits.map((split) => split.number)) + 1;
							onChange({
								...mode,
								splits: [...mode.splits, { number, footprint: 1 }],
							});
							onOpenSplit(number);
						},
					},
					{
						id: "add-channel",
						label: "Add channel",
						onPress: () => onChange(addChannel(mode, activeSplit)),
					},
				],
			},
		];
	}
	if (tab === "control")
		return [
			{
				id: "control-actions",
				actions: [
					{
						id: "add-control-action",
						label: "Add control action",
						onPress: () =>
							onChange({
								...mode,
								control_actions: [
									...mode.control_actions,
									{
										id: uuid(),
										name: `Action ${mode.control_actions.length + 1}`,
										semantic: "custom",
										kind: "momentary",
										duration_millis: null,
										assignments: [],
									},
								],
							}),
					},
				],
			},
		];
	return [];
}

export function ModeEditor({
	mode,
	geometry,
	tab,
	attributeRegistry,
	openSplit,
	onTabChange,
	onOpenSplit,
	onChange,
	onClose,
}: {
	mode: FixtureMode;
	/** The fixture's own graph, which this mode binds its heads to. */
	geometry: GeometryGraph;
	tab: ModeEditorTab;
	attributeRegistry: AttributeDescriptor[];
	openSplit: number;
	onTabChange: (tab: ModeEditorTab) => void;
	onOpenSplit: (split: number) => void;
	onChange: (mode: FixtureMode) => void;
	onClose: () => void;
}) {
	const editedMode = mode;
	const modeTab = tab;
	const tabLabel = MODE_TAB_LABELS[modeTab];
	const modeLabel = editedMode.name || "Unnamed mode";
	const trail = useEditorTrail([modeLabel, tabLabel], onClose);
	return (
		<EditorTrailProvider trail={trail}>
		<ModalRegistration onClose={onClose}>
			<div
				className="stacked-modal-layer fixture-mode-editor-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<section
					className="nested-modal fixture-mode-editor-modal"
					role="dialog"
					aria-modal="true"
					aria-label={`Edit ${editedMode.name || "unnamed"} mode`}
				>
					<ModalTitleBar
						title={`Edit channels · ${modeLabel}`}
						details={<EditorBreadcrumbs trail={trail} />}
						groups={[
							...tabActions(modeTab, editedMode, openSplit, onOpenSplit, onChange),
							{
								id: "mode-tabs",
								kind: "tabs",
								activeId: modeTab,
								onActiveChange: (id) => onTabChange(id as ModeEditorTab),
								actions: MODE_TABS.map((id) => ({
									id,
									label: MODE_TAB_LABELS[id],
								})),
							},
						]}
						closeLabel="Close mode editor"
						onClose={() => onClose()}
					/>
					<div className="fixture-mode-editor-body">
						{modeTab === "heads" && (
							<HeadsEditor mode={editedMode} onChange={onChange} />
						)}
						{modeTab === "channels" && (
							<ChannelsEditor
								mode={editedMode}
								attributeRegistry={attributeRegistry}
								openSplit={openSplit}
								onOpenSplit={onOpenSplit}
								onChange={onChange}
							/>
						)}
						{modeTab === "control" && (
							<ControlActionsEditor mode={editedMode} onChange={onChange} />
						)}
						{modeTab === "color" && (
							<ColorEditor mode={editedMode} onChange={onChange} />
						)}
						{modeTab === "emitters" && (
							<div className="fixture-mode-geometry-bindings">
								<EmitterBindings
									mode={editedMode}
									geometry={geometry}
									onChange={onChange}
								/>
								<MotionBindings
									mode={editedMode}
									geometry={geometry}
									attributeRegistry={attributeRegistry}
									onChange={onChange}
								/>
							</div>
						)}
					</div>
				</section>
			</div>
		</ModalRegistration>
		</EditorTrailProvider>
	);
}
