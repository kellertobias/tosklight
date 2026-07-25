export function renderHtml(catalog) {
	const json = JSON.stringify(catalog)
		.replaceAll("<", "\\u003c")
		.replaceAll(">", "\\u003e")
		.replaceAll("&", "\\u0026");
	const scenarios = catalog.scenarios.map(renderScenario).join("");
	const tree = renderTestTree(catalog.scenarios);
	return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>ToskLight semantic test catalog</title>
<style>
:root{color-scheme:dark;--header-height:0px;--bg:#101419;--panel:#171d24;--line:#2b3541;--text:#edf3f8;--muted:#9caebe;--accent:#58c7ff;--warn:#ffbf69;--good:#66d39e}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:15px/1.5 system-ui,sans-serif}
header{position:sticky;top:0;z-index:3;padding:1.25rem max(1rem,calc((100% - 1400px)/2));background:#101419f2;border-bottom:1px solid var(--line);backdrop-filter:blur(12px)}
h1{margin:0 0 .65rem;font-size:1.45rem}.summary{color:var(--muted);margin:.5rem 0}
.controls{display:flex;align-items:center;gap:.75rem}input[type=search]{flex:1;min-width:10rem;padding:.8rem 1rem;border:1px solid var(--line);border-radius:.5rem;background:var(--panel);color:var(--text);font:inherit}.source-toggle{display:flex;align-items:center;gap:.4rem;white-space:nowrap;color:#cbd9e4;cursor:pointer}.source-toggle input{accent-color:var(--accent)}
.layout{display:grid;grid-template-columns:280px minmax(0,1100px);gap:1rem;justify-content:center;align-items:start;padding:1rem}.test-tree{position:sticky;top:calc(var(--header-height) + 1rem);max-height:calc(100vh - var(--header-height) - 2rem);overflow:auto;padding:.8rem;background:var(--panel);border:1px solid var(--line);border-radius:.65rem}.test-tree h2{margin:0 0 .65rem;font-size:.95rem}.tree-suite{margin:.35rem 0}.tree-suite[hidden],.tree-test[hidden]{display:none}.tree-suite>summary{display:flex;align-items:center;gap:.4rem;cursor:pointer;color:#d7e2eb;font-size:.82rem;font-weight:700}.tree-suite>summary::before{content:"▸";color:var(--muted)}.tree-suite[open]>summary::before{content:"▾"}.tree-suite>summary::marker{content:""}.tree-count{margin-left:auto;color:var(--muted);font:11px ui-monospace,monospace}.tree-suite ul{list-style:none;margin:.3rem 0 .6rem .45rem;padding:0 0 0 .65rem;border-left:1px solid var(--line)}.tree-test a{display:block;padding:.24rem .35rem;border-radius:.3rem;color:#b9c9d6;text-decoration:none;font-size:.78rem;line-height:1.35}.tree-test a:hover,.tree-test a:focus{background:#24303b;color:#fff}.tree-id{color:var(--accent);font-family:ui-monospace,monospace}.tree-type{display:inline-block;margin-right:.3rem;padding:.02rem .25rem;border-radius:.25rem;background:#24303b;color:var(--muted);font-size:.62rem;text-transform:uppercase;letter-spacing:.03em}main{min-width:0}.scenario{scroll-margin-top:calc(var(--header-height) + 1rem);margin:0 0 1rem;padding:1rem 1.1rem;background:var(--panel);border:1px solid var(--line);border-radius:.65rem}
.scenario[hidden]{display:none}h2{font-size:1.08rem;margin:0}.id{color:var(--accent);font-family:ui-monospace,monospace}.meta,.empty{color:var(--muted)}
.chips{display:flex;flex-wrap:wrap;gap:.35rem;margin:.65rem 0}.chip{padding:.12rem .48rem;border-radius:1rem;background:#24303b;color:#cbd9e4;font-size:.8rem}
.context-chip{background:#213d61;border:1px solid #4e89c7;color:#e8f3ff;font-weight:650}
h3{font-size:.9rem;margin:1rem 0 .3rem;color:#c8d5df}details{margin:.7rem 0}summary{cursor:pointer;color:#c8d5df;font-weight:650}
table{width:100%;margin:.4rem 0 1rem;border-collapse:collapse;font-size:.9rem}th,td{padding:.48rem .55rem;border:1px solid var(--line);text-align:left;vertical-align:top}th{background:#202832;color:#c8d5df}td.kind{width:8.5rem}.kind-label{display:inline-block;padding:.08rem .4rem;border-radius:.3rem;font-size:.75rem;font-weight:700;text-transform:uppercase;letter-spacing:.025em}.kind-step{background:#183d50;color:#8edbff}.kind-tool,.tool-summary{background:#302959;color:#d6ccff}.kind-outcome{background:#183c2c;color:#8ce0ae}.tool-summary{display:inline-block;margin-left:.4rem;padding:.04rem .34rem;border:1px solid #665a9c;border-radius:1rem;font-size:.7rem;font-weight:700;vertical-align:middle;cursor:help}.tool-row{display:none}.show-tools .tool-row{display:table-row}.source{width:4.5rem;color:var(--muted);font-family:ui-monospace,monospace}
.contract-main+.config-code{margin-top:.45rem}.config-code,.source-code{margin:.45rem 0 0;padding:.55rem .65rem;overflow:auto;border:1px solid #334454;border-radius:.4rem;background:#101820;color:#cbd9e4;font:12.5px/1.45 ui-monospace,monospace;white-space:pre-wrap}.source-code{display:none;border-color:#3e4b59;background:#0d1218;color:#b9c8d5}.show-source .source-code{display:block}.inline-badges{display:inline-flex;flex-wrap:wrap;gap:.3rem;margin-left:.4rem;vertical-align:middle}.item-badge{padding:.05rem .35rem;border:1px solid #53687a;border-radius:1rem;background:#26333e;color:#d5e2ec;font-size:.72rem}.json-key{color:#7dcfff}.json-string{color:#a8d991}.json-number{color:#e6bc83}.json-literal{color:#d5a6ff}.json-symbol{color:#82aaff}.generated-token,.observed-token{display:inline-block;padding:.02rem .3rem;border:1px solid #4e89c7;border-radius:.28rem;background:#1d3854;color:#d9edff;font:600 .86em/1.45 ui-monospace,monospace}.observed-token{border-color:#8773d1;background:#302959;color:#e8e2ff}.unresolved-token{display:inline-block;padding:.02rem .3rem;border:1px solid #ff5868;border-radius:.28rem;background:#551d28;color:#ffd9dd;font:600 .86em/1.45 ui-monospace,monospace}.diagnostic-code{display:inline-block;padding:.05rem .35rem;border-radius:.28rem;background:#4b2e17;color:#ffd2a0;font:600 .75rem ui-monospace,monospace}.diagnostic-where{color:#8fc9ff;font-weight:650}.diagnostic{color:var(--warn)}.passed{color:var(--good)}code{font-family:ui-monospace,monospace}tr:target{outline:2px solid var(--accent);outline-offset:-2px}@media(max-width:900px){.layout{display:block}.test-tree{position:static;max-height:18rem;margin-bottom:1rem}}@media(max-width:700px){.controls{align-items:stretch;flex-direction:column}.source-toggle{align-self:flex-start}td.kind,.source{width:auto}th,td{padding:.4rem}.scenario{padding:.8rem}}
</style>
</head>
<body>
<header id="page-header"><h1>ToskLight semantic Playwright scenarios</h1><div class="controls"><input id="search" type="search" placeholder="Search ID, title, steps, outcomes, surfaces, or status" aria-label="Search scenarios"><label class="source-toggle"><input id="show-tools" type="checkbox"> Show tool calls</label><label class="source-toggle"><input id="show-source" type="checkbox"> Show source code</label></div><p class="summary" id="summary"></p></header>
<div class="layout"><aside class="test-tree" aria-label="Test suites and test cases"><h2>Test tree</h2>${tree}</aside><main id="catalog">${scenarios}</main></div>
<script id="catalog-data" type="application/json">${json}</script>
<script>
const data=JSON.parse(document.getElementById("catalog-data").textContent);
const host=document.getElementById("catalog"),pageHeader=document.getElementById("page-header"),search=document.getElementById("search"),summary=document.getElementById("summary"),sourceToggle=document.getElementById("show-source"),toolsToggle=document.getElementById("show-tools");
function filter(){const term=search.value.trim().toLowerCase();let visible=0;for(const node of host.children){node.hidden=term&&!node.dataset.search.includes(term);const treeNode=document.querySelector('[data-tree-test="'+node.id+'"]');if(treeNode)treeNode.hidden=node.hidden;if(!node.hidden)visible++}for(const suite of document.querySelectorAll(".tree-suite"))suite.hidden=![...suite.querySelectorAll(".tree-test")].some(node=>!node.hidden);summary.textContent=visible+" of "+data.scenarios.length+" scenarios · schema v"+data.schemaVersion}
function syncHeaderHeight(){document.documentElement.style.setProperty("--header-height",pageHeader.getBoundingClientRect().height+"px")}
search.addEventListener("input",filter);sourceToggle.addEventListener("change",()=>document.body.classList.toggle("show-source",sourceToggle.checked));toolsToggle.addEventListener("change",()=>document.body.classList.toggle("show-tools",toolsToggle.checked));new ResizeObserver(syncHeaderHeight).observe(pageHeader);syncHeaderHeight();filter();
</script>
</body>
</html>
`;
}

function renderScenario(scenario) {
	const search = escapeHtml(JSON.stringify(scenario).toLowerCase());
	const run = scenario.lastRun
		? `<span class="${scenario.lastRun.status === "passed" ? "passed" : ""}">last run: ${escapeHtml(scenario.lastRun.status)}</span>`
		: "last run: not merged";
	const context = (scenario.contextLabels ?? [])
		.map(
			(item) =>
				`<span class="chip context-chip">${escapeHtml(item.kind)}: ${escapeHtml(item.label)}</span>`,
		)
		.join("");
	const surfaces = scenario.testedSurfaces
		.map((surface) => `<span class="chip">${escapeHtml(surface)}</span>`)
		.join("");
	return `<article class="scenario" id="${scenarioAnchor(scenario)}" data-search="${search}">
<h2><span class="id">${escapeHtml(scenario.id)}</span> — ${escapeHtml(scenario.title)}</h2>
<p class="meta">${escapeHtml(scenario.source.file)}:${scenario.source.line} · ${escapeHtml(scenario.migration.status)} · ${run}</p>
<div class="chips">${context}${surfaces}</div>
${renderPreconditions(scenario)}
<h3>Scenario contract</h3>
${renderContractTable(scenario)}
<h3>Diagnostics</h3>
${renderDiagnostics(scenario)}
</article>`;
}

function renderPreconditions(scenario) {
	if (!scenario.preconditions?.length) return "";
	const rows = scenario.preconditions
		.map(
			(item) =>
				`<tr id="${rowAnchor(scenario, item)}"><td>${highlightTokens(item.description)}${renderSourceCode(item)}</td><td class="source">${escapeHtml(item.source.line)}</td></tr>`,
		)
		.join("");
	return `<details><summary>Preconditions (${scenario.preconditions.length})</summary><table><thead><tr><th>Context</th><th>Line</th></tr></thead><tbody>${rows}</tbody></table></details>`;
}

function renderTestTree(scenarios) {
	const suites = new Map();
	for (const scenario of scenarios) {
		const suite = suites.get(scenario.source.file) ?? [];
		suite.push(scenario);
		suites.set(scenario.source.file, suite);
	}
	return [...suites]
		.map(([file, tests]) => {
			const items = tests
				.map(
					(scenario) =>
						`<li class="tree-test" data-tree-test="${scenarioAnchor(scenario)}"><a href="#${scenarioAnchor(scenario)}"><span class="tree-type">Test</span><span class="tree-id">${escapeHtml(scenario.id)}</span> ${escapeHtml(scenario.title)}</a></li>`,
				)
				.join("");
			return `<details class="tree-suite" open><summary><span class="tree-type">Suite</span>${escapeHtml(file.split("/").at(-1))}<span class="tree-count">${tests.length}</span></summary><ul>${items}</ul></details>`;
		})
		.join("");
}

function renderContractTable(scenario) {
	const entries = [
		...scenario.steps.map((item) => ({
			kind: item.kind === "tool" ? "tool" : "step",
			item,
		})),
		...scenario.expectedOutcomes.map((item) => ({ kind: "outcome", item })),
	].sort((a, b) => (a.item.order ?? 0) - (b.item.order ?? 0));
	if (!entries.length)
		return '<p class="empty">No statically visible scenario contract.</p>';
	let previousVisible;
	for (const entry of entries) {
		if (entry.kind === "tool") {
			if (previousVisible) {
				previousVisible.tools ??= [];
				previousVisible.tools.push(entry.item);
			}
		} else previousVisible = entry;
	}
	const rows = entries
		.map(
			({ kind, item, tools }) =>
				`<tr class="${kind === "tool" ? "tool-row" : ""}" id="${rowAnchor(scenario, item)}"><td class="kind"><span class="kind-label kind-${kind}">${kind === "step" ? "Step" : kind === "tool" ? "Tool" : "Expected"}</span></td><td>${renderContract(item, renderToolSummary(tools))}${renderSourceCode(item)}</td><td class="source">${escapeHtml(item.source.line)}</td></tr>`,
		)
		.join("");
	return `<table><thead><tr><th>Kind</th><th>Scenario contract</th><th>Line</th></tr></thead><tbody>${rows}</tbody></table>`;
}

function renderToolSummary(tools) {
	if (!tools?.length) return "";
	const label = tools.length === 1 ? "Tool" : `${tools.length} tools`;
	const detail = tools.map((tool) => tool.description).join("\n");
	return `<span class="tool-summary" title="${escapeHtml(detail)}" aria-label="${escapeHtml(`${label}: ${detail}`)}">${label}</span>`;
}

function renderContract(item, trailing = "") {
	let description = item.description;
	for (const index of item.presentation?.omitArguments ?? [])
		description = description.replaceAll(item.arguments?.[index] ?? "", "");
	const rendered = [];
	let badgesRendered = false;
	for (const part of structuredFragments(description)) {
		if (part.kind === "text") {
			let cleaned = part.value
				.replace(" with layout .", ".")
				.replace(" with  .", ".");
			if (item.presentation?.omitStructuredArguments)
				cleaned = cleaned.replace(/\s+with layout\s*$/u, "");
			if (!cleaned.trim() || /^[,.\s]+$/u.test(cleaned)) continue;
			rendered.push(
				`<div class="contract-main">${highlightTokens(cleaned)}${badgesRendered ? "" : `${renderItemBadges(item)}${trailing}`}</div>`,
			);
			badgesRendered = true;
			continue;
		}
		const filtered = filterConfiguration(item, part.value);
		if (filtered)
			rendered.push(
				`<pre class="config-code">${highlightConfiguration(prettyConfiguration(filtered))}</pre>`,
			);
	}
	if (!badgesRendered && item.presentation?.badges?.length)
		rendered.unshift(
			`<div class="contract-main">${renderItemBadges(item)}${trailing}</div>`,
		);
	else if (!badgesRendered && trailing)
		rendered.unshift(`<div class="contract-main">${trailing}</div>`);
	return rendered.join("");
}

function renderItemBadges(item) {
	const badges = item.presentation?.badges ?? [];
	if (!badges.length) return "";
	return `<span class="inline-badges">${badges
		.map(
			(badge) =>
				`<span class="item-badge">${escapeHtml(badge.kind)}: ${escapeHtml(badge.label)}</span>`,
		)
		.join("")}</span>`;
}

function renderSourceCode(item) {
	const source = item.sourceCode ?? item.expression;
	if (!source) return "";
	return `<pre class="source-code"><code>${escapeHtml(source)}</code></pre>`;
}

function renderDiagnostics(scenario) {
	if (!scenario.diagnostics.length) return '<p class="empty">None.</p>';
	const rows = scenario.diagnostics
		.map((item) => {
			const expression = item.expression
				? `<span class="unresolved-token">&lt;unresolved: ${escapeHtml(item.expression)}&gt;</span> `
				: "";
			const location = diagnosticLocation(scenario, item);
			const where = location
				? `<a class="diagnostic-where" href="#${rowAnchor(scenario, location.item)}">${location.label}</a>`
				: "Source";
			return `<tr><td><span class="diagnostic-code">${escapeHtml(item.code)}</span></td><td>${where}</td><td class="diagnostic">${expression}${escapeHtml(item.message)}</td><td class="source">${escapeHtml(item.source.line)}</td></tr>`;
		})
		.join("");
	return `<table><thead><tr><th>Diagnostic</th><th>Where</th><th>Detail</th><th>Line</th></tr></thead><tbody>${rows}</tbody></table>`;
}

function diagnosticLocation(scenario, diagnostic) {
	for (const [label, items] of [
		["Precondition", scenario.preconditions ?? []],
		["Step", scenario.steps],
		["Expected outcome", scenario.expectedOutcomes],
	]) {
		const item = items.find(
			(candidate) =>
				(candidate.source.file === diagnostic.source.file &&
					candidate.source.line === diagnostic.source.line) ||
				(candidate.call === diagnostic.relatedCall &&
					candidate.source.file === diagnostic.relatedSource?.file &&
					candidate.source.line === diagnostic.relatedSource?.line),
		);
		if (item) return { label, item };
	}
	return undefined;
}

function rowAnchor(scenario, item) {
	return `${scenarioAnchor(scenario)}-line-${item.source.line}-item-${item.order ?? 0}`;
}

function scenarioAnchor(scenario) {
	return `test-${slug(scenario.source.file)}-${scenario.source.line}-${slug(scenario.id)}`;
}

function slug(value) {
	return value
		.toLowerCase()
		.replace(/[^a-z0-9]+/gu, "-")
		.replace(/^-|-$/gu, "");
}

function structuredFragments(value) {
	const parts = [];
	let text = "";
	let structured = "";
	let depth = 0;
	let quote = "";
	let escaped = false;
	for (let index = 0; index < value.length; index += 1) {
		if (!depth && value.startsWith("<unresolved:", index)) {
			const end = unresolvedEnd(value, index);
			const stop = end < 0 ? value.length : end + 1;
			text += value.slice(index, stop);
			index = stop - 1;
			continue;
		}
		const char = value[index];
		if (depth) {
			structured += char;
			if (quote) {
				if (!escaped && char === quote) quote = "";
				escaped = !escaped && char === "\\";
				continue;
			}
			if (char === '"' || char === "'") quote = char;
			else if (char === "{" || char === "[") depth += 1;
			else if (char === "}" || char === "]") {
				depth -= 1;
				if (!depth) {
					parts.push({ kind: "config", value: structured });
					structured = "";
				}
			}
			continue;
		}
		if (char === "{" || char === "[") {
			if (text) parts.push({ kind: "text", value: text });
			text = "";
			depth = 1;
			structured = char;
		} else text += char;
	}
	if (structured) text += structured;
	if (text) parts.push({ kind: "text", value: text });
	return parts;
}

function filterConfiguration(item, value) {
	if (item.presentation?.omitStructuredArguments) return "";
	const omitted = new Set(item.presentation?.omitConfigurationFields ?? []);
	if (!omitted.size || !value.startsWith("{")) return value;
	const kept = splitTopLevel(value.slice(1, -1)).filter(
		(part) => !omitted.has(propertyKey(part)),
	);
	return kept.length ? `{${kept.join(",")}}` : "";
}

function splitTopLevel(value) {
	const parts = [];
	let start = 0;
	let depth = 0;
	let quote = "";
	let escaped = false;
	for (let index = 0; index < value.length; index += 1) {
		const char = value[index];
		if (quote) {
			if (!escaped && char === quote) quote = "";
			escaped = !escaped && char === "\\";
			continue;
		}
		if (char === '"' || char === "'") quote = char;
		else if (char === "{" || char === "[") depth += 1;
		else if (char === "}" || char === "]") depth -= 1;
		else if (char === "," && !depth) {
			parts.push(value.slice(start, index));
			start = index + 1;
		}
	}
	parts.push(value.slice(start));
	return parts;
}

function propertyKey(value) {
	let depth = 0;
	let quote = "";
	let escaped = false;
	for (let index = 0; index < value.length; index += 1) {
		const char = value[index];
		if (quote) {
			if (!escaped && char === quote) quote = "";
			escaped = !escaped && char === "\\";
			continue;
		}
		if (char === '"' || char === "'") quote = char;
		else if (char === "{" || char === "[") depth += 1;
		else if (char === "}" || char === "]") depth -= 1;
		else if (char === ":" && !depth)
			return value
				.slice(0, index)
				.trim()
				.replace(/^["']|["']$/gu, "");
	}
	return "";
}

function prettyConfiguration(value) {
	let result = "";
	let indent = 0;
	let quote = "";
	let escaped = false;
	const pad = () => {
		result += "  ".repeat(indent);
	};
	for (let index = 0; index < value.length; index += 1) {
		if (
			value.startsWith("<unresolved:", index) ||
			value.startsWith("<generated:", index) ||
			value.startsWith("<observed:", index)
		) {
			const end = value.startsWith("<unresolved:", index)
				? unresolvedEnd(value, index)
				: value.indexOf(">", index);
			const stop = end < 0 ? value.length : end + 1;
			result += value.slice(index, stop);
			index = stop - 1;
			continue;
		}
		const char = value[index];
		if (quote) {
			result += char;
			if (!escaped && char === quote) quote = "";
			escaped = !escaped && char === "\\";
			continue;
		}
		if (char === '"' || char === "'") {
			quote = char;
			result += char;
		} else if (char === "{" || char === "[") {
			result += `${char}\n`;
			indent += 1;
			pad();
		} else if (char === "}" || char === "]") {
			result = `${result.trimEnd()}\n`;
			indent -= 1;
			pad();
			result += char;
		} else if (char === ",") {
			result += ",\n";
			pad();
		} else if (char === ":") result += ": ";
		else result += char;
	}
	return result;
}

function highlightConfiguration(value) {
	let result = "";
	let index = 0;
	while (index < value.length) {
		if (value.startsWith("<observed:", index)) {
			const end = value.indexOf(">", index);
			const stop = end < 0 ? value.length : end + 1;
			result += `<span class="observed-token">${escapeHtml(value.slice(index, stop))}</span>`;
			index = stop;
			continue;
		}
		if (value.startsWith("<generated:", index)) {
			const end = value.indexOf(">", index);
			const stop = end < 0 ? value.length : end + 1;
			result += `<span class="generated-token">${escapeHtml(value.slice(index, stop))}</span>`;
			index = stop;
			continue;
		}
		if (value.startsWith("<unresolved:", index)) {
			const end = unresolvedEnd(value, index);
			const stop = end < 0 ? value.length : end + 1;
			result += `<span class="unresolved-token">${escapeHtml(value.slice(index, stop))}</span>`;
			index = stop;
			continue;
		}
		const char = value[index];
		if (char === '"' || char === "'") {
			let stop = index + 1;
			let escaped = false;
			while (stop < value.length) {
				const next = value[stop];
				stop += 1;
				if (!escaped && next === char) break;
				escaped = !escaped && next === "\\";
			}
			result += `<span class="json-string">${escapeHtml(value.slice(index, stop))}</span>`;
			index = stop;
			continue;
		}
		if (
			(char >= "0" && char <= "9") ||
			(char === "-" && value[index + 1] >= "0" && value[index + 1] <= "9")
		) {
			let stop = index + 1;
			while (stop < value.length && "0123456789.".includes(value[stop]))
				stop += 1;
			result += `<span class="json-number">${escapeHtml(value.slice(index, stop))}</span>`;
			index = stop;
			continue;
		}
		if (isWordStart(char)) {
			let stop = index + 1;
			while (stop < value.length && isWord(value[stop])) stop += 1;
			const word = value.slice(index, stop);
			let look = stop;
			while (value[look] === " ") look += 1;
			const kind =
				value[look] === ":"
					? "json-key"
					: ["true", "false", "null"].includes(word)
						? "json-literal"
						: word.includes(".") && word[0] >= "A" && word[0] <= "Z"
							? "json-symbol"
							: "";
			result += kind
				? `<span class="${kind}">${escapeHtml(word)}</span>`
				: escapeHtml(word);
			index = stop;
			continue;
		}
		result += escapeHtml(char);
		index += 1;
	}
	return result;
}

function highlightTokens(value) {
	let result = "";
	let cursor = 0;
	while (true) {
		const unresolved = value.indexOf("<unresolved:", cursor);
		const generated = value.indexOf("<generated:", cursor);
		const observed = value.indexOf("<observed:", cursor);
		const starts = [unresolved, generated, observed].filter(
			(index) => index >= 0,
		);
		const start = starts.length ? Math.min(...starts) : -1;
		if (start < 0) return result + escapeHtml(value.slice(cursor));
		const isUnresolved = start === unresolved;
		const isObserved = start === observed;
		const end = isUnresolved
			? unresolvedEnd(value, start)
			: value.indexOf(">", start);
		if (end < 0) return result + escapeHtml(value.slice(cursor));
		const tokenClass = isUnresolved
			? "unresolved-token"
			: isObserved
				? "observed-token"
				: "generated-token";
		result += `${escapeHtml(value.slice(cursor, start))}<span class="${tokenClass}">${escapeHtml(value.slice(start, end + 1))}</span>`;
		cursor = end + 1;
	}
}

function unresolvedEnd(value, start) {
	let cursor = start + "<unresolved:".length;
	while (cursor < value.length) {
		const candidate = value.indexOf(">", cursor);
		if (candidate < 0) return -1;
		const next = value.slice(candidate + 1).trimStart()[0];
		if (!next || ",.}]".includes(next)) return candidate;
		cursor = candidate + 1;
	}
	return -1;
}

function isWordStart(char) {
	return (
		char === "_" ||
		char === "$" ||
		(char >= "A" && char <= "Z") ||
		(char >= "a" && char <= "z")
	);
}

function isWord(char) {
	return isWordStart(char) || (char >= "0" && char <= "9") || char === ".";
}

function escapeHtml(value) {
	return String(value).replace(
		/[&<>"']/gu,
		(char) =>
			({
				"&": "&amp;",
				"<": "&lt;",
				">": "&gt;",
				'"': "&quot;",
				"'": "&#39;",
			})[char],
	);
}
