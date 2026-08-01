(() => {
	"use strict";

	const config = window.ENCODER_LAYOUT;
	if (!config?.groups?.length) throw new Error("Encoder layout data is missing.");

	const elements = {
		groupList: document.getElementById("group-list"),
		groupKind: document.getElementById("group-kind"),
		groupTitle: document.getElementById("group-title"),
		groupDescription: document.getElementById("group-description"),
		widthOptions: document.getElementById("width-options"),
		pageOptions: document.getElementById("page-options"),
		pageTitle: document.getElementById("page-title"),
		pageSummary: document.getElementById("page-summary"),
		deck: document.getElementById("encoder-deck"),
	};

	const state = {
		groupId: config.defaultGroup ?? config.groups[0].id,
		width: [4, 5, 6].includes(config.defaultWidth) ? config.defaultWidth : 6,
		page: 0,
		selectedEncoderId: null,
	};

	function currentGroup() {
		return config.groups.find((group) => group.id === state.groupId) ?? config.groups[0];
	}

	function orderedBlocks(group) {
		const override = group.orders?.[state.width];
		if (!override) return group.blocks;
		const byId = new Map(
			group.blocks.map((block) => [blockKey(block), block]),
		);
		const ordered = override.map((id) => byId.get(id)).filter(Boolean);
		return [...ordered, ...group.blocks.filter((block) => !ordered.includes(block))];
	}

	function blockKey(block) {
		const first = block[0]?.id ?? "";
		return first.replace(/-(primary|secondary)$/, "");
	}

	function paginate(group) {
		const pages = [];
		let page = [];
		for (const block of orderedBlocks(group)) {
			if (block.length > state.width) {
				if (page.length) pages.push(page);
				for (let index = 0; index < block.length; index += state.width)
					pages.push(block.slice(index, index + state.width));
				page = [];
				continue;
			}
			if (page.length && page.length + block.length > state.width) {
				pages.push(page);
				page = [];
			}
			page.push(...block);
		}
		if (page.length || !pages.length) pages.push(page);
		return pages;
	}

	function button(label, className, onClick, pressed) {
		const control = document.createElement("button");
		control.type = "button";
		control.className = className;
		control.textContent = label;
		control.setAttribute("aria-pressed", String(pressed));
		control.addEventListener("click", onClick);
		return control;
	}

	function renderGroups() {
		elements.groupList.replaceChildren(
			...config.groups.map((group) => {
				const control = button(
					group.label,
					"group-button",
					() => {
						state.groupId = group.id;
						state.page = 0;
						state.selectedEncoderId = null;
						render();
					},
					group.id === state.groupId,
				);
				const count = document.createElement("span");
				count.textContent = String(group.blocks.flat().length);
				control.append(count);
				return control;
			}),
		);
	}

	function renderWidths() {
		elements.widthOptions.replaceChildren(
			...[4, 5, 6].map((width) =>
				button(
					String(width),
					"width-button",
					() => {
						state.width = width;
						state.page = 0;
						state.selectedEncoderId = null;
						render();
					},
					width === state.width,
				),
			),
		);
	}

	function renderPages(pages) {
		if (state.page >= pages.length) state.page = Math.max(0, pages.length - 1);
		elements.pageOptions.replaceChildren(
			...pages.map((_, index) =>
				button(
					`Page ${index + 1}`,
					"page-button",
					() => {
						state.page = index;
						state.selectedEncoderId = null;
						render();
					},
					index === state.page,
				),
			),
		);
	}

	function renderDeck(page) {
		const slots = Array.from({ length: state.width }, (_, index) => page[index] ?? null);
		if (!state.selectedEncoderId || !page.some((item) => item.id === state.selectedEncoderId))
			state.selectedEncoderId = page[0]?.id ?? null;

		elements.deck.style.setProperty("--encoder-count", String(state.width));
		elements.deck.replaceChildren(
			...slots.map((item, index) => {
				const control = document.createElement("button");
				control.type = "button";
				control.className = `encoder-card${item ? "" : " is-empty"}`;
				control.disabled = !item;
				control.setAttribute("aria-pressed", String(item?.id === state.selectedEncoderId));
				control.setAttribute(
					"aria-label",
					item ? `Encoder ${index + 1}, ${item.label}` : `Encoder ${index + 1}, unassigned`,
				);
				if (item) {
					control.addEventListener("click", () => {
						state.selectedEncoderId = item.id;
						renderDeck(page);
					});
				}
				const number = document.createElement("span");
				number.className = "encoder-number";
				number.textContent = `ENC ${index + 1}`;
				const knob = document.createElement("span");
				knob.className = "encoder-knob";
				knob.setAttribute("aria-hidden", "true");
				const marker = document.createElement("span");
				marker.className = "knob-marker";
				knob.append(marker);
				const label = document.createElement("strong");
				label.textContent = item?.label ?? "Unassigned";
				const description = document.createElement("span");
				description.className = "encoder-description";
				description.textContent = item?.description ?? "No control assigned to this encoder.";
				control.append(number, knob, label, description);
				return control;
			}),
		);
	}

	function render() {
		const group = currentGroup();
		const pages = paginate(group);
		renderGroups();
		renderWidths();
		renderPages(pages);
		elements.groupKind.textContent = group.kind;
		elements.groupTitle.textContent = group.label;
		elements.groupDescription.textContent = group.description;
		elements.pageTitle.textContent = `${group.label} · Page ${state.page + 1} of ${pages.length}`;
		elements.pageSummary.textContent = `${state.width} encoders · ${group.blocks.flat().length} controls`;
		renderDeck(pages[state.page]);
	}

	render();
})();
