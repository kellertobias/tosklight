import { useRef, useState } from "react";
import type { MediaServerFixture } from "../../api/types";

export function useMediaServerState() {
	const [mediaServers, setMediaServers] = useState<MediaServerFixture[]>([]);
	const [mediaPreviewUrls, setMediaPreviewUrls] = useState<
		Record<string, string>
	>({});
	const mediaPreviewUrlsRef = useRef<Record<string, string>>({});
	return {
		mediaServers,
		setMediaServers,
		mediaPreviewUrls,
		setMediaPreviewUrls,
		mediaPreviewUrlsRef,
	};
}
