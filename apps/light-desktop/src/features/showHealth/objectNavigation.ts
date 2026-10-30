import { useEffect } from "react";
import { useActiveShowId } from "../deskSnapshot/DeskSnapshotState";
import {
	consumeObjectEditorRequest,
	currentObjectEditorRequest,
	subscribeObjectEditorRequest,
	type ObjectEditorRequest,
} from "../controlSurfaceInteraction/objectEditorRequest";

export function useHealthObjectNavigation(
	kind: ObjectEditorRequest["kind"],
	active: boolean,
	showId: string | null,
	open: (request: ObjectEditorRequest) => boolean,
) {
	useEffect(() => {
		if (!active || !showId) return;
		const accept = (request: ObjectEditorRequest) => {
			if (request.kind !== kind) return;
			if ((request.showId && request.showId !== showId) || open(request))
				consumeObjectEditorRequest(request);
		};
		const request = currentObjectEditorRequest();
		if (request) accept(request);
		return subscribeObjectEditorRequest(accept);
	}, [kind, active, showId, open]);
}

export function useHealthPresetNavigation(
	active: boolean,
	compact: boolean | undefined,
	showId: string | null,
	cards: readonly ({ id: string } | null)[],
	activate: (index: number, configure?: boolean) => unknown,
) {
	useHealthObjectNavigation("preset", active && !compact, showId, (request) => {
		const index = cards.findIndex((card) => card?.id === request.objectId);
		if (index < 0) return false;
		activate(index, true);
		return true;
	});
}

export function useHealthPatchTab(
	active: boolean,
	compact: boolean,
	setTab: (tab: "fixtures") => void,
) {
	const showId = useActiveShowId();
	useEffect(() => {
		if (!active || compact || !showId) return;
		const reveal = (request: ObjectEditorRequest) => {
			if (
				request.kind === "fixture" &&
				(!request.showId || request.showId === showId)
			)
				setTab("fixtures");
		};
		const request = currentObjectEditorRequest();
		if (request) reveal(request);
		return subscribeObjectEditorRequest(reveal);
	}, [active, compact, showId, setTab]);
}
