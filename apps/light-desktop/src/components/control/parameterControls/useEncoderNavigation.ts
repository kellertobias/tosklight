import { useCallback, useId, useSyncExternalStore } from "react";
import { type ParameterFamily, parameterFamilyOrder } from "./model";

export interface EncoderNavigation {
	family: ParameterFamily;
	page: number;
	dynamicsMode: boolean;
	ownerContext: EncoderOwnerContext;
}
export type EncoderOwnerContext = "programmer" | "stage" | "timecode" | "patch" | "dynamic-editor";
const contexts: readonly EncoderOwnerContext[] = ["programmer", "stage", "timecode", "patch", "dynamic-editor"];
const initial: EncoderNavigation = { family: "Intensity", page: 1, dynamicsMode: false, ownerContext: "programmer" };
const snapshots = new Map<string, EncoderNavigation>();
const listeners = new Set<() => void>();
const prefix = "tosklight:encoder-navigation:";
const contextSuffix = ":owner-context";

export function parseEncoderNavigation(input: string | null): EncoderNavigation | null {
	try {
		const value = JSON.parse(input ?? "null");
		return value && parameterFamilyOrder.includes(value.family) &&
			Number.isInteger(value.page) && value.page > 0
			? { family: value.family, page: value.page, dynamicsMode: value.dynamicsMode === true, ownerContext: contexts.includes(value.ownerContext) ? value.ownerContext : "programmer" } : null;
	} catch {
		return null;
	}
}

function read(key: string) {
	if (!snapshots.has(key)) {
		let value: EncoderNavigation | null = null;
		try {
			value = parseEncoderNavigation(localStorage.getItem(key));
			const ownerContext = JSON.parse(localStorage.getItem(key + contextSuffix) ?? "null");
			if (contexts.includes(ownerContext)) value = { ...(value ?? initial), ownerContext };
		} catch { /* Storage can be unavailable in private windows. */ }
		snapshots.set(key, value ?? initial);
	}
	return snapshots.get(key)!;
}

function onStorage(event: StorageEvent) {
	if (!event.key?.startsWith(prefix)) return;
	if (event.key.endsWith(contextSuffix)) {
		let ownerContext: unknown;
		try { ownerContext = JSON.parse(event.newValue ?? "null"); } catch { return; }
		if (!contexts.includes(ownerContext as EncoderOwnerContext)) return;
		const key = event.key.slice(0, -contextSuffix.length);
		snapshots.set(key, { ...read(key), ownerContext: ownerContext as EncoderOwnerContext });
		for (const listener of listeners) listener();
		return;
	}
	const value = parseEncoderNavigation(event.newValue);
	if (!value) return;
	// A mirror's family/page write must not overwrite newer owner context while a
	// cross-window storage event is still in flight. Context has its own owner-written key.
	snapshots.set(event.key, { ...value, ownerContext: read(event.key).ownerContext });
	for (const listener of listeners) listener();
}

function subscribe(listener: () => void) {
	if (!listeners.size) window.addEventListener("storage", onStorage);
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
		if (!listeners.size) window.removeEventListener("storage", onStorage);
	};
}

function update(key: string, next: EncoderNavigation) {
	const previous = read(key);
	if (previous.family === next.family && previous.page === next.page && previous.dynamicsMode === next.dynamicsMode && previous.ownerContext === next.ownerContext) return;
	snapshots.set(key, next);
	try {
		localStorage.setItem(key, JSON.stringify({ family: next.family, page: next.page, dynamicsMode: next.dynamicsMode }));
	} catch { /* Same-window navigation still works without storage. */ }
	for (const listener of listeners) listener();
}

export function programmerEncoderOwnerContext(programmer: boolean, builtIn: string | null | undefined): EncoderOwnerContext {
	return programmer ? "programmer" : builtIn === "patch" ? "patch" : builtIn === "timecode" ? "timecode" : "stage";
}

/** Shared UI navigation only; programmer values and writes remain server authoritative. */
export function useEncoderNavigation(deskId: string | null) {
	const localId = useId();
	const key = prefix + (deskId ?? `unconnected:${localId}`);
	const snapshot = useSyncExternalStore(subscribe, () => read(key), () => initial);
	const select = useCallback((family: ParameterFamily, page: number) => {
		const previous = read(key);
		update(key, { ...previous, family, page, dynamicsMode: false });
	}, [key]);
	const setDynamicsMode = useCallback((value: boolean | ((current: boolean) => boolean)) => {
		const previous = read(key);
		update(key, { ...previous, dynamicsMode: typeof value === "function" ? value(previous.dynamicsMode) : value });
	}, [key]);
	const publishOwnerContext = useCallback((ownerContext: EncoderOwnerContext) => {
		const previous = read(key);
		if (previous.ownerContext === ownerContext) return;
		snapshots.set(key, { ...previous, ownerContext });
		try { localStorage.setItem(key + contextSuffix, JSON.stringify(ownerContext)); } catch { /* Owner context still reaches same-window mirrors. */ }
		for (const listener of listeners) listener();
	}, [key]);
	return [snapshot, select, { setDynamicsMode, publishOwnerContext }] as const;
}
