import { useEffect, useState } from "react";
import { api } from "../api/client";
import { useOutputs } from "../api/queries";

const DEFAULT_ASPECT_RATIO = 16 / 9;

/** The preview frame follows the first configured program output, which is the server's main one. */
export function useMainOutputAspectRatio(): number {
	const outputs = useOutputs();
	const outputId = outputs.data?.[0]?.id;
	const [aspectRatio, setAspectRatio] = useState(DEFAULT_ASPECT_RATIO);

	useEffect(() => {
		if (!outputId) return;
		let current = true;
		void api
			.outputConfiguration(outputId)
			.then((configuration) => {
				if (current && configuration.width > 0 && configuration.height > 0)
					setAspectRatio(configuration.width / configuration.height);
			})
			.catch(() => {
				// The owning page reports transport failures. A preview can safely keep its 16:9 fallback.
			});
		return () => {
			current = false;
		};
	}, [outputId]);

	return aspectRatio;
}
