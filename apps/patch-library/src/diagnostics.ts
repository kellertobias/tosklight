/**
 * Optional performance instrumentation for one patch mutation.
 *
 * A desk records these phases so a slow patch can be attributed without changing patch command or
 * event semantics. Hosts that do not measure anything pass nothing and pay for a few no-op calls.
 */
export interface PatchMutationSample {
	/** The optimistic fixtures reached the store, before the server answered. */
	optimisticStorePublished(): void;
	/** The server response finished decoding. */
	responseDecoded(): void;
	/** The authoritative server projection replaced the optimistic one. */
	authoritativeStorePublished(): void;
	/** The change became visible to the operator. */
	visiblePainted(): void;
}

export interface PatchDiagnostics {
	beginPatchMutation(
		requestId: string,
		fixtureCount: number,
	): PatchMutationSample;
}

const noSample: PatchMutationSample = {
	optimisticStorePublished: () => undefined,
	responseDecoded: () => undefined,
	authoritativeStorePublished: () => undefined,
	visiblePainted: () => undefined,
};

export const noPatchDiagnostics: PatchDiagnostics = {
	beginPatchMutation: () => noSample,
};
