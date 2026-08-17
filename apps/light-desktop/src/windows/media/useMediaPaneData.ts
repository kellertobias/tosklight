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
	fixtureId: string | undefined;
	endpointKey: string;
	serverLayers: MutableRefObject<MediaServerFixture["layers"]>;
	layerId: string;
	inspect: MutableRefObject<
		MediaServersState["inspectMediaServer"] | undefined
	>;
	hasInspect: boolean;
	reset(): void;
	setInspection: Dispatch<SetStateAction<MediaServerInspection>>;
	setInspectionError: Dispatch<SetStateAction<string | null>>;
	setDraftFolderId: Dispatch<SetStateAction<string>>;
	setDraftFileId: Dispatch<SetStateAction<string | null>>;
	initializedDraftScope: MutableRefObject<string | null>;
}

function useInspectionPolling({
	active,
	fixtureId,
	endpointKey,
	serverLayers,
	layerId,
	inspect,
	hasInspect,
	reset,
	setInspection,
	setInspectionError,
	setDraftFolderId,
	setDraftFileId,
	initializedDraftScope,
}: InspectionPollingInput) {
	useEffect(() => {
		if (!active || !hasInspect || !fixtureId || !endpointKey) {
			reset();
			return;
		}
		let disposed = false;
		let running = false;
		const refresh = async () => {
			if (running) return;
			running = true;
			try {
				const next = await inspect.current?.(fixtureId);
				if (!next) return;
				if (!disposed) {
					setInspection(next);
					setInspectionError(null);
					const scope = `${fixtureId}:${layerId}`;
					if (initializedDraftScope.current !== scope) {
						const draft = mediaDraftForLayer(
							next,
							serverLayers.current,
							layerId,
						);
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
		endpointKey,
		fixtureId,
		hasInspect,
		initializedDraftScope,
		inspect,
		layerId,
		reset,
		serverLayers,
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
	const fixtureId = server?.fixture_id;
	const endpointKey = server?.endpoint
		? `${server.endpoint.ip_address}:${server.endpoint.port}`
		: "";
	const serverLayers = useRef(server?.layers ?? []);
	serverLayers.current = server?.layers ?? [];
	const inspectRef = useRef(inspect);
	inspectRef.current = inspect;
	const refreshPreviewRef = useRef(refreshPreview);
	refreshPreviewRef.current = refreshPreview;
	const refreshThumbnailsRef = useRef(refreshThumbnails);
	refreshThumbnailsRef.current = refreshThumbnails;
	const loadThumbnailRef = useRef(loadThumbnail);
	loadThumbnailRef.current = loadThumbnail;
	const hasRefreshPreview = Boolean(refreshPreview);
	const hasRefreshThumbnails = Boolean(refreshThumbnails);
	const hasLoadThumbnail = Boolean(loadThumbnail);
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
			if (nextLayerId === "master") {
				setDraftFolderId("1");
				setDraftFileId(null);
				initializedDraftScope.current = fixtureId
					? `${fixtureId}:${nextLayerId}`
					: null;
				return;
			}
			const draft = mediaDraftForLayer(
				inspection,
				serverLayers.current,
				nextLayerId,
			);
			setDraftFolderId(draft?.folderId ?? "");
			setDraftFileId(draft?.fileId ?? null);
			initializedDraftScope.current = draft
				? `${fixtureId}:${nextLayerId}`
				: null;
		},
		[fixtureId, inspection],
	);

	useInspectionPolling({
		active,
		fixtureId,
		endpointKey,
		serverLayers,
		layerId,
		inspect: inspectRef,
		hasInspect: Boolean(inspect),
		reset,
		setInspection,
		setInspectionError,
		setDraftFolderId,
		setDraftFileId,
		initializedDraftScope,
	});

	const previewSourceIds = inspection.preview_sources
		.map((source) => source.id)
		.join(",");
	useEffect(() => {
		if (!active || !hasRefreshPreview || !fixtureId || !endpointKey) return;
		const sources = previewSourceIds.split(",").filter(Boolean).map(Number);
		if (sources.length === 0) return;
		let disposed = false;
		let running = false;
		const refresh = async () => {
			if (running || disposed) return;
			running = true;
			try {
				await Promise.all(
					sources.map((source) =>
						refreshPreviewRef.current?.(fixtureId, source),
					),
				);
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
		endpointKey,
		fixtureId,
		hasRefreshPreview,
		previewSourceIds,
		refreshPreviewRef,
	]);

	const draftFolder = Number(draftFolderId);
	const visibleFiles = useMemo(
		() => inspection.files.filter((file) => file.folder_id === draftFolder),
		[draftFolder, inspection.files],
	);
	useEffect(() => {
		setThumbnailUrls((current) => {
			for (const url of Object.values(current)) URL.revokeObjectURL(url);
			return {};
		});
	}, [draftFolder, fixtureId]);
	useEffect(() => {
		if (
			!active ||
			!hasRefreshThumbnails ||
			!hasLoadThumbnail ||
			!fixtureId ||
			!endpointKey ||
			visibleFiles.length === 0
		)
			return;
		let disposed = false;
		void (async () => {
			await refreshThumbnailsRef.current?.(
				fixtureId,
				draftFolder,
				visibleFiles.map((file) => file.id),
			);
			const entries = await Promise.all(
				visibleFiles.map(async (file) => {
					const blob = await loadThumbnailRef.current?.(
						fixtureId,
						file.folder_id,
						file.id,
					);
					if (!blob) throw new Error("Media thumbnail loading is unavailable");
					return [
						`${file.folder_id}:${file.id}`,
						URL.createObjectURL(blob),
					] as const;
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
		endpointKey,
		fixtureId,
		hasLoadThumbnail,
		hasRefreshThumbnails,
		loadThumbnailRef,
		refreshThumbnailsRef,
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
