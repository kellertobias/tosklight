import { useEffect, useRef, useState } from "react";
import type { PlaybackAddressValue } from "../../../api/playbackAddress";
import type {
	PlaybackSoloCapability,
	PlaybackSoloRegion,
} from "../../../features/playbackSolo/contracts";

export function usePlaybackSoloDraft(
	capability: PlaybackSoloCapability | null,
	regions: readonly PlaybackSoloRegion[] | null,
) {
	const [creating, setCreating] = useState(false);
	const createPending = useRef<number | null>(null);
	const [selecting, setSelecting] = useState(false);
	const [selected, setSelected] = useState<PlaybackAddressValue[]>([]);
	const [name, setName] = useState("");
	const [editingId, setEditingId] = useState<string | null>(null);
	const draftId = useRef<string | null>(null);
	const draftVersion = useRef(0);
	const [error, setError] = useState<string | null>(null);
	const [managing, setManaging] = useState(false);
	useEffect(() => {
		draftVersion.current += 1;
		createPending.current = null;
		setCreating(false);
		draftId.current = null;
		setEditingId(null);
		setSelecting(false);
		setSelected([]);
		setError(null);
		setManaging(false);
	}, [capability?.authorityGeneration]);
	const cancel = () => {
		draftVersion.current += 1;
		draftId.current = null;
		setEditingId(null);
		setSelecting(false);
		setSelected([]);
		setError(null);
	};
	const create = async () => {
		if (!capability || !regions || !selecting || createPending.current !== null) return;
		const id = draftId.current;
		if (!id) return;
		if (editingId && !regions.some((region) => region.id === editingId)) {
			setError("This Solo Region was removed. Cancel and start again.");
			return;
		}
		if (!name.trim()) {
			setError("Enter a Solo Region name");
			return;
		}
		if (selected.length < 2) {
			setError("Select at least two Playback addresses");
			return;
		}
		setError(null);
		const version = draftVersion.current;
		createPending.current = version;
		setCreating(true);
		try {
			const region = { id, name: name.trim(), members: selected };
			const next = await capability.save(
				editingId
					? regions.map((item) => item.id === editingId ? region : item)
					: [...regions, region],
			);
			if (next && version === draftVersion.current) {
				cancel();
				setName("");
			}
		} finally {
			if (createPending.current === version) {
				createPending.current = null;
				setCreating(false);
			}
		}
	};
	const beginEdit = (region?: PlaybackSoloRegion) => {
		if (createPending.current !== null) return;
		draftVersion.current += 1;
		draftId.current = region?.id ?? crypto.randomUUID();
		setEditingId(region?.id ?? null);
		setName(region?.name ?? `Solo Region ${(regions?.length ?? 0) + 1}`);
		setSelected(region?.members.map((address) => ({ ...address })) ?? []);
		setError(null);
		setSelecting(true);
	};
	return {
		creating, selecting, selected, setSelected, name, setName, error,
		managing, setManaging, cancel, create, beginEdit,
	};
}
