import {
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createLightApi } from "../api/client/api";
import { type MacroDefinition, MacrosApiClient } from "../api/client/macros";
import type { MacroExecution } from "../api/runtimeModels";
import type { VersionedObject } from "../api/types";
import { useCommandLineSurface } from "../components/control/commandLine/useCommandLineSurface";
import {
	consumeObjectEditorRequest,
	currentObjectEditorRequest,
	subscribeObjectEditorRequest,
} from "../features/controlSurfaceInteraction/objectEditorRequest";
import {
	poolMutationTarget,
	poolMutationTargetState,
} from "../features/controlSurfaceInteraction/poolCommandTarget";
import { useActiveShowId } from "../features/deskSnapshot/DeskSnapshotState";
import {
	type MacroActions,
	useMacroActions,
} from "../features/macros/MacroActionsContext";
import { MacroEditor } from "../features/macros/MacroEditor";
import { resolveMacroPoolGesture } from "../features/macros/poolGesture";
import type { WindowProps } from "./windowTypes";
import "./MacrosWindow.css";

const MACRO_POOL_SIZE = 200;
const MACRO_COLOR = "#8f3541";

type MacroObject = VersionedObject<MacroDefinition>;

export function MacrosWindow({ active = true, compact = false }: WindowProps) {
	const showId = useActiveShowId();
	const command = useCommandLineSurface({
		enabled: active,
		observeCommand: true,
	});
	const fallback = useMemo<MacroActions>(() => {
		const api = createLightApi();
		return {
			macros: new MacrosApiClient(api.runtime.capabilityTransport()),
			showObjects: api.showObjects,
		};
	}, []);
	const actions = useMacroActions() ?? fallback;
	const [macros, setMacros] = useState<MacroObject[]>([]);
	const [executions, setExecutions] = useState<MacroExecution[]>([]);
	const [editing, setEditing] = useState<MacroObject | NewMacro | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const copyPending = useRef(false);

	const refresh = useCallback(async () => {
		if (!showId) return;
		const [objects, runtime] = await Promise.all([
			actions.showObjects.objects<MacroDefinition>(showId, "macro"),
			actions.macros.runtime(showId),
		]);
		setMacros(
			objects.sort((left, right) => left.body.number - right.body.number),
		);
		setExecutions([...runtime.active, ...runtime.recent]);
	}, [actions, showId]);

	useEffect(() => {
		if (!active || !showId) return;
		let cancelled = false;
		const update = () =>
			void refresh().catch((reason) => {
				if (!cancelled) setError(String(reason));
			});
		const unsubscribe = actions.events?.onEvent((event) => {
			if (event.type !== "macro_execution_changed") return;
			setExecutions((current) =>
				upsertMacroExecution(current, event.execution),
			);
		});
		update();
		return () => {
			cancelled = true;
			unsubscribe?.();
		};
	}, [actions.events, active, refresh, showId]);

	useEffect(() => {
		if (!active) return;
		const openRequested = (request: {
			kind: "macro" | "timecode";
			objectId: string;
		}) => {
			if (request.kind !== "macro") return;
			const macro = macros.find(
				(candidate) => candidate.id === request.objectId,
			);
			if (!macro) return;
			setEditing(macro);
			consumeObjectEditorRequest(request);
		};
		const current = currentObjectEditorRequest();
		if (current) openRequested(current);
		return subscribeObjectEditorRequest(openRequested);
	}, [active, macros]);

	const run = async (macro: MacroObject) => {
		if (!showId || busy) return;
		setBusy(true);
		setError(null);
		try {
			const execution = await actions.macros.run(showId, macro.id, {
				source_revision: macro.revision,
				trigger: { type: "pool" },
			});
			setExecutions((current) => upsertMacroExecution(current, execution));
		} catch (reason) {
			setError(String(reason));
		} finally {
			setBusy(false);
		}
	};

	const open = async (macro: MacroObject) => {
		setEditing(macro);
		if (/^SET$/i.test(command.read().text.trim())) await command.reset();
	};

	const byNumber = new Map(macros.map((macro) => [macro.body.number, macro]));
	const mutationTarget = poolMutationTarget(command.text);
	const copyFromCommand = async (number: number) => {
		if (
			!showId ||
			busy ||
			copyPending.current ||
			mutationTarget?.operation !== "copy"
		)
			return false;
		if (mutationTarget.phase === "source") {
			if (!byNumber.has(number)) return false;
			await command.replace(`COPY ${number} AT`);
			return true;
		}
		const source = byNumber.get(Number(mutationTarget.source));
		if (!source) {
			setError(`Macro ${mutationTarget.source} does not exist.`);
			return true;
		}
		if (byNumber.has(number)) {
			setError(`Macro ${number} is already occupied.`);
			return true;
		}
		setBusy(true);
		copyPending.current = true;
		try {
			await actions.macros.copy(showId, source.id, source.revision, number);
			await refresh();
			await command.reset();
		} catch (reason) {
			setError(`Copy failed: ${String(reason)}`);
		} finally {
			copyPending.current = false;
			setBusy(false);
		}
		return true;
	};
	const running = new Set(
		executions
			.filter((execution) =>
				["queued", "validating", "running"].includes(execution.state),
			)
			.map((execution) => execution.macro_id),
	);
	const slots: PoolSlotViewModel<number>[] = macros.map((macro) => ({
		id: macro.body.number,
		position: macro.body.number - 1,
		card: { number: macro.body.number, primary: macro.body.name },
	}));
	const setClick = /^SET$/i.test(command.read().text.trim());

	return editing ? (
		<MacroEditor
			showId={showId}
			macro={editing}
			api={actions.macros}
			onClose={() => setEditing(null)}
			onSaved={async () => {
				await refresh();
			}}
		/>
	) : (
		<section className="macro-window" aria-busy={busy}>
			{!compact && (
				<WindowHeader
					title="Macros"
					info={{ primary: `${macros.length} Macros` }}
				/>
			)}
			{error && (
				<p role="alert" className="macro-error">
					{error}
				</p>
			)}
			<WindowScrollArea>
				<MacroPool
					macros={macros}
					slots={slots}
					byNumber={byNumber}
					running={running}
					setClick={setClick}
					mutationTarget={mutationTarget}
					onCreate={(number) => setEditing(newMacro(number))}
					onCopyTarget={(number) => void copyFromCommand(number)}
					onOpen={(macro) => void open(macro)}
					onRun={(macro) => void run(macro)}
				/>
			</WindowScrollArea>
		</section>
	);
}

