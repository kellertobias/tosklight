import { useEffect, useRef, useState } from "react";
import { useDesktopBridge } from "./platform/desktop";
import { runImprovedBeamCapabilitySpike } from "./windows/stage3d/improvedBeamCapabilitySpike";

export function PackagedImprovedBeamSpikeApp() {
	const desktop = useDesktopBridge();
	const [prepared, setPrepared] = useState(false);
	const started = useRef(false);

	useEffect(() => {
		let cancelled = false;
		const poll = async () => {
			while (!cancelled) {
				if (await desktop.packagedStageBenchmarkPrepared().catch(() => false)) {
					setPrepared(true);
					return;
				}
				await new Promise<void>((resolve) => window.setTimeout(resolve, 100));
			}
		};
		void poll();
		return () => {
			cancelled = true;
		};
	}, [desktop]);

	useEffect(() => {
		if (!prepared || started.current) return;
		started.current = true;
		void desktop.appendPackagedStageBenchmarkSample({
			schemaVersion: 1,
			kind: "started",
			measurementSurface: "packaged-tauri-webview",
			profile: "improved-beam-spike",
			startedAt: new Date().toISOString(),
		});
		try {
			const spike = runImprovedBeamCapabilitySpike();
			void desktop.appendPackagedStageBenchmarkSample({
				schemaVersion: 1,
				kind: "complete",
				measurementSurface: "packaged-tauri-webview",
				profile: "improved-beam-spike",
				recordedAt: new Date().toISOString(),
				spike,
			});
		} catch (reason) {
			void desktop.appendPackagedStageBenchmarkSample({
				schemaVersion: 1,
				kind: "error",
				profile: "improved-beam-spike",
				message: reason instanceof Error ? reason.message : String(reason),
			});
		}
	}, [desktop, prepared]);

	return (
		<div data-testid="packaged-improved-beam-spike">
			{prepared
				? "Measuring Improved beams capability"
				: "Preparing Improved beams capability spike"}
		</div>
	);
}
