import { createContext, type ReactNode, useContext } from "react";

export type VisibleEncoderCount = 4 | 5 | 6;

const VisibleEncoderCountContext = createContext<VisibleEncoderCount>(6);

export function VisibleEncoderCountProvider({
	children,
	count,
}: {
	children: ReactNode;
	count: VisibleEncoderCount;
}) {
	return (
		<VisibleEncoderCountContext.Provider value={count}>
			{children}
		</VisibleEncoderCountContext.Provider>
	);
}

export function useVisibleEncoderCount() {
	return useContext(VisibleEncoderCountContext);
}

export function resolveVisibleEncoderCount(
	softwareCount: VisibleEncoderCount,
	_hardwareConnected: boolean,
): VisibleEncoderCount {
	return softwareCount;
}
