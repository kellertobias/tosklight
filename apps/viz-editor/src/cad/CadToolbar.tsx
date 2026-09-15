/**
 * The CAD window title's tool buttons: Undo and Redo, what to add to the venue, and what the pointer
 * draws. They are ordinary window title groups, so they look, group and divide exactly like the
 * title's other buttons, and they show icons only — each names itself in a tooltip below it.
 */
import type { TitleActionGroup } from "@tosklight/ui";
import boxSvg from "../../../../assets/icons/drawing/box.svg?raw";
import eraseSvg from "../../../../assets/icons/drawing/erase.svg?raw";
import measureSvg from "../../../../assets/icons/drawing/measure.svg?raw";
import polylineSvg from "../../../../assets/icons/drawing/polyline.svg?raw";
import redoSvg from "../../../../assets/icons/drawing/redo.svg?raw";
import selectSvg from "../../../../assets/icons/drawing/select.svg?raw";
import textSvg from "../../../../assets/icons/drawing/text.svg?raw";
import undoSvg from "../../../../assets/icons/drawing/undo.svg?raw";
import curtainSvg from "../../../../assets/icons/misc/curtain.svg?raw";
import stageElementSvg from "../../../../assets/icons/misc/stage-element.svg?raw";
import trussSvg from "../../../../assets/icons/misc/truss-segment.svg?raw";
import venueObjectSvg from "../../../../assets/icons/misc/venue-object.svg?raw";
import type { CadAddKind, CadDrawTool, CadTools } from "./cadTools";
import "./cadTitleTools.css";

export const CAD_ADD_ACTIONS: readonly {
	kind: CadAddKind;
	label: string;
	svg: string;
}[] = [
	{ kind: "truss", label: "Add truss", svg: trussSvg },
	{ kind: "stage", label: "Add stage element", svg: stageElementSvg },
	{ kind: "curtain", label: "Add curtain", svg: curtainSvg },
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
						actions: CAD_ADD_ACTIONS.map(({ kind, label, svg }) => ({
							id: `add-${kind}`,
							icon: <ToolIcon svg={svg} />,
							ariaLabel: label,
							onPress: () => onAdd(kind),
						})),
					},
					{
						id: "cad-draw",
						actions: DRAW_TOOLS.map(({ tool, label, svg }) => ({
							id: `tool-${tool}`,
							icon: <ToolIcon svg={svg} />,
							ariaLabel: label,
							active: tools.tool === tool,
							onPress: () => tools.setTool(tool),
						})),
					},
				]
			: []),
	];
}

/** Why the show refused the last drawn item, until the operator dismisses it. */
export function CadToolError({ tools }: { tools: CadTools }) {
	return tools.error ? (
		<output className="cad-error" role="alert" onClick={tools.clearError}>
			{tools.error}
		</output>
	) : null;
}
