import {
	Button, ErrorAlert, FormLayout, ModalPortal, ModalTitleBar,
	NumberField, SelectField,
} from "@tosklight/ui";
import { useState } from "react";
import type { PlaybackPageSource } from "../../api/types";
import { useScreens } from "../../features/screens/ScreensContext";
import { PlaybackSoloSettings } from "./PlaybackSoloSettings";

export function PlaybackSectionSettingsModal({ surfaceId, onClose, onShowGrid, preparationError }: {
	surfaceId: string;
	onClose(): void;
	onShowGrid?(): void;
	preparationError?: string | null;
}) {
	const screens = useScreens();
	const [tab, setTab] = useState<"page" | "solo">("page");
	const surface = screens.surfaces?.playback_surfaces[surfaceId];
	return (
		<ModalPortal onClose={onClose}>
			<div className="modal-backdrop" onPointerDown={(event) => {
				if (event.target === event.currentTarget) onClose();
			}}>
			<section className="modal-card" role="dialog" aria-label="Playback section settings">
				<ModalTitleBar title="Playback section settings" onClose={onClose} />
				<div role="tablist" aria-label="Playback section settings">
					<Button role="tab" aria-selected={tab === "page"} onClick={() => setTab("page")}>
						Page source
					</Button>
					<Button role="tab" aria-selected={tab === "solo"} onClick={() => { setTab("solo"); onShowGrid?.(); }}>
						Manage Solo Regions
					</Button>
				</div>
				{preparationError && <ErrorAlert role="alert">{preparationError}</ErrorAlert>}
				{tab === "solo" ? <PlaybackSoloSettings surfaceId={surfaceId} onClose={onClose} />
					: surface ? <PageSourceSettings key={surfaceId} surfaceId={surfaceId} initial={surface.page_source} />
						: <p role="status">Playback surface is unavailable.</p>}
			</section>
			</div>
		</ModalPortal>
	);
}

function PageSourceSettings({ surfaceId, initial }: {
	surfaceId: string;
	initial: PlaybackPageSource;
}) {
	const screens = useScreens();
	const [draft, setDraft] = useState(initial);
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const surfaces = Object.entries(screens.surfaces?.playback_surfaces ?? {})
		.filter(([id]) => id !== surfaceId);
	const save = async () => {
		setSaving(true);
		setError(null);
		try {
			await screens.updatePlaybackSurface(surfaceId, { page_source: draft });
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setSaving(false);
		}
	};
	return (
		<>
			<FormLayout labelPlacement="side">
				<SelectField label="Page source" ariaLabel="Page source" value={draft.type} disabled={saving}
					options={[
						{ value: "own", label: "Own page" },
						{ value: "pinned", label: "Pinned page" },
						{ value: "follow", label: "Follow surface" },
					]}
					onChange={(type) => setDraft(type === "follow"
						? { type, surface_id: surfaces[0]?.[0] ?? surfaceId }
						: {
							type: type as "own" | "pinned",
							page: draft.type === "follow"
								? screens.surfaces?.resolved_pages[surfaceId] ?? 1 : draft.page,
						})} />
				{draft.type === "follow"
					? <SelectField label="Follow surface" ariaLabel="Follow surface" value={draft.surface_id} disabled={saving}
						options={surfaces.map(([id, surface]) => ({ value: id, label: surface.name }))}
						onChange={(surface_id) => setDraft({ type: "follow", surface_id })} />
					: <NumberField label="Page" min="1" max="99" value={draft.page} disabled={saving}
						onChange={(event) => setDraft({ ...draft, page: Number(event.target.value) })} />}
			</FormLayout>
			<Button disabled={saving} onClick={() => void save()}>Save</Button>
			{error && <ErrorAlert role="alert">{error}</ErrorAlert>}
		</>
	);
}
