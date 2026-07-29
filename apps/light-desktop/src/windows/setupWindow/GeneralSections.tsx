import { Button, FormLayout, NumberField } from "@tosklight/ui";
import { ShowRecoveryFileManager } from "../../components/setup/ShowRecoveryFileManager";
import {
	useBootstrapSnapshot,
	useSessionSnapshot,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { useConnectionStatus } from "../../features/shellStatus/ShellStatusState";
import { useShowLifecycle } from "../../features/showLifecycle/ShowLifecycleContext";
import type { SetupWindowController } from "./controller";

export function ShowsRecoverySection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const bootstrap = useBootstrapSnapshot();
	const lifecycle = useShowLifecycle();
	const connectionStatus = useConnectionStatus();
	const { draft } = controller;
	return (
		<>
			<h2>Shows & recovery</h2>
			<div className="setup-cards">
				<section>
					<b>{bootstrap?.active_show?.name ?? "No show loaded"}</b>
					<small>
						{bootstrap?.active_show?.updated_at ??
							"Choose a show from the library"}
					</small>
				</section>
				<section>
					<b>{lifecycle?.shows.length ?? 0} library shows</b>
					<small>Portable SQLite files</small>
				</section>
				<section>
					<b>{connectionStatus}</b>
					<small>
						{bootstrap?.active_show ? "Autosave active" : "No active show"}
					</small>
				</section>
			</div>
			{draft && (
				<FormLayout
					className="configuration-form"
					columns={3}
					minColumnWidth={190}
				>
					<NumberField
						label="Autosave interval"
						min="5"
						max="3600"
						value={draft.autosave_interval_seconds}
						onChange={(event) =>
							controller.editDraft({
								...draft,
								autosave_interval_seconds: Number(event.target.value),
							})
						}
						description="5–3600 s between recovery checkpoints"
					/>
				</FormLayout>
			)}
			<ShowRecoveryFileManager
				onOpenFixtureLibrary={() => controller.setFixtureLibraryOpen(true)}
			/>
		</>
	);
}

export function UsersSessionsSection(_props: {
	controller: SetupWindowController;
}) {
	const bootstrap = useBootstrapSnapshot();
	const session = useSessionSnapshot();
	const lifecycle = useShowLifecycle();
	return (
		<>
			<h2>Users & sessions</h2>
			<div className="setup-list">
				{bootstrap?.users.map((user) => (
					<article key={user.id}>
						<b>{user.name}</b>
						<span>{user.enabled ? "Enabled" : "Disabled"}</span>
						<small>
							{user.id === session?.user.id ? "Current operator" : user.id}
						</small>
						{user.enabled && user.id !== session?.user.id && (
							<Button onClick={() => lifecycle?.switchUser(user.name)}>
								Use this operator
							</Button>
						)}
					</article>
				))}
			</div>
		</>
	);
}

export function TimecodeSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	return (
		<>
			<h2>Timecode</h2>
			<div className="setup-list">
				{controller.draft?.timecode_sources.map((source) => (
					<article key={source.source_prefix}>
						<b>{source.source_prefix}</b>
						<span>Priority {source.priority}</span>
						<small>
							{source.fallback ? "Fallback allowed" : "Explicit source only"}
						</small>
					</article>
				))}
			</div>
		</>
	);
}
