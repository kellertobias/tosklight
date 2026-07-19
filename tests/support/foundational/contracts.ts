export interface VersionedObject<T = Record<string, any>> {
	kind: string;
	id: string;
	revision: number;
	body: T;
}

export interface ShowEntry {
	id: string;
	name: string;
}

export interface ProgrammerState {
	selected: string[];
	selection_expression: any;
	values: Array<{
		fixture_id: string;
		attribute: string;
		value: { value?: number } | number;
	}>;
	group_values: Record<
		string,
		Record<string, { value: { value?: number } | number }>
	>;
	command_line: string;
}

export const INTENSITY = "intensity";
