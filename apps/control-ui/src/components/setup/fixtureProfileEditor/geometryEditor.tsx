import { useState } from "react";
import type {
	FixtureMode,
	GeometryEmitter,
	GeometryNode,
} from "../../../api/types";
import {
	type GeometryTemplateName,
	geometryTemplate,
	uuid,
} from "../fixtureProfileModel";
import { GeometryEmitterForm } from "./geometryEmitterForm";
import { GeometryNodeForm } from "./geometryNodeForm";
import { GeometryPreview } from "./geometryPreview";
import {
	type GeometrySelection,
	GeometryTemplates,
	GeometryTree,
} from "./geometryTree";

function newNode(mode: FixtureMode, parentId: string | null): GeometryNode {
	return {
		id: uuid(),
		name: `Part ${mode.geometry.nodes.length + 1}`,
		parent_id: parentId,
		transform: {
			translation: { x: 0, y: 0, z: 0 },
			rotation_degrees: { x: 0, y: 0, z: 0 },
			scale: { x: 1, y: 1, z: 1 },
		},
		pivot: { x: 0, y: 0, z: 0 },
		glb_node: null,
		motion: null,
	};
}

function newEmitter(
	mode: FixtureMode,
	node: GeometryNode,
	headId: string,
): GeometryEmitter {
	return {
		id: uuid(),
		name: `Emitter ${mode.geometry.emitters.length + 1}`,
		node_id: node.id,
		head_id: headId,
		origin: { x: 0, y: 0, z: 0 },
		orientation_degrees: { x: 0, y: 0, z: 0 },
		beam_angle_degrees: 20,
		field_angle_degrees: 24,
		feather: 0,
		focus: 1,
		directional: true,
		layout: { type: "point" },
	};
}

export function GeometryEditor({
	mode,
	onChange,
}: {
	mode: FixtureMode;
	onChange: (mode: FixtureMode) => void;
}) {
	const [selected, setSelected] = useState<GeometrySelection>(() =>
		mode.geometry.nodes[0]
			? { type: "node", id: mode.geometry.nodes[0].id }
			: null,
	);
	const selectedNode =
		selected?.type === "node"
			? mode.geometry.nodes.find((node) => node.id === selected.id)
			: null;
	const selectedEmitter =
		selected?.type === "emitter"
			? mode.geometry.emitters.find((emitter) => emitter.id === selected.id)
			: null;
	const setNode = (node: GeometryNode) =>
		onChange({
			...mode,
			geometry: {
				...mode.geometry,
				nodes: mode.geometry.nodes.map((candidate) =>
					candidate.id === node.id ? node : candidate,
				),
			},
		});
	const setEmitter = (emitter: GeometryEmitter) =>
		onChange({
			...mode,
			geometry: {
				...mode.geometry,
				emitters: mode.geometry.emitters.map((candidate) =>
					candidate.id === emitter.id ? emitter : candidate,
				),
			},
		});
	const useTemplate = (template: GeometryTemplateName) => {
		const geometry = geometryTemplate(
			template,
			mode.heads.map((head) => head.id),
		);
		onChange({ ...mode, geometry });
		setSelected(
			geometry.nodes[0] ? { type: "node", id: geometry.nodes[0].id } : null,
		);
	};
	const addNode = () => {
		const node = newNode(
			mode,
			selectedNode?.id ?? mode.geometry.nodes[0]?.id ?? null,
		);
		onChange({
			...mode,
			geometry: { ...mode.geometry, nodes: [...mode.geometry.nodes, node] },
		});
		setSelected({ type: "node", id: node.id });
	};
	const addEmitter = () => {
		const node = selectedNode ?? mode.geometry.nodes[0];
		const head = mode.heads[0];
		if (!node || !head) return;
		const emitter = newEmitter(mode, node, head.id);
		onChange({
			...mode,
			geometry: {
				...mode.geometry,
				emitters: [...mode.geometry.emitters, emitter],
			},
		});
		setSelected({ type: "emitter", id: emitter.id });
	};
	return (
		<div className="fixture-geometry-editor">
			<GeometryTemplates onSelect={useTemplate} />
			<div className="geometry-workspace">
				<GeometryTree
					mode={mode}
					selected={selected}
					onSelect={setSelected}
					onAddNode={addNode}
					onAddEmitter={addEmitter}
				/>
				<section className="geometry-properties">
					{selectedNode && (
						<GeometryNodeForm
							node={selectedNode}
							nodes={mode.geometry.nodes}
							onChange={setNode}
							onRemove={() => {
								const inUse =
									mode.geometry.nodes.some(
										(candidate) => candidate.parent_id === selectedNode.id,
									) ||
									mode.geometry.emitters.some(
										(emitter) => emitter.node_id === selectedNode.id,
									);
								if (inUse) return;
								onChange({
									...mode,
									geometry: {
										nodes: mode.geometry.nodes.filter(
											(candidate) => candidate.id !== selectedNode.id,
										),
										emitters: mode.geometry.emitters,
									},
								});
								setSelected(null);
							}}
						/>
					)}{" "}
					{selectedEmitter && (
						<GeometryEmitterForm
							emitter={selectedEmitter}
							mode={mode}
							onChange={setEmitter}
							onRemove={() => {
								onChange({
									...mode,
									geometry: {
										...mode.geometry,
										emitters: mode.geometry.emitters.filter(
											(candidate) => candidate.id !== selectedEmitter.id,
										),
									},
								});
								setSelected(null);
							}}
						/>
					)}
				</section>
				<GeometryPreview mode={mode} />
			</div>
		</div>
	);
}
