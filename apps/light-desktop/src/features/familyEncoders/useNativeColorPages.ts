import { useEffect, useState } from "react";
import type { NativeColorPagesSnapshot } from "../../api/nativeColorModels";
import {
	boundedFamilyFixtureIds,
	useFamilyEncodersContext,
} from "./FamilyEncodersProvider";
import { useNativeColorReferenceChoice } from "./nativeColorReference";

/**
 * TL-554: the Direct Color pages of `fixtureIds` for the current reference choice. An inert
 * read, re-read when the selection, the reference choice or `refreshKey` changes. `null` while
 * unknown, inactive, without a provider or after a failed read: the pages are then absent
 * (never guessed), and the semantic pages 1/2 are unaffected.
 *
 * TL-653: a snapshot that names a reference head but carries no current values (no accepted
 * frame of this generation yet, for example right after a show load) is read again, every
 * {@link NATIVE_VALUES_RETRY_MILLIS} up to {@link NATIVE_VALUES_RETRIES} times, so the Direct
 * encoders show the head's real values instead of staying a dash.
 */
export const NATIVE_VALUES_RETRIES = 20;
export const NATIVE_VALUES_RETRY_MILLIS = 250;

export function useNativeColorPages(
	fixtureIds: readonly string[],
	active: boolean,
	refreshKey?: unknown,
): NativeColorPagesSnapshot | null {
	const context = useFamilyEncodersContext();
	const reference = useNativeColorReferenceChoice();
	const key = `${boundedFamilyFixtureIds(fixtureIds).join(",")}|${reference?.fixtureId ?? ""}|${reference?.headId ?? ""}`;
	const [state, setState] = useState<{
		key: string;
		snapshot: NativeColorPagesSnapshot;
	} | null>(null);
	const load = context?.loadNativePages;
	useEffect(() => {
		if (!active || !load) return;
		let current = true;
		let retry: ReturnType<typeof setTimeout> | undefined;
		const ids = key.split("|")[0];
		const read = (attempt: number) =>
			load(ids ? ids.split(",") : [], reference).then(
				(snapshot) => {
					if (!current) return;
					setState({ key, snapshot });
					if (snapshot.reference && !snapshot.values && attempt < NATIVE_VALUES_RETRIES)
						retry = setTimeout(() => void read(attempt + 1), NATIVE_VALUES_RETRY_MILLIS);
				},
				() => {
					if (current) setState(null);
				},
			);
		void read(0);
		return () => {
			current = false;
			if (retry !== undefined) clearTimeout(retry);
		};
		// `reference` is part of `key`.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [active, load, key, refreshKey]);
	return active && load && state?.key === key ? state.snapshot : null;
}
