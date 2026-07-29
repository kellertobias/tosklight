export interface SlowVisualizationClient {
	close(): void;
}

export function startSlowVisualizationClient(
	baseUrl: string,
	token: string,
): Promise<SlowVisualizationClient>;
