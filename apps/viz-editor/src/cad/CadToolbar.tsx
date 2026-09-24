/**
 * The CAD window title's tool buttons: Undo and Redo, what to add to the venue, and what the pointer
 * draws. They are ordinary window title groups, so they look, group and divide exactly like the
 * title's other buttons, and they show icons only — each names itself, and the key that picks it, in
 * a tooltip below it. The part buttons are split: the button places its part, a caret in its corner
 * chooses which part that is.
 */
import type { TitleAction, TitleActionGroup } from "@tosklight/ui";
import boxSvg from "../../../../assets/icons/drawing/box.svg?raw";
import eraseSvg from "../../../../assets/icons/drawing/erase.svg?raw";
import measureSvg from "../../../../assets/icons/drawing/measure.svg?raw";
import polylineSvg from "../../../../assets/icons/drawing/polyline.svg?raw";
import redoSvg from "../../../../assets/icons/drawing/redo.svg?raw";
import selectSvg from "../../../../assets/icons/drawing/select.svg?raw";
import textSvg from "../../../../assets/icons/drawing/text.svg?raw";
import undoSvg from "../../../../assets/icons/drawing/undo.svg?raw";
import curtainSvg from "../../../../assets/icons/misc/curtain.svg?raw";
import primitiveSvg from "../../../../assets/icons/misc/primitive.svg?raw";
import stageElementSvg from "../../../../assets/icons/misc/stage-element.svg?raw";
import trussSvg from "../../../../assets/icons/misc/truss-segment.svg?raw";
import venueObjectSvg from "../../../../assets/icons/misc/venue-object.svg?raw";
import { CAD_TOOL_SHORTCUTS } from "./cadShortcuts";
import type { CadAddKind, CadDrawTool, CadTools } from "./cadTools";
import { CadPartMenu } from "./CadPartMenu";
import { rememberPart } from "./cadAddChoice";
import { LOAD_MODEL } from "./cadModelImport";
import { placedWith } from "./cadPlacement";
import { ErrorMessage } from "../ErrorMessage";
import { type CadPartKind, findPart, partLabel } from "./venueParts";
import "./cadTitleTools.css";

export const CAD_ADD_ACTIONS: readonly {
	kind: CadAddKind;
	label: string;
	svg: string;
}[] = [
	{ kind: "truss", label: "Add truss", svg: trussSvg },
	{ kind: "stage", label: "Add stage element", svg: stageElementSvg },
	{ kind: "curtain", label: "Add scenery", svg: curtainSvg },
	{ kind: "primitive", label: "Add primitive", svg: primitiveSvg },
	{ kind: "venue", label: "Add venue element", svg: venueObjectSvg },
];

const DRAW_TOOLS: readonly { tool: CadDrawTool; label: string; svg: string }[] = [
	{ tool: "select", label: "Select", svg: selectSvg },
	{ tool: "polyline", label: "Draw line", svg: polylineSvg },
	{ tool: "box", label: "Draw box", svg: boxSvg },
	{ tool: "text", label: "Place text", svg: textSvg },
	{ tool: "measure", label: "Measure", svg: measureSvg },
	{ tool: "erase", label: "Erase", svg: eraseSvg },
];

/** A shared icon drawn inline, so it takes the button's colour; its title would only repeat the label. */
export function ToolIcon({ svg }: { svg: string }) {
	return (
		<span
			className="cad-tool-icon"
			aria-hidden="true"
			// The icons are repository-owned files, not operator input.
			dangerouslySetInnerHTML={{
				__html: svg.replace(/<title>[\s\S]*?<\/title>/u, ""),
			}}
		/>
	);
}

/**
 * The title groups, left to right: history, adding, drawing. Adding and drawing need a host that
 * offers them; history is always there.
 */
export function cadTitleGroups(
	tools: CadTools,
	history: { disabled: boolean; onUndo(): void; onRedo(): void },
): TitleActionGroup[] {
	const { onAdd } = tools;
	return [
		{
			id: "cad-history",
			actions: [
				{
					id: "undo",
					icon: <ToolIcon svg={undoSvg} />,
					ariaLabel: "Undo",
					disabled: history.disabled,
					onPress: history.onUndo,
				},
				{
					id: "redo",
					icon: <ToolIcon svg={redoSvg} />,
					ariaLabel: "Redo",
					disabled: history.disabled,
					onPress: history.onRedo,
				},
			],
		},
		...(onAdd
			? [
					{
						id: "cad-add",
						actions: CAD_ADD_ACTIONS.map(
							({ kind, label, svg }): TitleAction => ({
								id: `add-${kind}`,
								icon: <ToolIcon svg={svg} />,
								ariaLabel: label,
								onPress: () => onAdd(kind),
								// A part button's caret chooses the part it places; a Venue element is chosen from its
								// own picture list on every press.
								...(kind === "venue"
									? {}
									: {
											dropdownPlacement: "corner" as const,
											dropdown: {
												kind: "content" as const,
												ariaLabel: label.replace(/^Add /u, "Choose "),
												render: ({ close }: { close(): void }) => (
													<CadPartMenu
														kind={kind}
														onChoose={(profileId) => {
															close();
															onAdd(kind, profileId);
														}}
														onAddSeveral={(key) => {
															close();
															holdPart(tools, kind, key);
														}}
														onLoadModel={
															kind === "primitive"
																? () => {
																		close();
																		onAdd(kind, LOAD_MODEL);
																	}
																: undefined
														}
														onSeveral={
															kind === "truss" || kind === "stage"
																? (profileId) => {
																		close();
																		onAdd(kind, profileId, true);
																	}
																: undefined
														}
													/>
												),
											},
										}),
							}),
						),
					},
					{
						id: "cad-draw",
						actions: DRAW_TOOLS.map(({ tool, label, svg }) => ({
							id: `tool-${tool}`,
							icon: <ToolIcon svg={svg} />,
							ariaLabel: label,
							shortcut: CAD_TOOL_SHORTCUTS[tool],
							active: tools.tool === tool,
							onPress: () => tools.setTool(tool),
						})),
					},
				]
			: []),
	];
}

/**
 * Holds one catalogue part for repeated placement, as the Venue list's Add Several does: every
 * press on a viewport then places one more copy, with the part's own options and size.
 */
export function holdPart(tools: CadTools, kind: CadPartKind, key: string) {
	const found = findPart(kind, key);
	if (!found) return;
	rememberPart(kind, key);
	tools.startPlacing({
		profileId: found.part.profileId,
		name: partLabel(found),
		with: placedWith(found.part),
	});
}

/** Why the show refused the last drawn item, until the operator dismisses it. */
export function CadToolError({ tools }: { tools: CadTools }) {
	return tools.error ? (
		<ErrorMessage className="cad-error" message={tools.error} onDismiss={tools.clearError} />
	) : null;
}
