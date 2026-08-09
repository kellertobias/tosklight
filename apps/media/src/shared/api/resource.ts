// The one server-state mechanism the whole application uses.
//
// A media server has no session and no user-specific data: every panel is a projection of one
// authoritative snapshot. So the rule is simple — a resource is a key, a loader, and a poll
// interval; features own the interval and the invalidation, and nothing keeps a private copy of
// server state in a component.

import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { ApiFailure } from "./client";

/** What a consumer sees. `data` survives a later failure so a panel does not blank out. */
export interface Resource<T> {
	data: T | undefined;
	failure: ApiFailure | undefined;
	loading: boolean;
	/** True once a load has failed but stale data is still on screen. */
	stale: boolean;
	reload: () => void;
}

interface Entry<T> {
	data: T | undefined;
	failure: ApiFailure | undefined;
	loading: boolean;
	listeners: Set<() => void>;
	inFlight: Promise<void> | undefined;
	snapshot: Omit<Resource<T>, "reload">;
}

const entries = new Map<string, Entry<unknown>>();

function entryFor<T>(key: string): Entry<T> {
	let entry = entries.get(key) as Entry<T> | undefined;
	if (!entry) {
		entry = {
			data: undefined,
			failure: undefined,
			loading: false,
			listeners: new Set(),
			inFlight: undefined,
			snapshot: { data: undefined, failure: undefined, loading: false, stale: false },
		};
		entries.set(key, entry as Entry<unknown>);
	}
	return entry;
}

function publish<T>(entry: Entry<T>): void {
	// A fresh object identity per change is what `useSyncExternalStore` compares, and a stable
	// one between changes is what stops React re-rendering the whole page on every poll.
	entry.snapshot = {
		data: entry.data,
		failure: entry.failure,
		loading: entry.loading,
		stale: entry.failure !== undefined && entry.data !== undefined,
	};
	for (const listener of entry.listeners) listener();
}

async function load<T>(key: string, loader: () => Promise<T>): Promise<void> {
	const entry = entryFor<T>(key);
	if (entry.inFlight) return entry.inFlight;

	entry.loading = true;
	publish(entry);
	entry.inFlight = (async () => {
		try {
			entry.data = await loader();
			entry.failure = undefined;
		} catch (error) {
			entry.failure =
				error instanceof ApiFailure
					? error
					: new ApiFailure("unexpected-error", String(error), 0);
		} finally {
			entry.loading = false;
			entry.inFlight = undefined;
			publish(entry);
		}
	})();
	return entry.inFlight;
}

/** Replaces a resource's value without a round trip — used by an optimistic write. */
export function writeResource<T>(key: string, data: T): void {
	const entry = entryFor<T>(key);
	entry.data = data;
	entry.failure = undefined;
	publish(entry);
}

/** Drops a resource so the next reader loads it again. */
export function invalidateResource(key: string): void {
	const entry = entries.get(key);
	if (!entry) return;
	entry.data = undefined;
	publish(entry);
}

/** Forgets everything. Only tests need this; a running server never does. */
export function resetResources(): void {
	entries.clear();
}

export interface ResourceOptions {
	/** How often to re-read, in milliseconds. Omit for a resource that only changes on request. */
	pollMs?: number;
}

export function useResource<T>(
	key: string,
	loader: () => Promise<T>,
	options: ResourceOptions = {},
): Resource<T> {
	const entry = entryFor<T>(key);
	const loaderRef = useRef(loader);
	loaderRef.current = loader;

	const snapshot = useSyncExternalStore(
		useCallback(
			(listener: () => void) => {
				entry.listeners.add(listener);
				return () => entry.listeners.delete(listener);
			},
			[entry],
		),
		() => entry.snapshot,
	);

	const reload = useCallback(() => {
		void load(key, () => loaderRef.current());
	}, [key]);

	const { pollMs } = options;
	useEffect(() => {
		reload();
		if (!pollMs) return;
		const timer = setInterval(reload, pollMs);
		return () => clearInterval(timer);
	}, [reload, pollMs]);

	return { ...snapshot, reload };
}
