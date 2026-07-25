export enum Show {
	Empty = "empty",
	TwelveDimmers = "twelve-dimmers",
	CompactRig = "compact-rig",
	DefaultStage = "default-stage",
}

export interface ShowPrerequisites {
	fixtureNumbers?: readonly number[];
	profiles?: readonly string[];
	groups?: readonly string[];
	desktops?: readonly string[];
}

export interface DefinedShow {
	readonly name: string;
}

export interface RegisteredShow {
	base: Show;
	requires: ShowPrerequisites;
}

export const initialShowCatalog: Readonly<Record<Show, ShowPrerequisites>> = {
	[Show.Empty]: { fixtureNumbers: [], profiles: [], groups: [], desktops: [] },
	[Show.TwelveDimmers]: {
		fixtureNumbers: Array.from({ length: 12 }, (_, index) => index + 1),
		profiles: ["Generic Dimmer"],
		groups: ["1", "2", "3"],
		desktops: [],
	},
	[Show.CompactRig]: {
		fixtureNumbers: [
			...Array.from({ length: 12 }, (_, index) => index + 1),
			21,
			22,
			23,
			24,
		],
		groups: ["1", "2", "3", "4"],
		desktops: [],
	},
	[Show.DefaultStage]: { fixtureNumbers: [1], groups: [], desktops: [] },
};

const recipes = new Map<string, RegisteredShow>();

class ShowRecipeBuilder {
	baseShow: Show | null = null;
	prerequisites: ShowPrerequisites = {};

	from(show: Show): this {
		this.baseShow = show;
		return this;
	}

	requires(prerequisites: ShowPrerequisites): this {
		this.prerequisites = prerequisites;
		return this;
	}
}

export function defineShow(
	name: string,
	recipe: (show: ShowRecipeBuilder) => void,
): DefinedShow {
	if (!name.trim()) throw new Error("Defined show name must not be empty");
	if (recipes.has(name)) {
		throw new Error(`Show recipe "${name}" is already registered`);
	}
	const builder = new ShowRecipeBuilder();
	recipe(builder);
	if (!builder.baseShow) {
		throw new Error(`Show recipe "${name}" must select a catalog base`);
	}
	recipes.set(name, {
		base: builder.baseShow,
		requires: builder.prerequisites,
	});
	return Object.freeze({ name });
}

export function resolveShow(show: Show | DefinedShow): RegisteredShow {
	if (typeof show === "string") return { base: show, requires: {} };
	const recipe = recipes.get(show.name);
	if (!recipe) {
		throw new Error(
			`Unknown show recipe "${show.name}". Available entries: ${availableShowNames().join(", ")}`,
		);
	}
	return recipe;
}

export function isDefinedShow(name: string): boolean {
	return recipes.has(name);
}

export function showName(show: Show | DefinedShow): string {
	return typeof show === "string" ? show : show.name;
}

export function availableShowNames(): string[] {
	return [...Object.values(Show), ...recipes.keys()];
}

export function mergeShowRequirements(
	base: ShowPrerequisites,
	extra: ShowPrerequisites,
): ShowPrerequisites {
	return {
		fixtureNumbers: [
			...new Set([
				...(base.fixtureNumbers ?? []),
				...(extra.fixtureNumbers ?? []),
			]),
		],
		profiles: [
			...new Set([...(base.profiles ?? []), ...(extra.profiles ?? [])]),
		],
		groups: [...new Set([...(base.groups ?? []), ...(extra.groups ?? [])])],
		desktops: [
			...new Set([...(base.desktops ?? []), ...(extra.desktops ?? [])]),
		],
	};
}
