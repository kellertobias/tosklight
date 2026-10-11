import {
	Button,
	ErrorAlert,
	FormLayout,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import { useEffect, useState } from "react";
import { usePreloadScope } from "../../features/playbackPreloadScope/usePreloadScope";
import {
	parsePlaybackPreloadScope,
	preloadScopeText,
} from "../../features/playbackPreloadScope/scopeText";
export function PlaybackPreloadScopeSettings() {
	const model = usePreloadScope();
	const [draft, setDraft] = useState<{
		key: string;
		edited: ReturnType<typeof preloadScopeText> | null;
	}>({ key: model.key, edited: null });
	if (draft.key !== model.key) setDraft({ key: model.key, edited: null });
	const edited = draft.key === model.key ? draft.edited : null;
	const [inputError, setInputError] = useState<string | null>(null);
	const dirty = edited !== null;
	const text = dirty
		? edited
		: model.snapshot
			? preloadScopeText(model.snapshot.scope)
			: { positions: "1-1000", allPages: true, pages: "" };
	const { positions, allPages, pages } = text;
	const edit = (change: Partial<ReturnType<typeof preloadScopeText>>) =>
		setDraft({ key: model.key, edited: { ...text, ...change } });
	useEffect(() => setInputError(null), [model.key]);
	const save = async () => {
		try {
			setInputError(null);
			const scope = parsePlaybackPreloadScope(positions, allPages, pages);
			const key = model.key;
			if (await model.save(scope))
				setDraft((current) =>
					current.key === key ? { key, edited: null } : current,
				);
		} catch (e) {
			setInputError(e instanceof Error ? e.message : String(e));
		}
	};
	if (!model.available)
		return <p>Playback eligibility becomes available with an active Show.</p>;
	return (
		<section aria-label="Playback Preload eligibility">
			<p>
				Preload keeps the existing action rules. These positions are eligible on
				all pages unless restricted below.
			</p>
			<FormLayout labelPlacement="side">
				<TextField
					label="Playback position ranges"
					value={positions}
					disabled={!model.snapshot || model.saving}
					onChange={(e) => edit({ positions: e.target.value })}
				/>
				<SwitchField
					label="All pages"
					offLabel="Restricted pages"
					onLabel="All pages"
					checked={allPages}
					disabled={!model.snapshot || model.saving}
					onChange={(e) => edit({ allPages: e.target.checked })}
				/>
				{!allPages && (
					<TextField
						label="Eligible pages"
						value={pages}
						disabled={model.saving}
						onChange={(e) => edit({ pages: e.target.value })}
					/>
				)}
			</FormLayout>
			<p>
				Leave position ranges empty to ignore playback actions. A restricted
				empty page list makes no pages eligible.
			</p>
			<p>
				Use comma-separated numbers or ranges, for example 1-15,101-115. Pages
				are 1–99; positions are 1–1000.
			</p>
			<Button
				disabled={!dirty || !model.snapshot || model.saving}
				onClick={() => void save()}
			>
				{model.saving ? "Saving eligibility…" : "Save playback eligibility"}
			</Button>
			{(inputError || model.error) && (
				<ErrorAlert role="alert">{inputError || model.error}</ErrorAlert>
			)}
			{!model.snapshot && !model.error && <p>Loading playback eligibility…</p>}
		</section>
	);
}
