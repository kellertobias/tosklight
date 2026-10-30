import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { PoolGrid } from "../../../apps/ui-library/src/pools";
import { WindowScrollArea } from "../../../apps/ui-library/src/window-kit";
import {
	AppProvider,
	useApp,
} from "../../../apps/light-desktop/src/state/AppContext";
import { PaneScrollRestoration } from "../../../apps/light-desktop/src/components/shell/PaneScrollRestoration";
import type { DeskModel } from "../../../apps/light-desktop/src/types";
import "../../../apps/ui-library/src/styles.css";

const desks: DeskModel[] = ["Early", "Later"].map((name) => ({
	id: name,
	name,
	panes: [
		{
			id: `${name}-groups`,
			kind: "groups",
			title: "Groups",
			x: 1,
			y: 1,
			width: 12,
			height: 12,
		},
	],
}));
function Fixture() {
	const { state, dispatch } = useApp();
	const [content, setContent] = useState(true);
	useEffect(() => {
		dispatch({ type: "HYDRATE_LAYOUT", desks, activeDeskId: "Early" });
	}, []);
	const desk = state.desks.find((item) => item.id === state.activeDeskId)!;
	const pane = desk.panes[0];
	if (!pane) return null;
	return (
		<>
			<header style={{ height: 90 }}>
				{state.desks.map((item) => (
					<button
						key={item.id}
						onClick={() => dispatch({ type: "OPEN_DESK", id: item.id })}
					>
						{item.name}
					</button>
				))}
				<button
					onClick={() => dispatch({ type: "RECORD_DESK_SCROLL", id: desk.id })}
				>
					Record positions
				</button>
				<button
					onClick={() => {
						dispatch({ type: "START_SAVE_DESK" });
						dispatch({ type: "NEW_DESK" });
					}}
				>
					Clone view
				</button>
				<button onClick={() => setContent((current) => !current)}>
					{content ? "Unload content" : "Load content"}
				</button>
				<button
					onClick={() => {
						localStorage.setItem(
							"proof-layout",
							JSON.stringify({
								desks: state.desks,
								activeDeskId: state.activeDeskId,
							}),
						);
						location.reload();
					}}
				>
					Restart layout
				</button>
				<output data-testid="active-view">{desk.name}</output>
				<output data-testid="recorded-state">
					{JSON.stringify(pane.recordedScroll ?? {})}
				</output>
			</header>
			<section
				data-pane-id={pane.id}
				style={{
					height: "calc(100vh - 90px)",
					display: "flex",
					flexDirection: "column",
				}}
			>
				<PaneScrollRestoration
					key={pane.id}
					saved={pane.recordedScroll}
					recall={state.desktopRecallGeneration ?? 0}
				>
					<WindowScrollArea scrollKey="groups">
						{content && (
							<PoolGrid
								minimumCardWidth={100}
								slots={[]}
								slotCount={384}
								emptySlot={(position) => ({
									id: String(position + 1),
									position,
									card: {
										number: position + 1,
										primary: `Group ${position + 1}`,
									},
								})}
							/>
						)}
					</WindowScrollArea>
				</PaneScrollRestoration>
			</section>
		</>
	);
}
function RestoreLayout() {
	const { dispatch } = useApp();
	useEffect(() => {
		const saved = localStorage.getItem("proof-layout");
		if (saved) dispatch({ type: "HYDRATE_LAYOUT", ...JSON.parse(saved) });
	}, []);
	return null;
}
document.body.style.margin = "0";
createRoot(document.getElementById("root")!).render(
	<AppProvider>
		<Fixture />
		<RestoreLayout />
	</AppProvider>,
);
