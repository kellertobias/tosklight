export interface ShowHandle {
	readonly name: string;
}

export interface ShowRevisionCopy {
	show_id: string;
	show_name: string;
	revision: number;
	revision_name: string;
	copied_at: string;
}

export interface ActiveShow {
	id: string;
	name: string;
	revision: number;
	revision_copy?: ShowRevisionCopy;
}

export interface ResolvedShowTarget {
	readonly name: string;
	readonly id?: string;
}

const handles = new WeakMap<ShowHandle, ResolvedShowTarget>();

export function showHandle(target: ResolvedShowTarget): ShowHandle {
	const handle = Object.freeze({ name: target.name });
	handles.set(handle, target);
	return handle;
}

export function resolveShowHandle(handle: ShowHandle): ResolvedShowTarget {
	const target = handles.get(handle);
	if (!target) {
		throw new Error(
			`"${handle.name}" is not a show handle created by this scenario world`,
		);
	}
	return target;
}
