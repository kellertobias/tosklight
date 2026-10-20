import { useState } from "react";
import { createRoot } from "react-dom/client";
import {
	PoolGrid,
	PoolTileLayoutProvider,
	type PoolTileLayout,
} from "../../../apps/ui-library/src/pools";
import { WindowScrollArea } from "../../../apps/ui-library/src/window-kit";
import "../../../apps/ui-library/src/styles.css";

function Fixture() {
	const [layout, setLayout] = useState<PoolTileLayout>({ mode: "automatic" });
	return (
		<>
			<div style={{ height: 50 }}>
				<button onClick={() => setLayout({ mode: "automatic" })}>
					Automatic squares
				</button>
				<button
					onClick={() => setLayout({ mode: "fixed", columns: 4, rows: 3 })}
				>
					Fixed 4 by 3
				</button>
			</div>
			<section
				style={{
					height: "calc(100vh - 50px)",
					display: "flex",
					flexDirection: "column",
				}}
			>
				<PoolTileLayoutProvider layout={layout}>
					<WindowScrollArea>
						<PoolGrid
							columns={2}
							minimumCardWidth={100}
							slots={[]}
							emptySlot={(position) => ({
								id: position + 1,
								position,
								card: {
									number: position + 1,
									primary: `Group ${position + 1}`,
								},
							})}
						/>
					</WindowScrollArea>
				</PoolTileLayoutProvider>
			</section>
		</>
	);
}
document.body.style.margin = "0";
createRoot(document.getElementById("root")!).render(<Fixture />);
