import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { usePatchedFixturesView } from "../features/patch/PatchState";
import { useServer } from "../api/ServerContext";
import type { PatchedFixture, VisualizationSnapshot } from "../api/types";
import { Button } from "../components/common";
import { VerticalTouchFader } from "../components/control/VerticalTouchFader";
import { FaderView, WindowHeader } from "../components/window-kit";
import {
	useProgrammingSelectionActions,
	useProgrammingSelectionView,
} from "../features/programmingInteraction/ProgrammingInteractionView";
import {
	normalizedFixtureMutations,
	programmerValuesMutationKey,
	useProgrammerValuesMutationQueue,
} from "../features/programmerValues/useProgrammerValuesMutationQueue";
import { useVisualizationRuntimeSnapshot } from "../features/visualizationRuntime/VisualizationRuntimeView";
import { fixtureValue } from "./fixtureVisualization";
import type { WindowProps } from "./windowTypes";

const PAGE_SIZE = 20;

interface Channel {
	number: number;
	fixture: PatchedFixture;
	name: string;
	level: number;
}

export function ChannelsWindow({ active = true, compact }: WindowProps) {
	const server = useServer();
	const selection = useProgrammingSelectionView(active);
	const selectionActions = useProgrammingSelectionActions(active);
	const values = useProgrammerValuesMutationQueue(active);
	const [page, setPage] = useState(0);
	const [pagePickerOpen, setPagePickerOpen] = useState(false);
	const visualization = useChannelVisualization(active);
	const selectedFixtureIds = useMemo(
		() => new Set(selection?.selected ?? []),
		[selection?.selected],
	);
	const fixtures = usePatchedFixturesView(active);
	const channels = channelProjection(fixtures, visualization);
	const pages = Math.max(8, Math.ceil(channels.length / PAGE_SIZE));
	const setIntensity = (fixtureId: string, level: number) => {
		const mutations = normalizedFixtureMutations(
			[{ fixtureId, attribute: "intensity", value: level }],
			server.configuration?.programmer_fade_millis,
		);
		return values.submitLatest(
			programmerValuesMutationKey(mutations),
			mutations,
		);
	};
	usePagePickerDismissal(pagePickerOpen, setPagePickerOpen);
	return (
		<div className="channels-window">
			{!compact && (
				<ChannelHeader
					page={page}
					pages={pages}
					onPage={setPage}
					onOpenPicker={() => setPagePickerOpen(true)}
				/>
			)}
			<ChannelFaderBank
				channels={channels.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE)}
				page={page}
				selectedFixtureIds={selectedFixtureIds}
				valuesReady={values.canWrite}
				onSelect={(fixtureId) =>
					void selectionActions?.replace({ resolvedFixtures: [fixtureId] })
				}
				onSetIntensity={(fixtureId, level) =>
					void setIntensity(fixtureId, level)
				}
			/>
			{pagePickerOpen && (
				<ChannelPagePicker
					page={page}
					pages={pages}
					onPage={setPage}
					onClose={() => setPagePickerOpen(false)}
				/>
			)}
		</div>
	);
}

function useChannelVisualization(active: boolean) {
	return useVisualizationRuntimeSnapshot({
		enabled: active,
		intervalMillis: 250,
	});
}

function channelProjection(
	fixtures: readonly PatchedFixture[],
	visualization: VisualizationSnapshot | null,
): Channel[] {
	return fixtures.map((fixture, index) => ({
		number: index + 1,
		fixture,
		name: fixture.definition.name ?? fixture.definition.model,
		level: Math.round(fixtureValue(visualization, fixture, "intensity") * 100),
	}));
}

function ChannelHeader({
	page,
	pages,
	onPage,
	onOpenPicker,
}: {
	page: number;
	pages: number;
	onPage(page: number): void;
	onOpenPicker(): void;
}) {
	return (
		<WindowHeader
			title="Channels"
			info={{ primary: "Intensity", secondary: "Two-row channel bank" }}
			actions={[
				[
					{
						id: "previous",
						label: "←",
						disabled: page === 0,
						ariaLabel: "Previous channel page",
						onClick: () => onPage(page - 1),
					},
					{
						id: "page",
						label: pageLabel(page),
						onClick: onOpenPicker,
					},
					{
						id: "next",
						label: "→",
						disabled: page >= pages - 1,
						ariaLabel: "Next channel page",
						onClick: () => onPage(page + 1),
					},
				],
			]}
		/>
	);
}

function ChannelFaderBank({
	channels,
	page,
	selectedFixtureIds,
	valuesReady,
	onSelect,
	onSetIntensity,
}: {
	channels: readonly Channel[];
	page: number;
	selectedFixtureIds: ReadonlySet<string>;
	valuesReady: boolean;
	onSelect(fixtureId: string): void;
	onSetIntensity(fixtureId: string, level: number): void;
}) {
	const visible = Array.from(
		{ length: PAGE_SIZE },
		(_, index) => channels[index] ?? null,
	);
	return (
		<FaderView rows={2} className="channel-fader-bank">
			{visible.map((channel, index) => {
				const number = page * PAGE_SIZE + index + 1;
				return (
					<article
						className={`channel-fader ${channel ? "" : "empty"} ${channel && selectedFixtureIds.has(channel.fixture.fixture_id) ? "selected" : ""}`}
						key={channel?.fixture.fixture_id ?? `empty-${number}`}
						onClick={() => channel && onSelect(channel.fixture.fixture_id)}
					>
						<VerticalTouchFader
							disabled={!channel || !valuesReady}
							label={channel ? `CH ${number}` : `CH ${number} · Empty`}
							mode={channel?.name ?? "Unpatched"}
							value={channel?.level ?? 0}
							display={channel ? `${channel.level}%` : "—"}
							onChange={(value) =>
								channel &&
								onSetIntensity(channel.fixture.fixture_id, value / 100)
							}
						/>
					</article>
				);
			})}
		</FaderView>
	);
}

function ChannelPagePicker({
	page,
	pages,
	onPage,
	onClose,
}: {
	page: number;
	pages: number;
	onPage(page: number): void;
	onClose(): void;
}) {
	return createPortal(
		<div
			className="stacked-modal-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && onClose()
			}
		>
			<div
				className="nested-modal channel-page-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Channel pages"
			>
				<Button className="modal-close" onClick={onClose}>
					×
				</Button>
				<h3>Channel pages</h3>
				<div>
					{Array.from({ length: pages }, (_, nextPage) => (
						<Button
							className={nextPage === page ? "active" : ""}
							key={nextPage}
							onClick={() => {
								onPage(nextPage);
								onClose();
							}}
						>
							{pageLabel(nextPage)}
						</Button>
					))}
				</div>
			</div>
		</div>,
		document.body,
	);
}

function usePagePickerDismissal(
	open: boolean,
	setOpen: (open: boolean) => void,
) {
	useEffect(() => {
		if (!open) return;
		const close = (event: KeyboardEvent) => {
			if (event.key !== "Escape") return;
			event.preventDefault();
			setOpen(false);
		};
		window.addEventListener("keydown", close, true);
		return () => window.removeEventListener("keydown", close, true);
	}, [open, setOpen]);
}

function pageLabel(page: number) {
	return `${page * PAGE_SIZE + 1}–${(page + 1) * PAGE_SIZE}`;
}
