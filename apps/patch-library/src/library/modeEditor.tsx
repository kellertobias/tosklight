import type {
	AttributeDescriptor,
	FixtureMode,
	GeometryGraph,
} from "../wire";
import { ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { ChannelsEditor } from "./channels";
import { ColorEditor } from "./colorEditor";
import { EmitterBindings } from "./emitterBindings";
import { HeadsEditor } from "./heads";

export type ModeEditorTab = "heads" | "channels" | "color" | "emitters";

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
	return (
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
						title={`Edit channels · ${editedMode.name || "Unnamed mode"}`}
						groups={[
							{
								id: "mode-tabs",
								kind: "tabs",
								activeId: modeTab,
								onActiveChange: (id) => onTabChange(id as ModeEditorTab),
								actions: (
									["heads", "channels", "color", "emitters"] as const
								).map((id) => ({
									id,
									label: id[0].toUpperCase() + id.slice(1),
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
						{modeTab === "color" && (
							<ColorEditor mode={editedMode} onChange={onChange} />
						)}
						{modeTab === "emitters" && (
							<EmitterBindings
								mode={editedMode}
								geometry={geometry}
								onChange={onChange}
							/>
						)}
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}