function MacroPool({
	macros,
	slots,
	byNumber,
	running,
	setClick,
	mutationTarget,
	onCreate,
	onCopyTarget,
	onOpen,
	onRun,
}: {
	macros: MacroObject[];
	slots: PoolSlotViewModel<number>[];
	byNumber: Map<number, MacroObject>;
	running: Set<string>;
	setClick: boolean;
	mutationTarget: ReturnType<typeof poolMutationTarget>;
	onCreate(number: number): void;
	onCopyTarget(number: number): void;
	onOpen(macro: MacroObject): void;
	onRun(macro: MacroObject): void;
}) {
	return (
		<PoolGrid
			slots={slots}
			slotCount={Math.max(
				MACRO_POOL_SIZE,
				...macros.map((item) => item.body.number),
			)}
			emptySlot={(index) => ({
				id: index + 1,
				position: index,
				card: { number: index + 1, primary: "Empty", states: ["empty"] },
			})}
			renderSlot={(_, index) => {
				const number = index + 1;
				const macro = byNumber.get(number);
				const gesture = (contextMenu: boolean) =>
					resolveMacroPoolGesture(Boolean(macro), setClick, contextMenu);
				return (
					<PoolCard
						key={number}
						aria-label={
							macro
								? `Macro ${number} ${macro.body.name}`
								: `Empty Macro ${number}`
						}
						aria-pressed={macro ? running.has(macro.id) : undefined}
						model={macroPoolCard(
							number,
							macro,
							running,
							setClick,
							mutationTarget,
						)}
						onClick={() => {
							if (mutationTarget?.operation === "copy") {
								onCopyTarget(number);
								return;
							}
							const outcome = gesture(false);
							if (outcome === "create") onCreate(number);
							else if (outcome === "edit" && macro) onOpen(macro);
							else if (macro) onRun(macro);
						}}
						onContextMenu={(event) => {
							event.preventDefault();
							const outcome = gesture(true);
							if (outcome === "create") onCreate(number);
							else if (macro) onOpen(macro);
						}}
					/>
				);
			}}
		/>
	);
}

function macroPoolCard(
	number: number,
	macro: MacroObject | undefined,
	running: Set<string>,
	setClick: boolean,
	mutationTarget: ReturnType<typeof poolMutationTarget> = null,
) {
	return {
		number,
		primary: macro?.body.name ?? "Empty",
		secondary: macro
			? `${macro.body.source.split("\n").filter((line) => line.trim()).length} lines`
			: "Tap to create",
		icon: macro?.body.presentation.icon,
		color: macro?.body.presentation.color ?? MACRO_COLOR,
		states: [
			...(!macro ? (["empty"] as const) : []),
			...(macro && running.has(macro.id) ? (["active"] as const) : []),
			...(macro && setClick ? (["set-target"] as const) : []),
			...(mutationTarget?.operation === "copy" &&
			((mutationTarget.phase === "source" && macro) ||
				(mutationTarget.phase === "destination" && !macro))
				? [poolMutationTargetState(mutationTarget)!]
				: []),
		],
	};
}

function upsertMacroExecution(
	current: readonly MacroExecution[],
	next: MacroExecution,
): MacroExecution[] {
	const existing = current.find(
		(execution) => execution.execution_id === next.execution_id,
	);
	if (existing && executionRank(existing.state) > executionRank(next.state)) {
		return [...current];
	}
	return [
		next,
		...current.filter(
			(execution) => execution.execution_id !== next.execution_id,
		),
	];
}

function executionRank(state: MacroExecution["state"]): number {
	return [
		"queued",
		"validating",
		"running",
		"succeeded",
		"failed",
		"cancelled",
	].indexOf(state);
}

interface NewMacro {
	id: string;
	revision: 0;
	body: MacroDefinition;
	isNew: true;
}

function newMacro(number: number): NewMacro {
	const id = crypto.randomUUID();
	return {
		id,
		revision: 0,
		isNew: true,
		body: {
			id,
			number,
			name: `Macro ${number}`,
			source: "",
			presentation: { color: MACRO_COLOR },
		},
	};
}
