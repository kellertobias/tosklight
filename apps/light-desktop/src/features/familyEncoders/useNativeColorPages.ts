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
 */
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
		const ids = key.split("|")[0];
		load(ids ? ids.split(",") : [], reference).then(
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
		// `reference` is part of `key`.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [active, load, key, refreshKey]);
	return active && load && state?.key === key ? state.snapshot : null;
}
