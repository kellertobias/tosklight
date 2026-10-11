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
	const [positions, setPositions] = useState("1-1000"),
		[allPages, setAllPages] = useState(true),
		[pages, setPages] = useState("");
	const [dirty, setDirty] = useState(false),
		[inputError, setInputError] = useState<string | null>(null);
	useEffect(() => {
		setDirty(false);
		setInputError(null);
	}, [model.key]);
	useEffect(() => {
		if (model.snapshot && !dirty) {
			const text = preloadScopeText(model.snapshot.scope);
			setPositions(text.positions);
			setAllPages(text.allPages);
			setPages(text.pages);
		}
	}, [model.snapshot, dirty]);
	const save = async () => {
		try {
			setInputError(null);
			const scope = parsePlaybackPreloadScope(positions, allPages, pages);
			if (await model.save(scope)) setDirty(false);
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
					onChange={(e) => {
						setPositions(e.target.value);
						setDirty(true);
					}}
				/>
				<SwitchField
					label="All pages"
					offLabel="Restricted pages"
					onLabel="All pages"
					checked={allPages}
					disabled={!model.snapshot || model.saving}
					onChange={(e) => {
						setAllPages(e.target.checked);
						setDirty(true);
					}}
				/>
				{!allPages && (
					<TextField
						label="Eligible pages"
						value={pages}
						disabled={model.saving}
						onChange={(e) => {
							setPages(e.target.value);
							setDirty(true);
						}}
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
