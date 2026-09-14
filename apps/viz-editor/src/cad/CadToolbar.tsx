/**
 * The CAD screen's toolbar under its title: what to add to the venue, and what the pointer draws.
 *
 * It is built from the same title chrome as the window title, so its buttons look and group like
 * the title's own, and it shows icons only.
 */
import { TitleChrome } from "@tosklight/ui";
import boxSvg from "../../../../assets/icons/drawing/box.svg?raw";
import eraseSvg from "../../../../assets/icons/drawing/erase.svg?raw";
import measureSvg from "../../../../assets/icons/drawing/measure.svg?raw";
import polylineSvg from "../../../../assets/icons/drawing/polyline.svg?raw";
import selectSvg from "../../../../assets/icons/drawing/select.svg?raw";
import textSvg from "../../../../assets/icons/drawing/text.svg?raw";
import curtainSvg from "../../../../assets/icons/misc/curtain.svg?raw";
import stageElementSvg from "../../../../assets/icons/misc/stage-element.svg?raw";
import trussSvg from "../../../../assets/icons/misc/truss-segment.svg?raw";
import venueObjectSvg from "../../../../assets/icons/misc/venue-object.svg?raw";
import { type CadAddKind, type CadDrawTool, useCadTools } from "./cadTools";

const ADD_ACTIONS: readonly { kind: CadAddKind; label: string; svg: string }[] = [
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
function ToolIcon({ svg }: { svg: string }) {
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

export function CadToolbar() {
	const tools = useCadTools();
	const { onAdd } = tools;
	if (!onAdd) return null;
	return (
		<div className="cad-toolbar" role="toolbar" aria-label="CAD tools">
			<TitleChrome
				className="ui-window-action-groups"
				groupClassName="ui-window-action-group"
				terminalActions={[]}
				groups={[
					{
						id: "cad-add",
						actions: ADD_ACTIONS.map(({ kind, label, svg }) => ({
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
				]}
			/>
			{tools.error ? (
				<output className="cad-error" role="alert" onClick={tools.clearError}>
					{tools.error}
				</output>
			) : null}
		</div>
	);
}
