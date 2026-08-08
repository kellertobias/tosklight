// The server resources this application reads, each with one stable cache key.
//
// The keys live here so an optimistic write and the panel it updates cannot disagree about which
// entry they are touching. How *often* each is read is a feature decision and travels as an
// argument.

import { api } from "./client";
import type { CatalogView, Health, OutputView } from "./generated/media-wire";
import { type Resource, useResource } from "./resource";

export const KEYS = {
	health: "health",
	catalog: "catalog",
	outputs: "outputs",
} as const;

export function useHealth(pollMs?: number): Resource<Health> {
	return useResource(KEYS.health, api.health, { pollMs });
}

export function useCatalog(pollMs?: number): Resource<CatalogView> {
	return useResource(KEYS.catalog, api.catalog, { pollMs });
}

export function useOutputs(pollMs?: number): Resource<OutputView[]> {
	return useResource(KEYS.outputs, api.outputs, { pollMs });
}
