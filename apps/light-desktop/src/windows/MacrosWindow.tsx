import { Button, TextArea, TextField } from "@tosklight/ui";
import {
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createLightApi } from "../api/client/api";
import { MacrosApiClient } from "../api/client/macros";
import type {
	MacroDefinition,
	MacroExecutionSnapshot,
	MacroLineDiagnostic,
	MacroValidation,
} from "../api/generated/light-wire";
import type { VersionedObject } from "../api/types";
import { useCommandLineSurface } from "../components/control/commandLine/useCommandLineSurface";
import { useActiveShowId } from "../features/deskSnapshot/DeskSnapshotState";
import { useMacroActions } from "../features/macros/MacroActionsContext";
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
		observeCommand: false,
	});
	const fallback = useMemo(() => {
		const api = createLightApi();
		return {
			macros: new MacrosApiClient(api.runtime.capabilityTransport()),
			showObjects: api.showObjects,
		};
	}, []);
	const actions = useMacroActions() ?? fallback;
	const [macros, setMacros] = useState<MacroObject[]>([]);
	const [executions, setExecutions] = useState<MacroExecutionSnapshot[]>([]);
	const [editing, setEditing] = useState<MacroObject | NewMacro | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);

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
		update();
		const timer = window.setInterval(update, 1_000);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [active, refresh, showId]);

	const run = async (macro: MacroObject) => {
		if (!showId || busy) return;
		setBusy(true);
		setError(null);
		try {
			await actions.macros.run(showId, macro.id, {
				source_revision: macro.revision,
				trigger: { type: "pool" },
			});
			await refresh();
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

	return editing ? (
		<MacroEditor
			showId={showId}
			macro={editing}
			api={actions.macros}
			onClose={() => setEditing(null)}
			onSaved={async () => {
				await refresh();
				setEditing(null);
			}}
		/>
	) : (
		<section className="macro-window" aria-busy={busy}>
			{!compact && (
				<WindowHeader
					title="Macros"
					info={{ primary: `${macros.length} Macros` }}
					actions={[]}
				/>
			)}
			{error && (
				<p role="alert" className="macro-error">
					{error}
				</p>
			)}
			<WindowScrollArea>
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
						const setClick = /^SET$/i.test(command.read().text.trim());
						return (
							<PoolCard
								key={number}
								aria-label={
									macro
										? `Macro ${number} ${macro.body.name}`
										: `Empty Macro ${number}`
								}
								aria-pressed={macro ? running.has(macro.id) : undefined}
								model={{
									number,
									primary: macro?.body.name ?? "Empty",
									secondary: macro
										? `${macro.body.source.split("\n").filter((line) => line.trim()).length} lines`
										: "Tap to create",
									icon: macro?.body.presentation.icon,
									color: macro?.body.presentation.color ?? MACRO_COLOR,
									states: [
										...(!macro ? (["empty"] as const) : []),
										...(macro && running.has(macro.id)
											? (["active"] as const)
											: []),
										...(macro && setClick ? (["set-target"] as const) : []),
									],
								}}
								onClick={() => {
									const outcome = resolveMacroPoolGesture(
										Boolean(macro),
										setClick,
										false,
									);
									if (outcome === "create") setEditing(newMacro(number));
									else if (outcome === "edit" && macro) void open(macro);
									else if (macro) void run(macro);
								}}
								onContextMenu={(event) => {
									event.preventDefault();
									const outcome = resolveMacroPoolGesture(
										Boolean(macro),
										setClick,
										true,
									);
									if (outcome === "create") setEditing(newMacro(number));
									else if (macro) void open(macro);
								}}
							/>
						);
					}}
				/>
			</WindowScrollArea>
		</section>
	);
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

interface MacroEditorProps {
	showId: string | null;
	macro: MacroObject | NewMacro;
	api: MacrosApiClient;
	onClose(): void;
	onSaved(): Promise<void>;
}

