import {
	useEffect,
	useMemo,
	useRef,
	useState,
	type MouseEvent as ReactMouseEvent,
	type PointerEvent as ReactPointerEvent,
	type CSSProperties,
} from "react";
import {
	Button,
	FormLayout,
	HorizontalFaderField,
	MultiValueToggleField,
	NumberField,
	SwitchField,
} from "../components/common";
import { GroupStrip } from "../components/shared/GroupStrip";
import { fixtures as visualFixtures } from "../data/mockData";
import { useServer } from "../api/ServerContext";
import type { WindowProps } from "./windowTypes";
import { Stage3dCanvas } from "./Stage3dCanvas";
import { fixtureValue } from "./fixtureVisualization";
import { migrateStagePosition } from "./stage3dScene";
import type { StagePosition3d } from "../api/ServerContext";
import type { VisualizationSnapshot } from "../api/types";
import { useApp } from "../state/AppContext";
import { WindowHeader, WindowSettings } from "../components/window-kit";

const symbols = ["◉", "◈", "◎", "◐", "◇", "◍"];
type Point = { x: number; y: number };

export function StageWindow({
	compact,
	showGroupShortcuts,
	stageView,
	followPreload: paneFollowPreload,
	showBeamGuides: forcedShowBeamGuides,
	showSelection: forcedShowSelection,
	showFloorGrid: forcedShowFloorGrid,
	environmentBrightness: forcedEnvironmentBrightness,
	camera3d,
	patchSelectionPreview = false,
}: WindowProps & {
	showSelection?: boolean;
	showFloorGrid?: boolean;
	environmentBrightness?: number;
	camera3d?: {
		position: readonly [number, number, number];
		target: readonly [number, number, number];
	};
	patchSelectionPreview?: boolean;
}) {
	const server = useServer();
	const { state, dispatch } = useApp();
	const mode = state.stageMode;
	const setMode = (value: typeof mode) =>
		dispatch({ type: "SET_STAGE_MODE", value });
	const view = compact ? (stageView ?? state.stageView) : state.stageView;
	const setView = (value: "2d" | "3d") =>
		dispatch({ type: "SET_STAGE_VIEW", value });
	const [dedicatedFollowPreload, setDedicatedFollowPreload] = useState(false);
	const lastFollowToggle = useRef(0);
	const followPreload = compact
		? Boolean(paneFollowPreload)
		: dedicatedFollowPreload;
	const [optionsOpen, setOptionsOpen] = useState(false);
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const groupsVisible = compact
		? Boolean(showGroupShortcuts)
		: state.stageGroupsVisible;
	const showSelection = forcedShowSelection ?? state.stageShowSelection;
	const showFloorGrid = forcedShowFloorGrid ?? state.stageShowFloorGrid;
	const showBeamGuides = forcedShowBeamGuides ?? state.stageShowBeamGuides;
	const environmentBrightness =
		forcedEnvironmentBrightness ?? state.stageEnvironmentBrightness;
	const tauri =
		typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
	const zoom = state.stageZoom;
	const [positions, setPositions] = useState<
		Record<string, { x: number; y: number; rotation: number }>
	>({});
	const [positions3d, setPositions3d] = useState<
		Record<string, StagePosition3d>
	>({});
	const positions3dRef = useRef(positions3d);
	const [visualization, setVisualization] =
		useState<VisualizationSnapshot | null>(null);
	const positionsRef = useRef(positions);
	const [draggingFixture, setDraggingFixture] = useState<string | null>(null);
	const [setupFixtureId, setSetupFixtureId] = useState<string | null>(null);
	const pan = { x: state.stagePanX, y: state.stagePanY };
	const navigationStart = useRef<
		(Point & { panX: number; panY: number }) | null
	>(null);
	const selectionAnchor = useRef<string | null>(null);
	const marqueeStart = useRef<(Point & { additive: boolean }) | null>(null);
	const [marquee, setMarquee] = useState<{
		left: number;
		top: number;
		width: number;
		height: number;
	} | null>(null);

	useEffect(() => {
		positionsRef.current = positions;
	}, [positions]);
	useEffect(() => {
		positions3dRef.current = positions3d;
	}, [positions3d]);
	useEffect(() => {
		const nextPositions = server.stageLayout?.body.positions ?? {};
		const nextPositions3d = server.stageLayout?.body.positions3d ?? {};
		positionsRef.current = nextPositions;
		setPositions(nextPositions);
		positions3dRef.current = nextPositions3d;
		setPositions3d(nextPositions3d);
	}, [server.stageLayout]);
	useEffect(() => {
		let cancelled = false;
		const refresh = () =>
			void server
				.readVisualization(followPreload)
				.then((next) => {
					if (!cancelled) setVisualization(next);
				})
				.catch(() => undefined);
		refresh();
		const timer = window.setInterval(refresh, 200);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [followPreload, server.readVisualization]);
	const stageFixtures = useMemo(
		() =>
			[...(server.patch?.fixtures ?? [])].sort(
				(left, right) =>
					(left.virtual_fixture_number ?? Number.MAX_SAFE_INTEGER) -
						(right.virtual_fixture_number ?? Number.MAX_SAFE_INTEGER) ||
					(left.fixture_number ?? Number.MAX_SAFE_INTEGER) -
						(right.fixture_number ?? Number.MAX_SAFE_INTEGER) ||
					left.fixture_id.localeCompare(right.fixture_id),
			),
		[server.patch],
	);
	const patchPreviewFixtures = useMemo(
		() =>
			stageFixtures
				.filter(
					(fixture) =>
						server.selectedFixtures.includes(fixture.fixture_id) ||
						fixture.logical_heads.some((head) =>
							server.selectedFixtures.includes(head.fixture_id),
						),
				)
				.map((fixture) => fixture.fixture_id),
		[stageFixtures, server.selectedFixtures],
	);
	const patched = server.patch
		? stageFixtures.map((fixture, index) => {
				const patchPreviewSelected =
					patchSelectionPreview &&
					patchPreviewFixtures.includes(fixture.fixture_id);
				const intensity = patchPreviewSelected
					? 1
					: (visualization?.blackout
							? 0
							: fixtureValue(visualization, fixture, "intensity")) *
						(visualization?.grand_master ?? 1);
				const red = fixtureValue(visualization, fixture, "color.red", 1);
				const green = fixtureValue(visualization, fixture, "color.green", 1);
				const blue = fixtureValue(visualization, fixture, "color.blue", 1);
				const panValue = fixtureValue(visualization, fixture, "pan");
				const tiltValue = fixtureValue(visualization, fixture, "tilt");
				return {
					fixtureId: fixture.fixture_id,
					fixtureNumber:
						fixture.virtual_fixture_number != null
							? `0.${fixture.virtual_fixture_number}`
							: (fixture.fixture_number ?? index + 1),
					name: fixture.definition.name ?? fixture.definition.model,
					icon: fixture.definition.icon_asset,
					color: `rgb(${Math.round(red * 255)},${Math.round(green * 255)},${Math.round(blue * 255)})`,
					dimmer: Math.round(intensity * 100),
					pan: panValue,
					tilt: tiltValue,
				};
			})
		: [];
	const fixtures = server.bootstrap
		? patched
		: visualFixtures.map((fixture, index) => ({
				fixtureId: "",
				fixtureNumber: index + 1,
				name: fixture.name,
				icon: null,
				color: fixture.color,
				dimmer: fixture.dimmer,
				pan: Math.max(0, Math.min(1, fixture.pan / 360)),
				tilt: Math.max(0, Math.min(1, fixture.tilt / 180)),
			}));
	const orderedFixtureIds = fixtures
		.map((fixture) => fixture.fixtureId)
		.filter(Boolean);
	const columns = compact ? 6 : 8;
	const fixtures3d = useMemo(
		() =>
			stageFixtures.flatMap((fixture, fixtureIndex) => {
				const physical = [
					{
						id: fixture.fixture_id,
						location: fixture.location,
						rotation: fixture.rotation,
					},
					...(fixture.multipatch ?? []),
				];
				return physical.map((instance, instanceIndex) => {
					const index = fixtureIndex * 16 + instanceIndex;
					const stored = positions3d[instance.id];
					const located =
						instance.location &&
						(instance.location.x || instance.location.y || instance.location.z)
							? {
									x: instance.location.x / 1000,
									y: instance.location.y / 1000,
									z: instance.location.z / 1000,
									rotationX: instance.rotation?.x ?? 0,
									rotationY: instance.rotation?.y ?? 0,
									rotationZ: instance.rotation?.z ?? 0,
								}
							: null;
					return {
						fixture,
						instanceId: instance.id,
						index,
						position:
							stored ??
							located ??
							migrateStagePosition(
								instanceIndex ? undefined : positions[fixture.fixture_id],
								index,
							),
					};
				});
			}),
		[stageFixtures, positions3d, positions],
	);
	const save3d = () =>
		server.saveStageLayout({
			version: 2,
			positions: positionsRef.current,
			positions3d: positions3dRef.current,
		});

	const selectFixture = (
		fixtureId: string,
		event: ReactMouseEvent<HTMLButtonElement>,
	) => {
		if (!fixtureId || mode !== "select") return;
		const anchor = selectionAnchor.current;
		if (event.shiftKey && anchor) {
			const from = orderedFixtureIds.indexOf(anchor);
			const to = orderedFixtureIds.indexOf(fixtureId);
			if (from >= 0 && to >= 0) {
				const members = orderedFixtureIds.slice(
					Math.min(from, to),
					Math.max(from, to) + 1,
				);
				for (const member of members)
					void server.selectionGesture({ type: "fixture", fixture_id: member });
			}
		} else {
			const toggled = event.ctrlKey || event.metaKey;
			void server.selectionGesture(
				{ type: "fixture", fixture_id: fixtureId },
				toggled && server.selectedFixtures.includes(fixtureId),
			);
		}
		selectionAnchor.current = fixtureId;
	};
	const moveFixture = (
		fixtureId: string,
		event: ReactPointerEvent<HTMLButtonElement>,
	) => {
		if (mode !== "setup" || draggingFixture !== fixtureId) return;
		const bounds = event.currentTarget.parentElement?.getBoundingClientRect();
		if (!bounds) return;
		setPositions((current) => {
			const next = {
				...current,
				[fixtureId]: {
					x: Math.max(
						2,
						Math.min(94, ((event.clientX - bounds.left) / bounds.width) * 100),
					),
					y: Math.max(
						3,
						Math.min(90, ((event.clientY - bounds.top) / bounds.height) * 100),
					),
					rotation: current[fixtureId]?.rotation ?? 0,
				},
			};
			positionsRef.current = next;
			return next;
		});
	};
	const beginCanvasGesture = (event: ReactPointerEvent<HTMLDivElement>) => {
		if ((event.target as HTMLElement).closest(".stage-fixture")) return;
		if (mode === "navigate") {
			event.currentTarget.setPointerCapture(event.pointerId);
			navigationStart.current = {
				x: event.clientX,
				y: event.clientY,
				panX: pan.x,
				panY: pan.y,
			};
		} else if (mode === "select" && event.button === 0) {
			const bounds = event.currentTarget.getBoundingClientRect();
			event.currentTarget.setPointerCapture(event.pointerId);
			marqueeStart.current = {
				x: event.clientX,
				y: event.clientY,
				additive: event.ctrlKey || event.metaKey,
			};
			setMarquee({
				left: event.clientX - bounds.left,
				top: event.clientY - bounds.top,
				width: 0,
				height: 0,
			});
		}
	};
	const updateCanvasGesture = (event: ReactPointerEvent<HTMLDivElement>) => {
		if (navigationStart.current && mode === "navigate")
			dispatch({
				type: "SET_STAGE_NAVIGATION",
				panX:
					navigationStart.current.panX +
					event.clientX -
					navigationStart.current.x,
				panY:
					navigationStart.current.panY +
					event.clientY -
					navigationStart.current.y,
			});
		const start = marqueeStart.current;
		if (!start || mode !== "select") return;
		const bounds = event.currentTarget.getBoundingClientRect();
		setMarquee({
			left: Math.min(start.x, event.clientX) - bounds.left,
			top: Math.min(start.y, event.clientY) - bounds.top,
			width: Math.abs(event.clientX - start.x),
			height: Math.abs(event.clientY - start.y),
		});
	};
	const finishCanvasGesture = (event: ReactPointerEvent<HTMLDivElement>) => {
		navigationStart.current = null;
		const start = marqueeStart.current;
		marqueeStart.current = null;
		if (!start) return setMarquee(null);
		const left = Math.min(start.x, event.clientX),
			right = Math.max(start.x, event.clientX),
			top = Math.min(start.y, event.clientY),
			bottom = Math.max(start.y, event.clientY);
		const moved = right - left >= 4 || bottom - top >= 4;
		if (moved) {
			const hits = Array.from(
				event.currentTarget.querySelectorAll<HTMLElement>(
					".stage-fixture[data-fixture-id]",
				),
			)
				.filter((node) => {
					const box = node.getBoundingClientRect();
					return (
						box.right >= left &&
						box.left <= right &&
						box.bottom >= top &&
						box.top <= bottom
					);
				})
				.map((node) => node.dataset.fixtureId!)
				.filter(Boolean);
			for (const fixtureId of hits)
				void server.selectionGesture(
					{ type: "fixture", fixture_id: fixtureId },
					start.additive && server.selectedFixtures.includes(fixtureId),
				);
		} else if (!start.additive) void server.setSelection([]);
		setMarquee(null);
	};

	return (
		<div className={`stage-window ${compact ? "compact" : ""}`}>
			{!compact && (
				<WindowHeader
					title="Stage"
					info={{
						primary: `${server.selectedFixtures.length} selected`,
						secondary:
							"Tap to select · Shift for range · Control/Command tracks macro",
					}}
					actions={[
						...(mode === "setup"
							? []
							: [
									[
										{
											id: "follow",
											label: "Follow Preload",
											active: followPreload,
											onClick: () => {
												const now = performance.now();
												if (now - lastFollowToggle.current < 400) return;
												lastFollowToggle.current = now;
												setDedicatedFollowPreload((current) => !current);
											},
										},
									],
								]),
						[
							{
								id: "select",
								label: "Select fixtures",
								active: mode === "select",
								onClick: () => setMode("select"),
							},
							{
								id: "setup",
								label: "Setup positions",
								active: mode === "setup",
								onClick: () => setMode("setup"),
							},
							{
								id: "navigate",
								label: "Navigate",
								active: mode === "navigate",
								onClick: () => setMode("navigate"),
							},
						],
					]}
					settings
					onSettings={(anchor) => {
						setSettingsAnchor(anchor.getBoundingClientRect());
						setOptionsOpen(true);
					}}
				/>
			)}
			{optionsOpen && !compact && (
				<WindowSettings
					modal={false}
					anchor={settingsAnchor}
					title="Stage Settings"
					onClose={() => setOptionsOpen(false)}
					tabs={[
						{
							id: "stage",
							label: "Stage",
							content: (
								<FormLayout labelPlacement="side">
									<MultiValueToggleField
										label="View"
										value={view}
										onChange={setView}
										options={[
											{ value: "2d", label: "2D" },
											{ value: "3d", label: "3D", disabled: !tauri },
										]}
									/>
									<SwitchField
										label="Groups shortcuts"
										checked={groupsVisible}
										onChange={(event) =>
											dispatch({
												type: "SET_STAGE_OPTIONS",
												groupsVisible: event.target.checked,
											})
										}
									/>
									<SwitchField
										label="Show Selection"
										checked={state.stageShowSelection}
										onChange={(event) =>
											dispatch({
												type: "SET_STAGE_OPTIONS",
												showSelection: event.target.checked,
											})
										}
									/>
									<SwitchField
										label="Floor grid"
										checked={state.stageShowFloorGrid}
										onChange={(event) =>
											dispatch({
												type: "SET_STAGE_OPTIONS",
												showFloorGrid: event.target.checked,
											})
										}
									/>
									<SwitchField
										label="Beam direction guides"
										checked={state.stageShowBeamGuides}
										onChange={(event) =>
											dispatch({
												type: "SET_STAGE_OPTIONS",
												showBeamGuides: event.target.checked,
											})
										}
									/>
									<HorizontalFaderField
										label="Environment brightness"
										value={state.stageEnvironmentBrightness}
										minimum={0}
										maximum={2}
										step={0.05}
										display={`${Math.round(state.stageEnvironmentBrightness * 100)}%`}
										onChange={(environmentBrightness) =>
											dispatch({
												type: "SET_STAGE_OPTIONS",
												environmentBrightness,
											})
										}
									/>
								</FormLayout>
							),
						},
					]}
				/>
			)}
			{view === "3d" ? (
				<div className="stage-canvas stage-canvas-3d">
					<Stage3dCanvas
						fixtures={fixtures3d}
						visualization={visualization}
						selected={
							mode === "setup" && setupFixtureId
								? [setupFixtureId]
								: server.selectedFixtures
						}
						virtualHighlight={patchSelectionPreview ? patchPreviewFixtures : []}
						setup={mode === "setup"}
						showSelection={showSelection}
						showFloorGrid={showFloorGrid}
						showBeamGuides={showBeamGuides}
						environmentBrightness={environmentBrightness}
						camera3d={camera3d}
						onSelect={(fixtureId, additive) => {
							if (mode === "setup") {
								setSetupFixtureId(fixtureId);
								return;
							}
							void server.selectionGesture(
								{ type: "fixture", fixture_id: fixtureId },
								additive && server.selectedFixtures.includes(fixtureId),
							);
						}}
						onMove={(fixtureId, position) =>
							setPositions3d((current) => {
								const next = { ...current, [fixtureId]: position };
								positions3dRef.current = next;
								return next;
							})
						}
						onMoveEnd={(fixtureId, position) => {
							const next = { ...positions3dRef.current, [fixtureId]: position };
							positions3dRef.current = next;
							void server.saveStageLayout({
								version: 2,
								positions: positionsRef.current,
								positions3d: next,
							});
						}}
					/>
					{mode === "setup" &&
						(setupFixtureId ?? server.selectedFixtures[0]) &&
						(() => {
							const id = setupFixtureId ?? server.selectedFixtures[0];
							const fixture = fixtures3d.find(
								(item) => item.fixture.fixture_id === id,
							);
							if (!fixture) return null;
							const update = (key: keyof StagePosition3d, value: number) => {
								const next = { ...fixture.position, [key]: value };
								setPositions3d((current) => {
									const result = { ...current, [id]: next };
									positions3dRef.current = result;
									return result;
								});
							};
							return (
								<aside className="stage-3d-inspector">
									<b>Fixture position</b>
									{(
										[
											"x",
											"y",
											"z",
											"rotationX",
											"rotationY",
											"rotationZ",
										] as const
									).map((key) => (
										<NumberField
											key={key}
											label={key}
											allowDecimal={!key.startsWith("rotation")}
											step={key.startsWith("rotation") ? 1 : 0.1}
											value={fixture.position[key]}
											onChange={(event) =>
												update(key, Number(event.target.value))
											}
											onBlur={() => void save3d()}
										/>
									))}
								</aside>
							);
						})()}
				</div>
			) : (
				<div
					className={`stage-canvas stage-mode-${mode}`}
					onPointerDown={beginCanvasGesture}
					onPointerMove={updateCanvasGesture}
					onPointerUp={finishCanvasGesture}
					onPointerCancel={() => {
						navigationStart.current = null;
						marqueeStart.current = null;
						setMarquee(null);
					}}
				>
					{fixtures.length === 0 && (
						<div className="empty-window-message">
							No fixtures are patched in the active show.
						</div>
					)}
					<div
						className="stage-fixture-layer"
						style={{
							transform: `translate(${pan.x}px,${pan.y}px) scale(${zoom})`,
						}}
					>
						{fixtures.slice(0, compact ? 18 : 24).map((fixture, index) => {
							const position = positions[fixture.fixtureId] ?? {
								x: 8 + (index % columns) * (compact ? 15 : 11.5),
								y: 12 + Math.floor(index / columns) * 31,
								rotation: index * 23 - 70,
							};
							return (
								<Button
									data-fixture-id={fixture.fixtureId || undefined}
									onClick={(event) => selectFixture(fixture.fixtureId, event)}
									onPointerDown={(event) => {
										if (mode === "setup" && fixture.fixtureId) {
											event.currentTarget.setPointerCapture(event.pointerId);
											setDraggingFixture(fixture.fixtureId);
										}
									}}
									onPointerMove={(event) =>
										moveFixture(fixture.fixtureId, event)
									}
									onPointerUp={() => {
										if (draggingFixture)
											void server.saveStageLayout({
												version: 2,
												positions: positionsRef.current,
												positions3d: positions3dRef.current,
											});
										setDraggingFixture(null);
									}}
									key={fixture.fixtureId || index}
									className={`stage-fixture ${state.stageShowSelection && server.selectedFixtures.includes(fixture.fixtureId) ? "selected" : ""}`}
									style={
										{
											left: `${position.x}%`,
											top: `${position.y}%`,
											color: fixture.color,
											"--lamp-fill": `${12 + fixture.dimmer * 0.36}%`,
											"--lamp-ring": `${20 + fixture.dimmer * 0.65}%`,
										} as CSSProperties
									}
									aria-label={`${fixture.name}, ${fixture.dimmer}%`}
								>
									<span>
										{fixture.icon ? (
											<img src={fixture.icon} alt="" />
										) : (
											symbols[index % symbols.length]
										)}
										<i
											className="lamp-color-dot"
											style={{ background: fixture.color }}
										/>
									</span>
									<i
										className={`lamp-position-line ${fixture.dimmer > 0 ? "active" : "inactive"}`}
										style={{
											transform: `rotate(${fixture.pan * 360 - 180}deg)`,
											color: fixture.dimmer > 0 ? fixture.color : undefined,
										}}
									>
										<i style={{ left: `${fixture.tilt * 100}%` }} />
									</i>
									<small>{fixture.fixtureNumber}</small>
								</Button>
							);
						})}
					</div>
					{marquee && (
						<div
							className="selection-marquee"
							style={marquee}
							aria-hidden="true"
						/>
					)}
				</div>
			)}
			{groupsVisible && <GroupStrip />}
		</div>
	);
}
