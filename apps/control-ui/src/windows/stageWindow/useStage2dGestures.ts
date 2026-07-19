import {
	type MouseEvent as ReactMouseEvent,
	type PointerEvent as ReactPointerEvent,
	useRef,
	useState,
} from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import type { StageMode } from "../../types";
import type { StageLayoutModel } from "./types";

type Point = { x: number; y: number };
type Marquee = { left: number; top: number; width: number; height: number };

export function useStageFixtureGestures(
	mode: StageMode,
	orderedFixtureIds: string[],
	layout: StageLayoutModel,
) {
	const server = useServer();
	const selectionAnchor = useRef<string | null>(null);
	const [draggingFixture, setDraggingFixture] = useState<string | null>(null);
	const select = (
		fixtureId: string,
		event: ReactMouseEvent<HTMLButtonElement>,
	) => {
		if (!fixtureId || mode !== "select") return;
		const anchor = selectionAnchor.current;
		if (event.shiftKey && anchor) {
			const from = orderedFixtureIds.indexOf(anchor);
			const to = orderedFixtureIds.indexOf(fixtureId);
			if (from >= 0 && to >= 0) {
				const members = orderedFixtureIds.slice(
					Math.min(from, to),
					Math.max(from, to) + 1,
				);
				for (const member of members)
					void server.selectionGesture({
						type: "fixture",
						fixture_id: member,
					});
			}
		} else {
			const toggled = event.ctrlKey || event.metaKey;
			void server.selectionGesture(
				{ type: "fixture", fixture_id: fixtureId },
				toggled && server.selectedFixtures.includes(fixtureId),
			);
		}
		selectionAnchor.current = fixtureId;
	};
	const beginMove = (
		fixtureId: string,
		event: ReactPointerEvent<HTMLButtonElement>,
	) => {
		if (mode !== "setup" || !fixtureId) return;
		event.currentTarget.setPointerCapture(event.pointerId);
		setDraggingFixture(fixtureId);
	};
	const move = (
		fixtureId: string,
		event: ReactPointerEvent<HTMLButtonElement>,
	) => {
		if (mode !== "setup" || draggingFixture !== fixtureId) return;
		const bounds = event.currentTarget.parentElement?.getBoundingClientRect();
		if (!bounds) return;
		layout.updatePosition2d(fixtureId, {
			x: Math.max(
				2,
				Math.min(94, ((event.clientX - bounds.left) / bounds.width) * 100),
			),
			y: Math.max(
				3,
				Math.min(90, ((event.clientY - bounds.top) / bounds.height) * 100),
			),
			rotation: layout.positions[fixtureId]?.rotation ?? 0,
		});
	};
	const finishMove = () => {
		if (draggingFixture) void layout.save();
		setDraggingFixture(null);
	};
	return { select, beginMove, move, finishMove };
}

function marqueeHits(
	host: HTMLDivElement,
	left: number,
	right: number,
	top: number,
	bottom: number,
) {
	return Array.from(
		host.querySelectorAll<HTMLElement>(".stage-fixture[data-fixture-id]"),
	)
		.filter((node) => {
			const box = node.getBoundingClientRect();
			return (
				box.right >= left &&
				box.left <= right &&
				box.bottom >= top &&
				box.top <= bottom
			);
		})
		.map((node) => node.dataset.fixtureId ?? "")
		.filter(Boolean);
}

export function useStageCanvasGestures(mode: StageMode) {
	const server = useServer();
	const { state, dispatch } = useApp();
	const navigationStart = useRef<
		(Point & { panX: number; panY: number }) | null
	>(null);
	const marqueeStart = useRef<(Point & { additive: boolean }) | null>(null);
	const [marquee, setMarquee] = useState<Marquee | null>(null);
	const begin = (event: ReactPointerEvent<HTMLDivElement>) => {
		if ((event.target as HTMLElement).closest(".stage-fixture")) return;
		if (mode === "navigate") {
			event.currentTarget.setPointerCapture(event.pointerId);
			navigationStart.current = {
				x: event.clientX,
				y: event.clientY,
				panX: state.stagePanX,
				panY: state.stagePanY,
			};
		} else if (mode === "select" && event.button === 0) {
			const bounds = event.currentTarget.getBoundingClientRect();
			event.currentTarget.setPointerCapture(event.pointerId);
			marqueeStart.current = {
				x: event.clientX,
				y: event.clientY,
				additive: event.ctrlKey || event.metaKey,
			};
			setMarquee({
				left: event.clientX - bounds.left,
				top: event.clientY - bounds.top,
				width: 0,
				height: 0,
			});
		}
	};
	const update = (event: ReactPointerEvent<HTMLDivElement>) => {
		if (navigationStart.current && mode === "navigate")
			dispatch({
				type: "SET_STAGE_NAVIGATION",
				panX:
					navigationStart.current.panX +
					event.clientX -
					navigationStart.current.x,
				panY:
					navigationStart.current.panY +
					event.clientY -
					navigationStart.current.y,
			});
		const start = marqueeStart.current;
		if (!start || mode !== "select") return;
		const bounds = event.currentTarget.getBoundingClientRect();
		setMarquee({
			left: Math.min(start.x, event.clientX) - bounds.left,
			top: Math.min(start.y, event.clientY) - bounds.top,
			width: Math.abs(event.clientX - start.x),
			height: Math.abs(event.clientY - start.y),
		});
	};
	const finish = (event: ReactPointerEvent<HTMLDivElement>) => {
		navigationStart.current = null;
		const start = marqueeStart.current;
		marqueeStart.current = null;
		if (!start) return setMarquee(null);
		const left = Math.min(start.x, event.clientX);
		const right = Math.max(start.x, event.clientX);
		const top = Math.min(start.y, event.clientY);
		const bottom = Math.max(start.y, event.clientY);
		if (right - left >= 4 || bottom - top >= 4) {
			for (const fixtureId of marqueeHits(
				event.currentTarget,
				left,
				right,
				top,
				bottom,
			))
				void server.selectionGesture(
					{ type: "fixture", fixture_id: fixtureId },
					start.additive && server.selectedFixtures.includes(fixtureId),
				);
		} else if (!start.additive) void server.setSelection([]);
		setMarquee(null);
	};
	const cancel = () => {
		navigationStart.current = null;
		marqueeStart.current = null;
		setMarquee(null);
	};
	return {
		begin,
		update,
		finish,
		cancel,
		marquee,
		pan: { x: state.stagePanX, y: state.stagePanY },
		zoom: state.stageZoom,
	};
}
