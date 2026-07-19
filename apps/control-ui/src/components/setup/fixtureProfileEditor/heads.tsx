import { useState } from "react";
import type { FixtureMode } from "../../../api/types";
import { Button, CheckboxField, TextField } from "../../common";
import { blankHead, reorder } from "../fixtureProfileModel";

type Head = FixtureMode["heads"][number];

function TouchHeadDragHandle({
	headId,
	onMove,
}: {
	headId: string;
	onMove: (sourceId: string, targetId: string) => void;
}) {
	return (
		<span
			className="drag-handle touch-drag-handle"
			aria-hidden="true"
			title="Drag to reorder heads"
			onPointerDown={(event) => {
				if (event.pointerType === "mouse") return;
				event.preventDefault();
				event.currentTarget.setPointerCapture(event.pointerId);
			}}
			onPointerMove={(event) => {
				if (
					event.pointerType === "mouse" ||
					!event.currentTarget.hasPointerCapture(event.pointerId)
				)
					return;
				const target = document
					.elementFromPoint(event.clientX, event.clientY)
					?.closest<HTMLElement>("[data-head-reorder-id]")
					?.dataset.headReorderId;
				if (target) onMove(headId, target);
			}}
			onPointerUp={(event) =>
				event.currentTarget.hasPointerCapture(event.pointerId) &&
				event.currentTarget.releasePointerCapture(event.pointerId)
			}
			onPointerCancel={(event) =>
				event.currentTarget.hasPointerCapture(event.pointerId) &&
				event.currentTarget.releasePointerCapture(event.pointerId)
			}
		>
			⠿
		</span>
	);
}

function HeadActions({
	head,
	index,
	mode,
	ownsChannels,
	onChange,
	onRemove,
}: {
	head: Head;
	index: number;
	mode: FixtureMode;
	ownsChannels: boolean;
	onChange: (mode: FixtureMode) => void;
	onRemove: () => void;
}) {
	return (
		<div className="reorder-actions">
			<Button
				iconOnly
				aria-label={`Move ${head.name} up`}
				disabled={index === 0}
				onClick={() =>
					onChange({
						...mode,
						heads: reorder(mode.heads, index, index - 1),
					})
				}
			>
				▲
			</Button>
			<Button
				iconOnly
				aria-label={`Move ${head.name} down`}
				disabled={index === mode.heads.length - 1}
				onClick={() =>
					onChange({
						...mode,
						heads: reorder(mode.heads, index, index + 1),
					})
				}
			>
				▼
			</Button>
			<Button
				iconOnly
				aria-label={`Remove ${head.name}`}
				disabled={mode.heads.length === 1 || ownsChannels}
				title={
					ownsChannels
						? "Remove or reassign this head's channels first"
						: mode.heads.length === 1
							? "The final head cannot be removed"
							: "Remove head"
				}
				onClick={onRemove}
			>
				×
			</Button>
		</div>
	);
}

function HeadRow({
	head,
	index,
	mode,
	dragHead,
	onDragHead,
	onMove,
	onChange,
	onRemove,
}: {
	head: Head;
	index: number;
	mode: FixtureMode;
	dragHead: number | null;
	onDragHead: (index: number | null) => void;
	onMove: (sourceId: string, targetId: string) => void;
	onChange: (mode: FixtureMode) => void;
	onRemove: () => void;
}) {
	const ownsChannels = mode.channels.some(
		(channel) => channel.head_id === head.id,
	);
	return (
		<article
			data-head-reorder-id={head.id}
			draggable
			onDragStart={() => onDragHead(index)}
			onDragOver={(event) => event.preventDefault()}
			onDrop={(event) => {
				event.preventDefault();
				if (dragHead != null)
					onChange({
						...mode,
						heads: reorder(mode.heads, dragHead, index),
					});
				onDragHead(null);
			}}
		>
			<TouchHeadDragHandle headId={head.id} onMove={onMove} />
			<TextField
				label="Head name"
				value={head.name}
				onChange={(event) =>
					onChange({
						...mode,
						heads: mode.heads.map((candidate) =>
							candidate.id === head.id
								? { ...candidate, name: event.target.value }
								: candidate,
						),
					})
				}
			/>
			<CheckboxField
				label="Master/shared head"
				checked={head.master_shared}
				onChange={(event) =>
					onChange({
						...mode,
						heads: mode.heads.map((candidate) => ({
							...candidate,
							master_shared:
								candidate.id === head.id
									? event.target.checked
									: event.target.checked
										? false
										: candidate.master_shared,
						})),
					})
				}
			/>
			<HeadActions
				head={head}
				index={index}
				mode={mode}
				ownsChannels={ownsChannels}
				onChange={onChange}
				onRemove={onRemove}
			/>
		</article>
	);
}

function removeHead(mode: FixtureMode, headId: string): FixtureMode | null {
	if (
		mode.heads.length === 1 ||
		mode.channels.some((channel) => channel.head_id === headId)
	)
		return null;
	return {
		...mode,
		heads: mode.heads.filter((head) => head.id !== headId),
		color_systems: mode.color_systems.filter(
			(system) => system.head_id !== headId,
		),
		geometry: {
			...mode.geometry,
			emitters: mode.geometry.emitters.filter(
				(emitter) => emitter.head_id !== headId,
			),
		},
	};
}

export function HeadsEditor({
	mode,
	onChange,
}: {
	mode: FixtureMode;
	onChange: (mode: FixtureMode) => void;
}) {
	const [dragHead, setDragHead] = useState<number | null>(null);
	const moveHead = (sourceId: string, targetId: string) => {
		const from = mode.heads.findIndex((head) => head.id === sourceId);
		const to = mode.heads.findIndex((head) => head.id === targetId);
		if (from >= 0 && to >= 0 && from !== to)
			onChange({ ...mode, heads: reorder(mode.heads, from, to) });
	};
	return (
		<div className="fixture-heads-editor">
			<section>
				<header>
					<h3>Heads</h3>
					<Button
						onClick={() => {
							const head = blankHead(mode.heads.length);
							onChange({
								...mode,
								heads: [...mode.heads, { ...head, master_shared: false }],
							});
						}}
					>
						Add head
					</Button>
				</header>
				<div className="fixture-head-list">
					{mode.heads.map((head, index) => (
						<HeadRow
							key={head.id}
							head={head}
							index={index}
							mode={mode}
							dragHead={dragHead}
							onDragHead={setDragHead}
							onMove={moveHead}
							onChange={onChange}
							onRemove={() => {
								const next = removeHead(mode, head.id);
								if (next) onChange(next);
							}}
						/>
					))}
				</div>
			</section>
		</div>
	);
}
