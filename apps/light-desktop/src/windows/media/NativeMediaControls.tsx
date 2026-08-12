import { Button, TextAreaField } from "@tosklight/ui";
import { useEffect, useState } from "react";
import type {
	NativeMediaSnapshot,
	NativeMediaTextSlot,
} from "../../api/client/mediaOutput";

interface NativeMediaControlsProps {
	fixtureId: string;
	load(fixtureId: string): Promise<NativeMediaSnapshot>;
	updateText(
		fixtureId: string,
		folder: number,
		file: number,
		text: string,
	): Promise<NativeMediaTextSlot>;
}

export function NativeMediaControls({
	fixtureId,
	load,
	updateText,
}: NativeMediaControlsProps) {
	const [snapshot, setSnapshot] = useState<NativeMediaSnapshot | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [refresh, setRefresh] = useState(0);

	useEffect(() => {
		let current = true;
		setSnapshot(null);
		setError(null);
		void load(fixtureId).then(
			(next) => current && setSnapshot(next),
			(reason) =>
				current &&
				setError(reason instanceof Error ? reason.message : String(reason)),
		);
		return () => {
			current = false;
		};
	}, [fixtureId, load, refresh]);

	if (error)
		return (
			<div className="native-media-controls unavailable">
				<strong>Native controls unavailable</strong>
				<span>{error}</span>
				<Button onClick={() => setRefresh((value) => value + 1)}>Retry</Button>
			</div>
		);
	if (!snapshot)
		return <div className="native-media-controls">Connecting to Media Server…</div>;

	return (
		<div className="native-media-controls">
			<div className="native-media-summary">
				<strong>{snapshot.status === "ok" ? "Media Server online" : snapshot.status}</strong>
				<span>{snapshot.endpoint}</span>
				<span>
					{snapshot.outputs} outputs · {snapshot.catalogItems} catalog items · revision{" "}
					{snapshot.catalogRevision}
				</span>
				<span>Instance {snapshot.instance}</span>
			</div>
			<section className="native-media-text" aria-label="Native Media text sources">
				<h3>Text sources</h3>
				{snapshot.textSlots.length ? (
					snapshot.textSlots.map((slot) => (
						<NativeTextSlotEditor
							key={`${slot.folder}:${slot.file}`}
							slot={slot}
							onSave={async (text) => {
								const updated = await updateText(
									fixtureId,
									slot.folder,
									slot.file,
									text,
								);
								setSnapshot((current) =>
									current
										? {
												...current,
												textSlots: current.textSlots.map((candidate) =>
													candidate.folder === updated.folder &&
													candidate.file === updated.file
														? updated
														: candidate,
												),
											}
										: current,
								);
							}}
						/>
					))
				) : (
					<span>No text sources are configured.</span>
				)}
			</section>
			{!snapshot.effectControlsAvailable && (
				<div className="native-media-effects unavailable">
					<strong>Effect controls unavailable</strong>
					<span>
						This Media Server build does not yet expose effect parameters through its native API.
					</span>
				</div>
			)}
		</div>
	);
}

function NativeTextSlotEditor({
	slot,
	onSave,
}: {
	slot: NativeMediaTextSlot;
	onSave(text: string): Promise<void>;
}) {
	const [text, setText] = useState(slot.text ?? "");
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const editable = slot.kind === "static";
	return (
		<div className="native-media-text-slot">
			<TextAreaField
				label={`${slot.folder}.${slot.file} · ${slot.name}`}
				value={text}
				disabled={!editable || saving}
				onChange={(event) => setText(event.target.value)}
			/>
			<Button
				disabled={!editable || saving || text === (slot.text ?? "")}
				onClick={() => {
					setSaving(true);
					setError(null);
					void onSave(text).catch((reason) =>
						setError(reason instanceof Error ? reason.message : String(reason)),
					).finally(() => setSaving(false));
				}}
			>
				{saving ? "Saving…" : "Save text"}
			</Button>
			{!editable && <span>{slot.kind} sources are shown read-only.</span>}
			{error && <span className="field-error">{error}</span>}
		</div>
	);
}
