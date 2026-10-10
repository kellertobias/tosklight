import { expect, it } from "vitest";
import { updateTargetContext } from "./updateWorkflowPresentation";

it("describes a Cuelist cue without masquerading its internal assignment as a surface playback", () => {
	expect(
		updateTargetContext({
			family: { type: "cue" },
			object_id: "stable-cuelist-uuid",
			name: "Cuelist 101",
			playback_number: 21,
			cue: { id: "stable-cue-uuid", number: "1" },
		}),
	).toBe("Cuelist · Current Cue 1");
});
