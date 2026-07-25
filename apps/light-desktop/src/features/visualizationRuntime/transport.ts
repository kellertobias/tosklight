import type { VisualizationSnapshot } from "../../api/types";
import type {
	VisualizationRuntimeLane,
	VisualizationRuntimeScope,
} from "./contracts";

export interface VisualizationRuntimeTransport {
	loadSnapshot(
		scope: VisualizationRuntimeScope,
		lane: VisualizationRuntimeLane,
	): Promise<VisualizationSnapshot>;
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
