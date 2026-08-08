// The server resources this application reads, each with one stable cache key.
//
// The keys live here so an optimistic write and the panel it updates cannot disagree about which
// entry they are touching. How *often* each is read is a feature decision and travels as an
// argument.

import { api } from "./client";
import type {
	AudioPanelView,
	CatalogView,
	Health,
	NetworkView,
	OutputView,
	TextSlotView,
	VisualizerView,
} from "./generated/media-wire";
import { type Resource, useResource } from "./resource";

export const KEYS = {
	health: "health",
	catalog: "catalog",
	outputs: "outputs",
	visualizers: "visualizers",
	network: "network",
	audio: "audio",
	text: "text",
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

/// Configuration, not state: it changes when an operator reassigns an address, so it is not polled.
export function useVisualizers(): Resource<VisualizerView[]> {
	return useResource(KEYS.visualizers, api.visualizers);
}

/// Configuration. It changes when an operator saves it, so it is read once.
export function useNetwork(): Resource<NetworkView> {
	return useResource(KEYS.network, api.network);
}

export function useText(): Resource<TextSlotView[]> {
	return useResource(KEYS.text, api.text);
}

/**
 * The audio settings, with one analysis for a panel to draw before its socket is up.
 *
 * The analysis in this snapshot is deliberately not polled — the telemetry socket carries it after
 * the first paint, and polling a meter would spend a show's worth of requests on it.
 */
export function useAudio(): Resource<AudioPanelView> {
	return useResource(KEYS.audio, api.audio);
}
