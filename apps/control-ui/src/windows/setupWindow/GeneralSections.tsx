import { Button } from "../../components/common";
import { ShowRecoveryFileManager } from "../../components/setup/ShowRecoveryFileManager";
import type { SetupWindowController } from "./controller";

export function ShowsRecoverySection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { server } = controller;
	return (
		<>
			<h2>Shows & recovery</h2>
			<div className="setup-cards">
				<section>
					<b>{server.bootstrap?.active_show?.name ?? "No show loaded"}</b>
					<small>
						{server.bootstrap?.active_show?.updated_at ??
							"Choose a show from the library"}
					</small>
				</section>
				<section>
					<b>{server.shows.length} library shows</b>
					<small>Portable SQLite files</small>
				</section>
				<section>
					<b>{server.status}</b>
					<small>
						{server.bootstrap?.active_show
							? "Autosave active"
							: "No active show"}
					</small>
				</section>
			</div>
			<ShowRecoveryFileManager
				onOpenFixtureLibrary={() => controller.setFixtureLibraryOpen(true)}
			/>
		</>
	);
}

export function UsersSessionsSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { server } = controller;
	return (
		<>
			<h2>Users & sessions</h2>
			<div className="setup-list">
				{server.bootstrap?.users.map((user) => (
					<article key={user.id}>
						<b>{user.name}</b>
						<span>{user.enabled ? "Enabled" : "Disabled"}</span>
						<small>
							{user.id === server.session?.user.id
								? "Current operator"
								: user.id}
						</small>
						{user.enabled && user.id !== server.session?.user.id && (
							<Button onClick={() => server.switchUser(user.name)}>
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
