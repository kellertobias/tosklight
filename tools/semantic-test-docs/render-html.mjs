export function renderHtml(catalog) {
	const json = JSON.stringify(catalog)
		.replaceAll("<", "\\u003c")
		.replaceAll(">", "\\u003e")
		.replaceAll("&", "\\u0026");
	return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>ToskLight semantic test catalog</title>
<style>
:root{color-scheme:dark;--bg:#101419;--panel:#171d24;--line:#2b3541;--text:#edf3f8;--muted:#9caebe;--accent:#58c7ff;--warn:#ffbf69;--good:#66d39e}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:15px/1.5 system-ui,sans-serif}
header{position:sticky;top:0;z-index:2;padding:1.25rem max(1rem,calc((100% - 1100px)/2));background:#101419f2;border-bottom:1px solid var(--line);backdrop-filter:blur(12px)}
h1{margin:0 0 .65rem;font-size:1.45rem}.summary{color:var(--muted);margin:.5rem 0}
input{width:100%;padding:.8rem 1rem;border:1px solid var(--line);border-radius:.5rem;background:var(--panel);color:var(--text);font:inherit}
main{max-width:1100px;margin:0 auto;padding:1rem}.scenario{margin:1rem 0;padding:1rem 1.1rem;background:var(--panel);border:1px solid var(--line);border-radius:.65rem}
.scenario[hidden]{display:none}h2{font-size:1.08rem;margin:0}.id{color:var(--accent);font-family:ui-monospace,monospace}.meta,.empty{color:var(--muted)}
.chips{display:flex;flex-wrap:wrap;gap:.35rem;margin:.65rem 0}.chip{padding:.12rem .45rem;border-radius:1rem;background:#24303b;color:#cbd9e4;font-size:.8rem}
h3{font-size:.9rem;margin:1rem 0 .3rem;color:#c8d5df}ol,ul{margin:.25rem 0;padding-left:1.5rem}.diagnostic{color:var(--warn)}.passed{color:var(--good)}code{font-family:ui-monospace,monospace}
</style>
</head>
<body>
<header><h1>ToskLight semantic Playwright scenarios</h1><input id="search" type="search" placeholder="Search ID, title, steps, outcomes, surfaces, or status" aria-label="Search scenarios"><p class="summary" id="summary"></p></header>
<main id="catalog"></main>
<script id="catalog-data" type="application/json">${json}</script>
<script>
const data=JSON.parse(document.getElementById("catalog-data").textContent);
const host=document.getElementById("catalog"),search=document.getElementById("search"),summary=document.getElementById("summary");
const esc=value=>String(value).replace(/[&<>"']/g,char=>({"&":"&amp;","<":"&lt;",">":"&gt;","\\"":"&quot;","'":"&#39;"}[char]));
const list=(items,empty)=>items.length?"<ol>"+items.map(item=>"<li>"+esc(item.description)+" <span class=meta>("+esc(item.source.line)+")</span></li>").join("")+"</ol>":"<p class=empty>"+empty+"</p>";
for(const scenario of data.scenarios){const article=document.createElement("article");article.className="scenario";article.dataset.search=JSON.stringify(scenario).toLowerCase();const run=scenario.lastRun?"<span class='"+(scenario.lastRun.status==="passed"?"passed":"")+"'>last run: "+esc(scenario.lastRun.status)+"</span>":"last run: not merged";article.innerHTML="<h2><span class=id>"+esc(scenario.id)+"</span> — "+esc(scenario.title)+"</h2><p class=meta>"+esc(scenario.source.file)+":"+scenario.source.line+" · "+esc(scenario.migration.status)+" · "+run+"</p><div class=chips>"+scenario.testedSurfaces.map(x=>"<span class=chip>"+esc(x)+"</span>").join("")+"</div><h3>Steps</h3>"+list(scenario.steps,"No narrated actions.")+"<h3>Expected outcomes</h3>"+list(scenario.expectedOutcomes,"No explicit expected outcomes were statically visible.")+"<h3>Diagnostics</h3>"+(scenario.diagnostics.length?"<ul>"+scenario.diagnostics.map(x=>"<li class=diagnostic>"+esc(x.message)+" <span class=meta>("+x.source.line+")</span></li>").join("")+"</ul>":"<p class=empty>None.</p>");host.append(article)}
function filter(){const term=search.value.trim().toLowerCase();let visible=0;for(const node of host.children){node.hidden=term&&!node.dataset.search.includes(term);if(!node.hidden)visible++}summary.textContent=visible+" of "+data.scenarios.length+" scenarios · schema v"+data.schemaVersion}
search.addEventListener("input",filter);filter();
</script>
</body>
</html>
`;
}
