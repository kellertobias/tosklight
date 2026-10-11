import { Button, ErrorAlert, TextField } from "@tosklight/ui";
import { useEffect, useState, useSyncExternalStore } from "react";
import {
	assertPlaybackAddress,
	playbackAddressKey,
} from "../../api/playbackAddress";
import { usePlaybackSoloEditor } from "../../features/playbackSolo/PlaybackSoloEditorContext";
import { PlaybackSoloRegionManager } from "./PlaybackSoloRegionManager";
import { usePlaybackSolo } from "../../features/playbackSolo/PlaybackSoloContext";
import type { PlaybackSoloRegion } from "../../features/playbackSolo/contracts";

export function parseSoloMembers(value: string) {
	const members = value
		.split(/[ ,;]+/)
		.filter(Boolean)
		.map((token) => {
			const match = /^(\d+)\.(\d+)$/.exec(token);
			if (!match) throw new Error("Use page.position for each member");
			const address = { page: Number(match[1]), position: Number(match[2]) };
			assertPlaybackAddress(address);
			return address;
		});
	if (members.length < 2)
		throw new Error("A Solo Region needs at least two members");
	if (new Set(members.map(playbackAddressKey)).size !== members.length)
		throw new Error("Solo Region members must be unique");
	return members;
}

export function PlaybackSoloSettings({ surfaceId, onClose }: {
	surfaceId?: string;
	onClose?: () => void;
} = {}) {
	const editor = usePlaybackSoloEditor(surfaceId);
	const capability = usePlaybackSolo();
	useEffect(() => capability.activate(), [capability.authorityGeneration]);
	const regions = useSyncExternalStore(
		capability.subscribe,
		capability.getRegions,
		capability.getRegions,
	);
	const [editing, setEditing] = useState<PlaybackSoloRegion | null>(null);
	const [name, setName] = useState("");
	const [members, setMembers] = useState("");
	const [error, setError] = useState<string | null>(null);
	const edit = (region: PlaybackSoloRegion | null) => {
		setEditing(region);
		setName(region?.name ?? "");
		setMembers(
			region?.members
				.map((address) => `${address.page}.${address.position}`)
				.join(", ") ?? "",
		);
		setError(null);
	};
	const save = async () => {
		try {
			if (!name.trim()) throw new Error("Enter a Solo Region name");
			const region = {
				id: editing?.id ?? crypto.randomUUID(),
				name: name.trim(),
				members: parseSoloMembers(members),
			};
			const next = editing
				? (regions ?? []).map((item) =>
						item.id === editing.id ? region : item,
					)
				: [...(regions ?? []), region];
			if (await capability.save(next)) edit(null);
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		}
	};
	if (editor) return (
		<PlaybackSoloRegionManager key={capability.authorityGeneration}
			regions={regions} capability={capability}
			editor={editor} onClose={onClose} />
	);
	if (surfaceId) return <p role="status">Open this surface’s Playback grid to edit Solo Regions.</p>;
	return (
		<section aria-label="Playback Solo Regions">
			<p>
				Solo Regions contain exact playback addresses across all surfaces and
				pages. Flash bypasses Solo. Editing a region never operates a playback.
			</p>
			{regions === null ? (
				<p role="status">Loading Solo Regions…</p>
			) : (
				regions.map((region) => (
					<article key={region.id}>
						<b>{region.name}</b>{" "}
						<span>
							{region.members
								.map((address) => `${address.page}.${address.position}`)
								.join(", ")}
						</span>
						<Button onClick={() => edit(region)}>Edit {region.name}</Button>
						<Button
							onClick={() =>
								void capability.save(
									regions.filter((item) => item.id !== region.id),
								)
							}
						>
							Delete {region.name}
						</Button>
					</article>
				))
			)}
			<TextField
				label="Region name"
				value={name}
				onChange={(event) => setName(event.target.value)}
			/>
			<TextField
				label="Members (page.position)"
				value={members}
				onChange={(event) => setMembers(event.target.value)}
			/>
			<Button
				disabled={regions === null || capability.isSaving()}
				onClick={() => void save()}
			>
				{editing ? "Save region" : "Create region"}
			</Button>
			{editing && <Button onClick={() => edit(null)}>Cancel edit</Button>}
			{(error || capability.error) && (
				<ErrorAlert role="alert">{error || capability.error}</ErrorAlert>
			)}
		</section>
	);
}
