// One output's pixel map, loaded and saved on its own.
//
// It reads the same output configuration the picture settings do, and writes back only the map, so
// editing a zone cannot disturb the monitor or the frame rate the output is on.

import { useCallback, useEffect, useState } from "react";
import { ApiFailure, api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	OutputConfigurationView,
	PixelMapView,
} from "../../shared/api/generated/media-wire";
import { SettingsSaveState } from "../settings/SettingsSaveState";
import { PixelMapSettings } from "./PixelMapSettings";

export function OutputPixelMap({
	outputId,
	outputName,
}: {
	outputId: string;
	outputName: string;
}) {
	const [output, setOutput] = useState<OutputConfigurationView>();
	const [failure, setFailure] = useState<ApiFailure>();
	const [revision, setRevision] = useState(0);
	const reload = useCallback(() => setRevision((current) => current + 1), []);
	const editing = useEditing(reload);

	useEffect(() => {
		let current = true;
		void api
			.outputConfiguration(outputId)
			.then((configuration) => {
				if (current) setOutput(configuration);
			})
			.catch((error: unknown) => {
				if (current && error instanceof ApiFailure) setFailure(error);
			});
		return () => {
			current = false;
		};
	}, [outputId, revision]);

	if (failure) {
		return (
			<article className="media-settings-section">
				<p>{outputName} pixel map could not be read.</p>
			</article>
		);
	}
	if (!output) {
		return (
			<article className="media-settings-section">
				<p>Reading {outputName}…</p>
			</article>
		);
	}
	return (
		<>
			<SettingsSaveState
				busy={editing.busy}
				failed={editing.failure !== undefined}
				restartBound={true}
			/>
			<PixelMapSettings
				key={`${output.id}-${revision}`}
				output={output}
				busy={editing.busy}
				onSave={(pixelMap: PixelMapView) =>
					void editing.save(() =>
						api.updateOutputConfiguration(output.id, {
							requestId: requestId(),
							pixelMap,
						}),
					)
				}
			/>
		</>
	);
}
