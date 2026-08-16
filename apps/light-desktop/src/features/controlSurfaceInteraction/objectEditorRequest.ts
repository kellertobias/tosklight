export interface ObjectEditorRequest {
	kind: "macro" | "timecode";
	objectId: string;
}

let current: ObjectEditorRequest | null = null;
const listeners = new Set<(request: ObjectEditorRequest) => void>();

export function publishObjectEditorRequest(request: ObjectEditorRequest) {
	current = request;
	for (const listener of listeners) listener(request);
}

export function currentObjectEditorRequest() {
	return current;
}

export function consumeObjectEditorRequest(request: ObjectEditorRequest) {
	if (current === request) current = null;
}

export function subscribeObjectEditorRequest(
	listener: (request: ObjectEditorRequest) => void,
) {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
}

export function resetObjectEditorRequestsForTests() {
	current = null;
	listeners.clear();
}
