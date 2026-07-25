import type { AttributeDescriptor, FixtureMode } from "../../../api/types";
import { ModalTitleBar } from "../../common";
import { ChannelsEditor } from "./channels";
import { ColorEditor } from "./colorEditor";
import { GeometryEditor } from "./geometryEditor";
import { HeadsEditor } from "./heads";

export type ModeEditorTab = "heads" | "channels" | "color" | "geometry";

export function ModeEditor({
	mode,
	tab,
	attributeRegistry,
	openSplit,
	onTabChange,
	onOpenSplit,
	onChange,
	onClose,
}: {
	mode: FixtureMode;
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
					tabs={(["heads", "channels", "color", "geometry"] as const).map(
						(id) => ({ id, label: id[0].toUpperCase() + id.slice(1) }),
					)}
					activeTab={modeTab}
					onTabChange={(id) => onTabChange(id as ModeEditorTab)}
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
					{modeTab === "geometry" && (
						<GeometryEditor mode={editedMode} onChange={onChange} />
					)}
				</div>
			</section>
		</div>
	);
}
