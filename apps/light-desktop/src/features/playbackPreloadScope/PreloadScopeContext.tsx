import { createContext, useContext, type PropsWithChildren } from "react";
import type { PreloadScopeAuthority, PreloadScopeTransport } from "./contracts";
interface Boundary {
	authority: PreloadScopeAuthority | null;
	transport: PreloadScopeTransport | null;
}
const Context = createContext<Boundary | null>(null);
export function PreloadScopeProvider({
	children,
	...boundary
}: PropsWithChildren<Boundary>) {
	return <Context.Provider value={boundary}>{children}</Context.Provider>;
}
export function usePreloadScopeBoundary() {
	return useContext(Context);
}
