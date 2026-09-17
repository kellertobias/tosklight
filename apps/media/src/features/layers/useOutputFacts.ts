import { useEffect, useState } from "react";
import { api } from "../../shared/api/client";

/** The selected output's picture size, read once per output. */
export function useOutputFacts(outputId: string | undefined) {
	const [previewSize, setPreviewSize] = useState<
		{ width: number; height: number } | undefined
	>();
	useEffect(() => {
		if (!outputId) return;
		let current = true;
		void api
			.outputConfiguration(outputId)
			.then((configuration) => {
				if (!current) return;
				setPreviewSize({
					width: configuration.width,
					height: configuration.height,
				});
			})
			.catch(() => {
				// Output state still remains usable with the shared 16:9 preview fallback.
			});
		return () => {
			current = false;
		};
	}, [outputId]);
	return { previewSize };
}