function MacroEditor({
	showId,
	macro,
	api,
	onClose,
	onSaved,
}: MacroEditorProps) {
	const [draft, setDraft] = useState(macro.body);
	const [savedBody, setSavedBody] = useState(macro.body);
	const [revision, setRevision] = useState(macro.revision);
	const [validation, setValidation] = useState<MacroValidation | null>(null);
	const [notice, setNotice] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const [undo, setUndo] = useState<{
		body: MacroDefinition;
		revision: number;
	} | null>(null);
	const editor = useRef<HTMLTextAreaElement | null>(null);
	const isNew = "isNew" in macro;

	useEffect(() => {
		if (!showId) return;
		const timer = window.setTimeout(() => {
			void api
				.validate(showId, draft.source)
				.then(setValidation)
				.catch((reason) => setNotice(String(reason)));
		}, 180);
		return () => window.clearTimeout(timer);
	}, [api, draft.source, showId]);

	const save = async () => {
		if (!showId || busy || validation?.valid === false) return;
		setBusy(true);
		setNotice(null);
		try {
			const before = savedBody;
			const outcome = isNew
				? await api.create(showId, draft)
				: await api.update(showId, macro.id, revision, {
						number: draft.number,
						name: draft.name,
						source: draft.source,
						presentation: draft.presentation,
					});
			setRevision(outcome.object.revision);
			setUndo({ body: before, revision: outcome.object.revision });
			setSavedBody(draft);
			setNotice("Saved");
			if (isNew) await onSaved();
		} catch (reason) {
			setNotice(
				`Save conflict or failure: ${String(reason)}. Your draft is preserved.`,
			);
		} finally {
			setBusy(false);
		}
	};

	const undoSave = async () => {
		if (!showId || !undo || busy) return;
		setBusy(true);
		try {
			const outcome = await api.update(showId, macro.id, undo.revision, {
				number: undo.body.number,
				name: undo.body.name,
				source: undo.body.source,
				presentation: undo.body.presentation,
			});
			setDraft(undo.body);
			setSavedBody(undo.body);
			setRevision(outcome.object.revision);
			setUndo(null);
			setNotice("Last save undone as a new guarded revision");
		} catch (reason) {
			setNotice(
				`Undo refused because the saved Macro changed: ${String(reason)}`,
			);
		} finally {
			setBusy(false);
		}
	};

	const runLine = async () => {
		if (!showId || isNew || draft.source !== savedBody.source) {
			setNotice("Save this revision before running one of its lines.");
			return;
		}
		const line = draft.source
			.slice(0, editor.current?.selectionStart ?? 0)
			.split("\n").length;
		try {
			await api.runLine(showId, macro.id, { source_revision: revision, line });
			setNotice(`Queued line ${line}`);
		} catch (reason) {
			setNotice(String(reason));
		}
	};

	const copy = async () => {
		if (!showId || isNew || busy) return;
		const requested = window.prompt(
			"Copy to Macro number",
			String(draft.number + 1),
		);
		if (requested == null) return;
		const number = Number(requested);
		if (!Number.isInteger(number) || number < 1 || number > 65_535) {
			setNotice("Enter a Macro number from 1 to 65535.");
			return;
		}
		setBusy(true);
		try {
			await api.create(showId, {
				...savedBody,
				id: crypto.randomUUID(),
				number,
				name: `${savedBody.name} Copy`,
			});
			setNotice(`Copied to Macro ${number}`);
		} catch (reason) {
			setNotice(`Copy failed: ${String(reason)}`);
		} finally {
			setBusy(false);
		}
	};

	const remove = async () => {
		if (
			!showId ||
			isNew ||
			!window.confirm(`Delete Macro ${draft.number} · ${draft.name}?`)
		)
			return;
		setBusy(true);
		try {
			await api.delete(showId, macro.id, revision);
			await onSaved();
		} catch (reason) {
			setNotice(String(reason));
		} finally {
			setBusy(false);
		}
	};

	return (
		<section className="macro-editor" aria-label={`Edit Macro ${draft.number}`}>
			<WindowHeader
				title={`Macro ${draft.number} · ${draft.name}`}
				info={{
					primary: validation?.valid === false ? "Invalid" : "Command Macro",
				}}
				actions={[
					[
						{ id: "back", label: "Back", onClick: onClose },
						{
							id: "run-line",
							label: "Run line",
							onClick: () => void runLine(),
						},
						{
							id: "copy",
							label: "Copy",
							disabled: isNew || busy,
							onClick: () => void copy(),
						},
						{
							id: "undo",
							label: "Undo save",
							disabled: !undo,
							onClick: () => void undoSave(),
						},
						{
							id: "save",
							label: busy ? "Saving…" : "Save",
							disabled: busy || validation?.valid === false,
							onClick: () => void save(),
						},
					],
				]}
			/>
			<div className="macro-editor-identity">
				<TextField
					label="Number"
					value={String(draft.number)}
					onChange={(event) =>
						setDraft({ ...draft, number: Number(event.target.value) })
					}
				/>
				<TextField
					label="Name"
					value={draft.name}
					onChange={(event) => setDraft({ ...draft, name: event.target.value })}
				/>
				<TextField
					label="Icon"
					value={draft.presentation.icon ?? ""}
					onChange={(event) =>
						setDraft({
							...draft,
							presentation: {
								...draft.presentation,
								icon: event.target.value || undefined,
							},
						})
					}
				/>
				{!isNew && (
					<Button
						className="danger"
						disabled={busy}
						onClick={() => void remove()}
					>
						Delete
					</Button>
				)}
			</div>
			<div className="macro-source-editor">
				<LineNumbers
					source={draft.source}
					diagnostics={validation?.diagnostics ?? []}
				/>
				<TextArea
					ref={editor}
					aria-label="Macro command lines"
					value={draft.source}
					spellCheck={false}
					onChange={(event) =>
						setDraft({ ...draft, source: event.target.value })
					}
					onKeyDown={(event) => {
						if (
							(event.metaKey || event.ctrlKey) &&
							event.key.toLowerCase() === "s"
						) {
							event.preventDefault();
							void save();
						}
					}}
				/>
			</div>
			<MacroDiagnostics diagnostics={validation?.diagnostics ?? []} />
			{notice && (
				<p
					role={
						notice.includes("failure") || notice.includes("refused")
							? "alert"
							: "status"
					}
				>
					{notice}
				</p>
			)}
		</section>
	);
}

function LineNumbers({
	source,
	diagnostics,
}: {
	source: string;
	diagnostics: MacroLineDiagnostic[];
}) {
	const byLine = new Map(
		diagnostics.map((diagnostic) => [diagnostic.line, diagnostic]),
	);
	return (
		<ol className="macro-line-numbers" aria-hidden="true">
			{source.split("\n").map((_, index) => {
				const line = index + 1;
				return (
					<li key={line} className={byLine.get(line)?.status ?? "blank"}>
						{line}
					</li>
				);
			})}
		</ol>
	);
}

function MacroDiagnostics({
	diagnostics,
}: {
	diagnostics: MacroLineDiagnostic[];
}) {
	const messages = diagnostics.filter(
		(diagnostic) => diagnostic.status !== "valid",
	);
	if (!messages.length)
		return (
			<p className="macro-validation-ok" role="status">
				All command lines are valid.
			</p>
		);
	return (
		<ul className="macro-diagnostics">
			{messages.map((diagnostic) => (
				<li key={diagnostic.line}>
					<b>Line {diagnostic.line}</b> · {diagnostic.message}
				</li>
			))}
		</ul>
	);
}
