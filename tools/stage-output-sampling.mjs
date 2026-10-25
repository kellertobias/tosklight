/** Counter-only benchmark reads use the typed performance endpoint, which returns the same
 * RuntimeOutputHealth as full diagnostics without serializing every programmer definition.
 * Keep reads and window closure in one FIFO: a delayed observation must precede its close.
 */
export function queueStageOutputSample(previous, requestJson, session, consume) {
	return previous.then(async () => {
		const snapshot = await requestJson(
			"GET",
			"/api/v2/diagnostics/performance",
			undefined,
			{ session },
		);
		return consume(snapshot.output);
	});
}
