import type { OutputReadoutSnapshot } from "../../api/familyEncoderModels";
import type { VisualizationSnapshot } from "../../api/types";
import type {
	VisualizationRuntimeLane,
	VisualizationRuntimeScope,
} from "./contracts";

export interface VisualizationRuntimeTransport {
	loadSnapshot(
		scope: VisualizationRuntimeScope,
		lane: VisualizationRuntimeLane,
		options?: { dynamicStackOnly?: boolean; fixtureIds?: readonly string[] },
	): Promise<VisualizationSnapshot>;
	openStream?(
		scope: VisualizationRuntimeScope,
		observer: VisualizationRuntimeStreamObserver,
	): VisualizationRuntimeStream;
}

export interface VisualizationRuntimeStreamObserver {
	snapshot(
		lane: VisualizationRuntimeLane,
		snapshot: VisualizationSnapshot,
	): void;
	error(error: Error): void;
	/**
	 * TL-594: typed readouts of the claimed owners, from the same accepted source and with the
	 * same lease as the Normal lane message of that publication.
	 */
	readouts?(snapshot: OutputReadoutSnapshot, sourceFrame: number): void;
}

export interface VisualizationRuntimeStream {
	updateClaims(
		lanes: readonly VisualizationRuntimeLane[],
		maxRateHz: number,
		includeDynamicStack?: boolean,
		/** Every resolved attribute rather than only those the Stage draws (Preset pools). */
		completeValues?: boolean,
	): void;
	/**
	 * Replaces the readout claim sent with the next Subscribe (`null` or empty clears it). The
	 * server answers on the Normal lane only, so a claim needs a Normal lane claim to deliver.
	 */
	updateReadoutClaim?(fixtureIds: readonly string[] | null): void;
	close(): void;
}

/** The v1 adapter returned data outside the exact requested authority or lane. */
export class VisualizationRuntimeProtocolError extends Error {
	constructor(message: string) {
		super(message);
		this.name = "VisualizationRuntimeProtocolError";
	}
}

export class VisualizationRuntimeHttpError extends Error {
	constructor(
		message: string,
		readonly status: number,
	) {
		super(message);
		this.name = "VisualizationRuntimeHttpError";
	}
}
