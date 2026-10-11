import { Button, ErrorAlert } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import { assertPlaybackAddress } from "../../api/playbackAddress";
import type { PlaybackConfigurationNotification } from "../../api/playbackConfigurationModels";
import {
	useActiveShowId,
	useSessionSnapshot,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { usePlaybackTopologyView } from "../../features/playbackTopology/PlaybackTopologyView";
import { useApp } from "../../state/AppContext";
import { AssignedPlaybackConfigurationModal } from "./AssignedPlaybackConfigurationModal";
import { emptyConfiguration } from "./playbackFaderBank/feedback";
import type { PlaybackConfigurationState } from "./playbackFaderBank/types";

/** One command-request editor per desktop surface, independent of visible bank geometry. */
export function PlaybackConfigurationRequestHost() {
	const session = useSessionSnapshot();
	const showId = useActiveShowId();
	const ordinal = useRef(0);
	const [request, setRequest] = useState<{
		notification: PlaybackConfigurationNotification;
		ordinal: number;
	} | null>(null);
	useEffect(() => setRequest(null), [showId, session?.desk.id]);
	useEffect(() => {
		const open = (event: Event) => {
			const notification = (
				event as CustomEvent<PlaybackConfigurationNotification>
			).detail;
			if (!showId || notification.desk_id !== session?.desk.id) return;
			assertPlaybackAddress(notification.address);
			setRequest({ notification, ordinal: ++ordinal.current });
		};
		window.addEventListener("light:playback-configuration", open);
		return () => window.removeEventListener("light:playback-configuration", open);
	}, [session?.desk.id, showId]);
	return request ? (
		<RequestedConfiguration
			key={request.ordinal}
			request={request.notification}
			onClose={() => setRequest(null)}
		/>
	) : null;
}

function RequestedConfiguration({ request, onClose }: {
	request: PlaybackConfigurationNotification;
	onClose(): void;
}) {
	const { dispatch } = useApp();
	const [configuration, setConfiguration] = useState<PlaybackConfigurationState | null>(null);
	const topology = usePlaybackTopologyView(!configuration);
	useEffect(() => {
		if (configuration || !topology.ready) return;
		const { page, position } = request.address;
		const pageObject = topology.pages.find((item) => item.body.number === page);
		const playback = pageObject?.body.assignments[position];
		setConfiguration({
			page,
			slot: position,
			playback: playback ?? emptyConfiguration(
				page, position, 3, true, topology.cueLists[0]?.body.id ?? "",
			),
			empty: !playback,
			fallbackButtons: playback?.button_count ?? 3,
			expectedPageRevision: pageObject?.revision ?? 0,
			expectedPageObjectId: pageObject?.id ?? null,
		});
		dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
		dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
		dispatch({ type: "SET_SHIFT_ARMED", value: false });
	}, [configuration, request, topology, dispatch]);
	if (configuration)
		return <AssignedPlaybackConfigurationModal {...configuration} onClose={onClose} />;
	return (
		<ErrorAlert role="status">
			{topology.error?.message ?? "Loading Playback configuration…"}
			<Button onClick={onClose}>Cancel</Button>
		</ErrorAlert>
	);
}
