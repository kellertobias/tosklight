import {
	createContext,
	type PropsWithChildren,
	useContext,
	useEffect,
	useMemo,
	useState,
} from "react";
import type { FamilyEncoderApiClient } from "../../api/client/familyEncoders";
import { MAX_FAMILY_ENCODER_FIXTURES } from "../../api/familyEncoderPagesWire";
import type { FamilyEncoderPagesSnapshot } from "../../api/familyEncoderModels";
import type { NativeColorPagesSnapshot } from "../../api/nativeColorModels";
import type { NativeColorReferenceChoice } from "../../api/nativeColorPagesWire";
import { useColorPresentation } from "../configuration/ConfigurationState";
import { DisplayedSourceReadouts } from "../programmerValues/displayedSource";
import type { VisualizationRuntimeSession } from "../visualizationRuntime/session";
import { useVisualizationRuntimeSession } from "../visualizationRuntime/VisualizationRuntimeView";

/**
 * Desk-session boundary for the semantic family encoders (TL-549/550/551 UI foundation).
 *
 * It owns one `DisplayedSourceReadouts` per desk session and Show, so every surface (encoders,
 * the family modals, hardware/OSC encoders) names the same newest lease per lane, and it reads
 * the server's per-selection family pages. Without the provider (stories, isolated tests) every
 * hook reports "not semantic" and the legacy normalized pages stay in force.
 */
export interface FamilyEncodersContextValue {
	loadPages(fixtureIds: readonly string[]): Promise<FamilyEncoderPagesSnapshot>;
	/** TL-554: Direct Color pages 3/4 and overflow of the selection's reference head. */
	loadNativePages?(
		fixtureIds: readonly string[],
		reference: NativeColorReferenceChoice | null,
	): Promise<NativeColorPagesSnapshot>;
	readouts: DisplayedSourceReadouts;
	/** The scoped visualization session whose stream carries Normal-lane readout claims. */
	session: VisualizationRuntimeSession | null;
}

const FamilyEncodersContext = createContext<FamilyEncodersContextValue | null>(
	null,
);

export function FamilyEncodersProvider({
	children,
	client,
	showId,
	enabled,
}: PropsWithChildren<{
	client: FamilyEncoderApiClient;
	showId: string | null;
	enabled: boolean;
}>) {
	const session = useVisualizationRuntimeSession();
	const readouts = useMemo(
		() =>
			enabled && showId
				? new DisplayedSourceReadouts({
						request: client.readoutRequest,
						showId: () => showId,
					})
				: null,
		[client, enabled, showId],
	);
	const value = useMemo<FamilyEncodersContextValue | null>(
		() =>
			readouts && showId
				? {
						loadPages: (fixtureIds) => client.pages(fixtureIds, showId),
						loadNativePages: (fixtureIds, reference) =>
							client.nativeColorPages(fixtureIds, showId, reference),
						readouts,
						session,
					}
				: null,
		[client, readouts, session, showId],
	);
	return (
		<FamilyEncodersContext.Provider value={value}>
			{children}
		</FamilyEncodersContext.Provider>
	);
}

/** Test and story seam: mount an explicit context value. */
export const FamilyEncodersContextProvider = FamilyEncodersContext.Provider;

export function useFamilyEncodersContext() {
	return useContext(FamilyEncodersContext);
}

/** Bounds a selection to what one request may carry, keeping selection order. */
export function boundedFamilyFixtureIds(fixtureIds: readonly string[]) {
	return fixtureIds.length > MAX_FAMILY_ENCODER_FIXTURES
		? fixtureIds.slice(0, MAX_FAMILY_ENCODER_FIXTURES)
		: fixtureIds;
}

/**
 * The server's family pages for `fixtureIds`, re-read when the selection or the desk's Color
 * presentation changes. `null` while unknown, inactive or without a provider; a failed read also
 * reports `null`, which keeps the legacy pages (never a guessed semantic layout).
 */
export function useFamilyEncoderPages(
	fixtureIds: readonly string[],
	active: boolean,
): FamilyEncoderPagesSnapshot | null {
	const context = useFamilyEncodersContext();
	const presentation = useColorPresentation();
	const key = boundedFamilyFixtureIds(fixtureIds).join(",");
	const [state, setState] = useState<{
		key: string;
		snapshot: FamilyEncoderPagesSnapshot;
	} | null>(null);
	useEffect(() => {
		if (!active || !context) return;
		let current = true;
		const ids = key ? key.split(",") : [];
		context.loadPages(ids).then(
			(snapshot) => {
				if (current) setState({ key, snapshot });
			},
			() => {
				if (current) setState(null);
			},
		);
		return () => {
			current = false;
		};
	}, [active, context, key, presentation]);
	return active && context && state?.key === key ? state.snapshot : null;
}

/** Whether the backend reports the semantic programming contract active for this selection. */
export function useSemanticFamilyEncoders(
	fixtureIds: readonly string[],
	active: boolean,
) {
	return useFamilyEncoderPages(fixtureIds, active)?.semantic === true;
}
