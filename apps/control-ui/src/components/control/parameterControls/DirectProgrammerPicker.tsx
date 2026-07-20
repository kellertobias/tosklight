import { Button } from "../../common";
import type { DirectControlChoice } from "./model";
import type { ParameterController } from "./useParameterController";

function actionContent(choice: DirectControlChoice) {
	return (
		<>
			<b>{choice.label}</b>
			<small>
				{choice.kind.replaceAll("_", " ")}
				{choice.durationMillis != null ? ` · ${choice.durationMillis} ms` : ""}
			</small>
		</>
	);
}

function MomentaryActionButton({
	controller,
	choice,
}: {
	controller: ParameterController;
	choice: DirectControlChoice;
}) {
	return (
		<Button
			disabled={Boolean(controller.selectedGroupId)}
			className={controller.latchedActions[choice.key] ? "active" : ""}
			aria-label={`${choice.label} ${choice.kind} control action`}
			onPointerDown={(event) => {
				event.currentTarget.setPointerCapture?.(event.pointerId);
				void controller.applyControlAction(choice, true);
			}}
			onPointerUp={(event) => {
				if (event.currentTarget.hasPointerCapture?.(event.pointerId))
					event.currentTarget.releasePointerCapture(event.pointerId);
				void controller.applyControlAction(choice, false);
			}}
			onPointerCancel={() => void controller.applyControlAction(choice, false)}
			onKeyDown={(event) => {
				if (event.repeat || (event.key !== "Enter" && event.key !== " "))
					return;
				event.preventDefault();
				void controller.applyControlAction(choice, true);
			}}
			onKeyUp={(event) => {
				if (event.key !== "Enter" && event.key !== " ") return;
				event.preventDefault();
				void controller.applyControlAction(choice, false);
			}}
		>
			{actionContent(choice)}
		</Button>
	);
}

function PersistentActionButton({
	controller,
	choice,
}: {
	controller: ParameterController;
	choice: DirectControlChoice;
}) {
	const active = Boolean(controller.latchedActions[choice.key]);
	return (
		<Button
			disabled={Boolean(controller.selectedGroupId)}
			className={active ? "active" : ""}
			aria-label={`${choice.label} ${choice.kind} control action`}
			onClick={() => {
				const next = choice.kind === "latched" ? !active : true;
				if (choice.kind === "latched")
					controller.setLatchedActions((current) => ({
						...current,
						[choice.key]: next,
					}));
				void controller.applyControlAction(choice, next);
			}}
		>
			{actionContent(choice)}
		</Button>
	);
}

function DirectChoiceColumns({
	controller,
}: {
	controller: ParameterController;
}) {
	const { directChoices } = controller;
	return (
		<div className="direct-programmer-columns">
			<section>
				<h3>Fixed and indexed values</h3>
				<div className="direct-value-grid">
					{directChoices.values.map((choice) => (
						<Button
							key={choice.key}
							disabled={
								Boolean(controller.selectedGroupId) ||
								!controller.canWriteValues
							}
							className={controller.directChoiceActive(choice) ? "active" : ""}
							aria-label={`${choice.label} ${choice.kind} value`}
							onClick={() => void controller.applyDirectValue(choice)}
						>
							<b>{choice.label}</b>
							<small>
								{choice.kind} · {choice.assignments[0]?.attribute}
							</small>
						</Button>
					))}
				</div>
			</section>
			<section>
				<h3>Control actions</h3>
				<div className="direct-value-grid">
					{directChoices.actions.map((choice) =>
						choice.kind === "momentary" ? (
							<MomentaryActionButton
								key={choice.key}
								controller={controller}
								choice={choice}
							/>
						) : (
							<PersistentActionButton
								key={choice.key}
								controller={controller}
								choice={choice}
							/>
						),
					)}
				</div>
			</section>
		</div>
	);
}

export function DirectProgrammerPicker({
	controller,
}: {
	controller: ParameterController;
}) {
	const { directChoices } = controller;
	const empty =
		!controller.selectedGroupId &&
		!directChoices.values.length &&
		!directChoices.actions.length;
	return (
		<section
			className="direct-programmer-picker"
			aria-label="Direct programmer values and actions"
		>
			<header>
				<div>
					<b>Fixed, indexed, and control values</b>
					<small>
						Semantic values stay portable across fixture-profile DMX ranges.
					</small>
				</div>
				<Button
					disabled={
						!directChoices.values.length || Boolean(controller.selectedGroupId)
					}
					onClick={() => void controller.generateDirectPresets()}
				>
					Generate portable presets
				</Button>
			</header>
			{controller.selectedGroupId && (
				<p role="note">
					Select concrete fixtures to use typed direct values or generate
					presets.
				</p>
			)}
			{empty ? (
				<div className="direct-programmer-empty">
					<b>No direct values configured</b>
					<small>
						The selected profile mode has no fixed/indexed functions or typed
						control actions.
					</small>
				</div>
			) : (
				<DirectChoiceColumns controller={controller} />
			)}
			{controller.generationStatus && (
				<footer role="status">{controller.generationStatus}</footer>
			)}
		</section>
	);
}
