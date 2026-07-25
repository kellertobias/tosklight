import type { FixtureMode, GeometryNode } from "../../../api/types";
import { Button } from "../../common";
import type { GeometryTemplateName } from "../fixtureProfileModel";

export type GeometrySelection = { type: "node" | "emitter"; id: string } | null;

function nodeDepth(mode: FixtureMode, node: GeometryNode) {
	let result = 0;
	let parent = node.parent_id;
	while (parent && result < mode.geometry.nodes.length) {
		result += 1;
		parent =
			mode.geometry.nodes.find((candidate) => candidate.id === parent)
				?.parent_id ?? null;
	}
	return result;
}

export function GeometryTemplates({
	onSelect,
}: {
	onSelect: (template: GeometryTemplateName) => void;
}) {
	return (
		<section className="geometry-templates">
			<h3>Geometry templates</h3>
			{(
				[
					["fixed", "Fixed fixture"],
					["moving_head", "Moving head"],
					["bar", "Bar"],
					["matrix", "Matrix"],
					["shared_pan_multi_head", "Shared-pan multi-head"],
				] as const
			).map(([id, label]) => (
				<Button key={id} onClick={() => onSelect(id)}>
					{label}
				</Button>
			))}
		</section>
	);
}

export function GeometryTree({
	mode,
	selected,
	onSelect,
	onAddNode,
	onAddEmitter,
}: {
	mode: FixtureMode;
	selected: GeometrySelection;
	onSelect: (selection: Exclude<GeometrySelection, null>) => void;
	onAddNode: () => void;
	onAddEmitter: () => void;
}) {
	return (
		<aside>
			<header>
				<h3>Parts and emitters</h3>
				<Button onClick={onAddNode}>Add part</Button>
				<Button onClick={onAddEmitter}>Add emitter</Button>
			</header>
			<div className="geometry-tree" role="tree">
				{mode.geometry.nodes.map((node) => {
					const depth = nodeDepth(mode, node);
					return (
						<Button
							role="treeitem"
							aria-level={depth + 1}
							key={node.id}
							active={selected?.type === "node" && selected.id === node.id}
							style={{ paddingLeft: `${12 + depth * 18}px` }}
							onClick={() => onSelect({ type: "node", id: node.id })}
						>
							◇ {node.name}
						</Button>
					);
				})}
				{mode.geometry.emitters.map((emitter) => (
					<Button
						role="treeitem"
						key={emitter.id}
						active={selected?.type === "emitter" && selected.id === emitter.id}
						onClick={() => onSelect({ type: "emitter", id: emitter.id })}
					>
						⌁ {emitter.name}
					</Button>
				))}
			</div>
		</aside>
	);
}
