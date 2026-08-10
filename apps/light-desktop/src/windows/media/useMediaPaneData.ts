import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { MediaServerInspection } from "../../api/client/mediaOutput";
import type { MediaServerFixture } from "../../api/types";
import type { MediaServersState } from "../../features/mediaServers/MediaServersContext";
import { mediaDraftForLayer } from "../MediaPaneWindow.helpers";

export const EMPTY_MEDIA_INSPECTION: MediaServerInspection = {
	library_revision: "",
	server: { name: "", layer_count: 0 },
	folders: [],
	files: [],
	preview_sources: [],
	layers: [],
	capabilities: { provider: "citp_msex", native_action: null, layers: [] },
};

interface MediaPaneDataInput {
	active: boolean;
	server: MediaServerFixture | undefined;
	layerId: string;
	inspect: MediaServersState["inspectMediaServer"] | undefined;
	refreshPreview: MediaServersState["refreshMediaPreview"] | undefined;
	refreshThumbnails: MediaServersState["refreshMediaThumbnails"] | undefined;
	loadThumbnail: MediaServersState["mediaThumbnail"] | undefined;
}

interface InspectionPollingInput {
	active: boolean;
	server: MediaServerFixture | undefined;
	layerId: string;
	inspect: MediaServersState["inspectMediaServer"] | undefined;
	reset(): void;
	setInspection: Dispatch<SetStateAction<MediaServerInspection>>;
	setInspectionError: Dispatch<SetStateAction<string | null>>;
	setDraftFolderId: Dispatch<SetStateAction<string>>;
	setDraftFileId: Dispatch<SetStateAction<string | null>>;
	initializedDraftScope: MutableRefObject<string | null>;
}

function useInspectionPolling({
	active,
	server,
	layerId,
	inspect,
	reset,
	setInspection,
	setInspectionError,
	setDraftFolderId,
	setDraftFileId,
	initializedDraftScope,
}: InspectionPollingInput) {
	useEffect(() => {
		if (!active || !inspect || !server) {
			reset();
			return;
		}
		let disposed = false;
		let running = false;
		const refresh = async () => {
			if (running) return;
			running = true;
			try {
				const next = await inspect(server.fixture_id);
				if (!disposed) {
					setInspection(next);
					setInspectionError(null);
					const scope = `${server.fixture_id}:${layerId}`;
					if (initializedDraftScope.current !== scope) {
						const draft = mediaDraftForLayer(next, server.layers, layerId);
						setDraftFolderId(draft?.folderId ?? "");
						setDraftFileId(draft?.fileId ?? null);
						initializedDraftScope.current = scope;
					}
				}
			} catch (cause) {
				if (!disposed) {
					setInspectionError(
						cause instanceof Error ? cause.message : String(cause),
					);
					setInspection(EMPTY_MEDIA_INSPECTION);
					setDraftFolderId("");
					setDraftFileId(null);
					initializedDraftScope.current = null;
				}
			} finally {
				running = false;
			}
		};
		void refresh();
		const timer = window.setInterval(() => void refresh(), 1_000);
		return () => {
			disposed = true;
			window.clearInterval(timer);
		};
	}, [
		active,
		initializedDraftScope,
		inspect,
		layerId,
		reset,
		server,
		setDraftFileId,
		setDraftFolderId,
		setInspection,
		setInspectionError,
	]);
}

export function useMediaPaneData({
	active,
	server,
	layerId,
	inspect,
	refreshPreview,
	refreshThumbnails,
	loadThumbnail,
}: MediaPaneDataInput) {
	const [inspection, setInspection] = useState(EMPTY_MEDIA_INSPECTION);
	const [inspectionError, setInspectionError] = useState<string | null>(null);
	const [draftFolderId, setDraftFolderId] = useState("");
	const [draftFileId, setDraftFileId] = useState<string | null>(null);
	const initializedDraftScope = useRef<string | null>(null);
	const [thumbnailUrls, setThumbnailUrls] = useState<Record<string, string>>(
		{},
	);
	const thumbnailUrlsRef = useRef(thumbnailUrls);
	thumbnailUrlsRef.current = thumbnailUrls;

	const reset = useCallback(() => {
		setInspection(EMPTY_MEDIA_INSPECTION);
		setInspectionError(null);
		setDraftFolderId("");
		setDraftFileId(null);
		initializedDraftScope.current = null;
	}, []);

	const initializeLayer = useCallback(
		(nextLayerId: string) => {
			const draft = mediaDraftForLayer(
				inspection,
				server?.layers ?? [],
				nextLayerId,
			);
			setDraftFolderId(draft?.folderId ?? "");
			setDraftFileId(draft?.fileId ?? null);
			initializedDraftScope.current = draft
				? `${server?.fixture_id}:${nextLayerId}`
				: null;
		},
		[inspection, server],
	);

	useInspectionPolling({
		active,
		server,
		layerId,
		inspect,
		reset,
		setInspection,
		setInspectionError,
		setDraftFolderId,
		setDraftFileId,
		initializedDraftScope,
	});

	useEffect(() => {
		if (!active || !refreshPreview || !server) return;
		const sources = inspection.preview_sources;
		if (sources.length === 0) return;
		const refresh = () => {
			for (const source of sources)
				void refreshPreview(server.fixture_id, source.id);
		};
		refresh();
		const timer = window.setInterval(refresh, 1_000);
		return () => window.clearInterval(timer);
	}, [active, inspection.preview_sources, refreshPreview, server]);

	const draftFolder = Number(draftFolderId);
	const visibleFiles = useMemo(
		() => inspection.files.filter((file) => file.folder_id === draftFolder),
		[draftFolder, inspection.files],
	);
	useEffect(() => {
		if (
			!active ||
			!refreshThumbnails ||
			!loadThumbnail ||
			!server ||
			visibleFiles.length === 0
		)
			return;
		let disposed = false;
		void (async () => {
			await refreshThumbnails(
				server.fixture_id,
				draftFolder,
				visibleFiles.map((file) => file.id),
			);
			const entries = await Promise.all(
				visibleFiles.map(async (file) => {
					const blob = await loadThumbnail(
						server.fixture_id,
						file.folder_id,
						file.id,
					);
					return [String(file.id), URL.createObjectURL(blob)] as const;
				}),
			);
			if (disposed) {
				for (const [, url] of entries) URL.revokeObjectURL(url);
				return;
			}
			setThumbnailUrls((current) => {
				for (const url of Object.values(current)) URL.revokeObjectURL(url);
				return Object.fromEntries(entries);
			});
		})().catch(() => undefined);
		return () => {
			disposed = true;
		};
	}, [
		active,
		draftFolder,
		loadThumbnail,
		refreshThumbnails,
		server,
		visibleFiles,
	]);

	useEffect(
		() => () => {
			for (const url of Object.values(thumbnailUrlsRef.current))
				URL.revokeObjectURL(url);
		},
		[],
	);

	return {
		inspection,
		inspectionError,
		setInspectionError,
		draftFolder,
		draftFolderId,
		setDraftFolderId,
		draftFileId,
		setDraftFileId,
		thumbnailUrls,
		visibleFiles,
		reset,
		initializeLayer,
	};
}
