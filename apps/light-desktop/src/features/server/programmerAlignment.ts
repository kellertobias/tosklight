import type { ProgrammingAlignOutcome } from "../../api/generated/light-wire";
import { reportDeskNotice } from "../deskNotice/deskNotice";
import type { ServerCapabilities } from "./capabilityContracts";
import type { ServerController } from "./model";

export const ALIGN_NO_SELECTION_NOTICE =
	"No fixtures selected. Align stays Off; nothing changed.";

export function createProgrammerAlignmentActions(
	model: ServerController,
): Pick<ServerCapabilities, "alignSelection"> {
	const { api, setError } = model;
	return {
		alignSelection: async (mode) => {
			let outcome: Partial<ProgrammingAlignOutcome> | undefined;
			try {
				outcome = (await api.programming.align(mode)) as
					| Partial<ProgrammingAlignOutcome>
					| undefined;
				setError(null);
			} catch (reason) {
				// Refusals and genuine desk failures keep the actionable error treatment.
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
			const resulting = outcome?.mode ?? mode;
			// The server reports an activation that changed nothing (no selection) as Off.
			if (mode !== "off" && resulting === "off")
				reportDeskNotice(ALIGN_NO_SELECTION_NOTICE);
			return resulting;
		},
	};
}
