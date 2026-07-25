import { useCallback, useLayoutEffect, useRef, useState } from "react";

interface OperationState {
	busy: boolean;
	pending: string | null;
	failure: string | null;
}

interface TaggedOperationState extends OperationState {
	generation: number;
}

interface CommittedAuthority {
	scope: readonly unknown[];
	generation: number;
}

const IDLE: OperationState = { busy: false, pending: null, failure: null };

/** Keeps modal-local async feedback inside the authority that started it. */
export function useScopedPageOperation(scope: readonly unknown[]) {
	const authority = useRef<CommittedAuthority>({
		scope: [...scope],
		generation: 1,
	});
	const mounted = useRef(false);
	const pending = useRef<number | null>(null);
	const [state, setState] = useState<TaggedOperationState>({
		...IDLE,
		generation: 1,
	});
	const scopeCommitted = sameScope(authority.current.scope, scope);
	const renderedGeneration =
		authority.current.generation + Number(!scopeCommitted);

	useLayoutEffect(() => {
		if (sameScope(authority.current.scope, scope)) return;
		const generation = authority.current.generation + 1;
		authority.current = { scope: [...scope], generation };
		pending.current = null;
		setState({ ...IDLE, generation });
	});
	useLayoutEffect(() => {
		mounted.current = true;
		return () => {
			mounted.current = false;
			pending.current = null;
		};
	}, []);
	const begin = useCallback((label: string) => {
		if (!mounted.current || pending.current !== null) return null;
		const generation = authority.current.generation;
		pending.current = generation;
		setState({ busy: true, pending: label, failure: null, generation });
		return generation;
	}, []);
	const complete = useCallback(
		(token: number, failure: string | null = null) => {
			if (!mounted.current || token !== authority.current.generation)
				return false;
			pending.current = null;
			setState({
				busy: false,
				pending: null,
				failure,
				generation: token,
			});
			return true;
		},
		[],
	);
	const isCurrent = useCallback(
		(token: number) =>
			mounted.current && token === authority.current.generation,
		[],
	);
	const report = useCallback((failure: string | null) => {
		if (!mounted.current) return;
		const generation = authority.current.generation;
		setState((current) => ({
			...(current.generation === generation ? current : IDLE),
			failure,
			generation,
		}));
	}, []);
	const visible =
		scopeCommitted && state.generation === authority.current.generation
			? state
			: IDLE;
	return {
		...visible,
		generation: renderedGeneration,
		begin,
		complete,
		isCurrent,
		report,
	};
}

function sameScope(left: readonly unknown[], right: readonly unknown[]) {
	return (
		left.length === right.length &&
		left.every((value, index) => Object.is(value, right[index]))
	);
}
