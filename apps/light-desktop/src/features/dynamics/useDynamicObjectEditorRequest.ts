import { useEffect } from "react";
import {
	consumeObjectEditorRequest,
	currentObjectEditorRequest,
	subscribeObjectEditorRequest,
} from "../controlSurfaceInteraction/objectEditorRequest";
import type { ShowObject } from "../showObjects/contracts";
import type { DynamicEditorSession } from "./DynamicEditorSessionContext";

export function useDynamicObjectEditorRequest(
	active: boolean,
	dynamics: readonly ShowObject<"dynamic">[],
	openEditor: (session: DynamicEditorSession) => void,
	select: (id: string) => void,
) {
	useEffect(() => {
		if (!active) return;
		const openRequested = (
			request: NonNullable<ReturnType<typeof currentObjectEditorRequest>>,
		) => {
			if (request.kind !== "dynamic") return;
			const dynamic = dynamics.find((item) => item.id === request.objectId);
			if (!dynamic) return;
			openEditor({
				dynamicId: dynamic.id,
				task: "curves",
				encoderPage: 1,
				primaryLaneId: dynamic.body.lanes[0]?.id ?? null,
				primaryKeyframeIndex: 0,
			});
			select(dynamic.id);
			consumeObjectEditorRequest(request);
		};
		const request = currentObjectEditorRequest();
		if (request) openRequested(request);
		return subscribeObjectEditorRequest(openRequested);
	}, [active, dynamics, openEditor, select]);
}
