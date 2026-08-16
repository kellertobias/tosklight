import { Button } from "@tosklight/ui";
import type { ProgrammerValuesMutation } from "../../../features/programmerValues/contracts";
import type { IndexedPresetChoice } from "../../control/parameterControls/indexedPresetChoices";

export function playModeMutations(
	choice: IndexedPresetChoice,
	programmerFadeMillis: number | undefined,
): ProgrammerValuesMutation[] {
	if (!choice.semanticId) return [];
	return choice.targets.map((target) => ({
		action: "set_fixture",
		fixtureId: target.fixtureId,
		attribute: "media.play_mode",
		value: { kind: "discrete", value: choice.semanticId as string },
		timing: {
			fade: true,
			fadeMillis: programmerFadeMillis ?? 3_000,
			delayMillis: null,
		},
	}));
}

interface MediaPlayModeDialogProps {
	choices: readonly IndexedPresetChoice[];
	value: string | null;
	mixed: boolean;
	disabled: boolean;
	apply: (choice: IndexedPresetChoice) => Promise<void>;
}

export function MediaPlayModeDialog({
	choices,
	value,
	mixed,
	disabled,
	apply,
}: MediaPlayModeDialogProps) {
	const playModes = choices.filter(
		(choice) => choice.semanticId !== null && choice.kind !== "control",
	);
	return (
		<section className="media-play-mode-dialog" aria-label="Play Mode">
			<header>
				<b>Play Mode</b>
				<span>{mixed ? "Mixed selection" : (value ?? "No mode selected")}</span>
			</header>
			{playModes.length ? (
				<div className="media-play-mode-options">
					{playModes.map((choice) => (
						<Button
							key={choice.id}
							className={value === choice.semanticId && !mixed ? "active" : ""}
							aria-pressed={value === choice.semanticId && !mixed}
							disabled={disabled || choice.disabled}
							onClick={() => void apply(choice)}
						>
							<b>{choice.label}</b>
							<small>{choice.description}</small>
						</Button>
					))}
				</div>
			) : (
				<p>No Play Mode exists on the selected fixtures.</p>
			)}
		</section>
	);
}
