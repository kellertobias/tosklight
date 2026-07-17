#!/usr/bin/env python3
"""Build an offline, deployable single-page HTML manual from docs/help."""

from __future__ import annotations

import argparse
import html
import re
import shutil
import zipfile
from collections import OrderedDict
from pathlib import Path
from urllib.parse import unquote

from markdown_it import MarkdownIt
from PIL import Image as PILImage

from build_manual import HELP, ROOT, SourcePage, slug, source_pages, validate_sources, workspace_version
from artifact_paths import artifact_path

DEFAULT_MANUAL_ROOT = artifact_path("LIGHT_MANUAL_ROOT", "MANUAL_ROOT")
DEFAULT_SITE = DEFAULT_MANUAL_ROOT / "html" / "tosklight-manual"
DEFAULT_ARCHIVE = DEFAULT_MANUAL_ROOT / "html" / "tosklight-manual-html.zip"


def keycap_category(label: str) -> str:
    if re.fullmatch(r"(?:\d|0-9|\.)", label):
        return "number"
    if label == "CLR":
        return "clear"
    if label == "REC":
        return "record"
    return "command"


def decorate_inline_html(rendered: str) -> str:
    def keyboard(match: re.Match[str]) -> str:
        label = html.escape(match.group(1).strip())
        return f'<span class="key keyboard-key"><kbd>{label}</kbd><small>keyboard</small></span>'

    def desk(match: re.Match[str]) -> str:
        label = match.group(1).strip()
        state = ""
        if len(label) > 1 and label[-1:] in {"+", "*"}:
            state = " held" if label[-1] == "+" else " optional"
            label = label[:-1]
        category = keycap_category(label)
        suffix = '<small>hold</small>' if state == " held" else ('<small>optional</small>' if state else "")
        return f'<span class="key desk-key desk-key-{category}{state}"><kbd>{html.escape(label)}</kbd>{suffix}</span>'

    rendered = re.sub(r"\[KBD:([^\]\n]+)\]", keyboard, rendered)
    rendered = re.sub(r"\[\s*([+\-−^.]|[A-Z0-9.][A-Z0-9._ ←-]*[+*]?)\s*\]", desk, rendered)
    rendered = re.sub(r"&lt;([A-Za-z][A-Za-z0-9+*._ -]*)&gt;", lambda m: f'<span class="placeholder">&lt;{m.group(1)}&gt;</span>', rendered)
    return rendered.replace("&lt;br&gt;", "<br>")


def image_renderer(tokens, index, _options, _env) -> str:
    token = tokens[index]
    src = token.attrGet("src") or ""
    width = token.attrGet("width") or ""
    height = token.attrGet("height") or ""
    dimensions = f' width="{width}" height="{height}"' if width and height else ""
    alt = html.escape(token.content)
    return (
        '<span class="manual-figure">'
        f'<img src="{html.escape(src, quote=True)}" alt="{alt}" loading="lazy" decoding="async"{dimensions}>'
        f'<span class="manual-caption">{alt}</span>'
        "</span>"
    )


def rewrite_tokens(
    page: SourcePage,
    tokens,
    page_bookmarks: dict[str, str],
    site: Path,
    copied_images: set[str],
) -> None:
    heading_counts: dict[str, int] = {}
    for index, token in enumerate(tokens):
        if token.type == "heading_open" and index + 1 < len(tokens):
            title = tokens[index + 1].content
            base = page.bookmark if token.tag == "h1" else f"{page.bookmark}-{slug(title)}"
            heading_counts[base] = heading_counts.get(base, 0) + 1
            identifier = base if heading_counts[base] == 1 else f"{base}-{heading_counts[base]}"
            token.attrSet("id", identifier)
        if token.type != "inline" or not token.children:
            continue
        for child in token.children:
            if child.type == "link_open":
                target = child.attrGet("href") or ""
                if target.startswith(("http://", "https://", "mailto:")):
                    child.attrSet("target", "_blank")
                    child.attrSet("rel", "noreferrer")
                    continue
                clean, separator, fragment = target.partition("#")
                if not clean:
                    destination = page.bookmark
                else:
                    resolved = (page.path.parent / unquote(clean)).resolve()
                    try:
                        relative = resolved.relative_to(HELP).as_posix()
                    except ValueError:
                        continue
                    destination = page_bookmarks.get(relative, "")
                if destination:
                    child.attrSet("href", f"#{destination}-{slug(fragment)}" if separator and fragment else f"#{destination}")
            elif child.type == "image":
                source = (page.path.parent / unquote(child.attrGet("src") or "")).resolve()
                relative = source.relative_to(HELP).as_posix()
                destination = site / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                if relative not in copied_images:
                    shutil.copy2(source, destination)
                    copied_images.add(relative)
                child.attrSet("src", relative)
                try:
                    with PILImage.open(source) as image:
                        child.attrSet("width", str(image.width))
                        child.attrSet("height", str(image.height))
                except OSError:
                    pass


def render_page(md: MarkdownIt, page: SourcePage, bookmarks: dict[str, str], site: Path, copied_images: set[str]) -> str:
    tokens = md.parse(page.markdown)
    rewrite_tokens(page, tokens, bookmarks, site, copied_images)
    rendered = decorate_inline_html(md.renderer.render(tokens, md.options, {}))
    if "Dynamics is a future feature." in page.markdown:
        rendered = re.sub(
            r'(<h2 id="[^"]+-dynamics">Dynamics</h2>\s*<blockquote>\s*<p><strong>Dynamics is a future feature\.</strong></p>\s*</blockquote>)',
            r'<section class="future-feature">\1</section>',
            rendered,
        )
    return f'<article class="manual-page" data-page="{page.bookmark}" aria-labelledby="{page.bookmark}">{rendered}</article>'


def navigation(pages: list[SourcePage]) -> str:
    top_level: list[SourcePage] = []
    groups: OrderedDict[str, list[SourcePage]] = OrderedDict()
    for page in pages:
        parts = Path(page.relative).parts
        if len(parts) == 1:
            top_level.append(page)
        else:
            groups.setdefault(parts[0], []).append(page)
    items = [f'<a class="nav-page top-level" href="#{page.bookmark}" data-page-link="{page.bookmark}">{html.escape(page.title)}</a>' for page in top_level]
    for pages_in_group in groups.values():
        chapter = next((page for page in pages_in_group if Path(page.relative).name.lower() in {"index.md", "index.markdown"}), pages_in_group[0])
        children = [page for page in pages_in_group if page is not chapter]
        child_html = "".join(f'<a class="nav-page" href="#{page.bookmark}" data-page-link="{page.bookmark}">{html.escape(page.title)}</a>' for page in children)
        items.append(
            '<section class="nav-chapter">'
            f'<a class="nav-page chapter" href="#{chapter.bookmark}" data-page-link="{chapter.bookmark}">{html.escape(chapter.title)}</a>'
            f'<div class="nav-children">{child_html}</div>'
            "</section>"
        )
    return "".join(items)


CSS = r"""
html,body{overflow-x:hidden}
:root{color-scheme:light;--navy:#071621;--ink:#17202a;--muted:#64748b;--teal:#0f8f82;--cyan:#5ee7f0;--paper:#f7f6f1;--line:#d8dee5;--panel:#fff;--shadow:0 16px 48px #07162118;font-family:Inter,ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}*{box-sizing:border-box}html{scroll-behavior:smooth;scroll-padding-top:5rem}body{margin:0;background:var(--paper);color:var(--ink);line-height:1.55}.skip-link{position:fixed;left:1rem;top:-4rem;z-index:100;padding:.7rem 1rem;background:#fff;color:#000}.skip-link:focus{top:1rem}.mobile-nav{display:none}.sidebar{position:fixed;inset:0 auto 0 0;width:20rem;overflow:auto;background:var(--navy);color:#fff;padding:1.4rem 1rem 3rem;z-index:20}.brand{display:flex;gap:.8rem;align-items:center;margin-bottom:1.2rem}.brand img{width:3rem;height:3rem;border-radius:.8rem}.brand strong{font-size:1.1rem}.brand span{display:block;color:#9fb3c1;font-size:.75rem}.search{position:sticky;top:0;background:var(--navy);padding:.35rem 0 1rem;z-index:2}.search input{width:100%;padding:.72rem .8rem;border:1px solid #36515f;border-radius:.55rem;background:#102632;color:#fff;font:inherit}.search small{display:block;min-height:1.2rem;margin:.35rem .2rem 0;color:#9fb3c1}.nav-page{display:block;padding:.42rem .65rem;border-left:3px solid transparent;border-radius:.25rem;color:#dbe7ec;text-decoration:none;font-size:.88rem}.nav-page:hover,.nav-page:focus{background:#15313d;color:#fff}.nav-page.active{border-color:var(--cyan);background:#123642;color:var(--cyan)}.nav-page.chapter,.nav-page.top-level{margin-top:.42rem;color:var(--cyan);font-weight:800;text-decoration:underline;text-underline-offset:.22rem}.nav-children{margin-left:.7rem;border-left:1px solid #294652}.manual{margin-left:20rem}.hero{min-height:48vh;padding:8rem max(7vw,3rem) 5rem;background:var(--navy);color:#fff;overflow:hidden;position:relative}.hero:after{content:"";position:absolute;width:24rem;height:24rem;border-radius:50%;right:-7rem;top:-10rem;background:#12b8a6}.hero img{width:6rem;height:6rem;border-radius:1.4rem;box-shadow:0 12px 36px #0008}.hero h1{margin:1.6rem 0 .25rem;font-size:clamp(3rem,7vw,5.6rem);line-height:1}.hero p{margin:0;color:var(--cyan);font-size:1.25rem}.hero .kicker{margin-top:4rem;color:#fff;font-size:.75rem;font-weight:800;letter-spacing:.12em}.manual-page{max-width:72rem;margin:2.8rem auto;padding:3.2rem clamp(1.3rem,5vw,4.5rem);background:var(--panel);box-shadow:var(--shadow);border-radius:.45rem}.manual-page[hidden],.nav-page[hidden],.nav-chapter[hidden]{display:none}.manual-page h1{margin:-3.2rem clamp(-4.5rem,-5vw,-1.3rem) 2rem;padding:1.8rem clamp(1.3rem,5vw,4.5rem);background:var(--navy);color:#fff;text-decoration:underline;text-decoration-color:var(--cyan);text-underline-offset:.35rem;font-size:clamp(2rem,4vw,3rem)}.manual-page h2{margin-top:2.2rem;padding-bottom:.35rem;border-bottom:2px solid #d6eeeb;color:#0f766e}.manual-page h3{margin-top:1.8rem;color:var(--navy)}.manual-page a{color:#087f8c}.manual-page table{display:block;width:100%;overflow-x:auto;border-collapse:collapse;margin:1rem 0}.manual-page th,.manual-page td{min-width:9rem;padding:.55rem .65rem;border:1px solid var(--line);text-align:left;vertical-align:top}.manual-page th{background:var(--navy);color:#fff}.manual-page tr:nth-child(even) td{background:#f1f5f4}.manual-page pre{overflow:auto;padding:1rem;border-radius:.45rem;background:var(--navy);color:#d9f7f4}.manual-page code{color:#0f766e}.manual-page pre code{color:inherit}.manual-figure{display:block;margin:1.8rem auto;text-align:center}.manual-figure img{display:block;max-width:100%;height:auto;margin:auto;border:1px solid var(--line);border-radius:.5rem}.manual-caption{display:block;margin-top:.4rem;color:var(--muted);font-size:.78rem;font-style:italic}.manual-page blockquote{margin:1.5rem 0;padding:1rem 1.2rem;border-left:.28rem solid var(--teal);background:#eaf7f5}.future-feature{min-height:72vh;display:grid;align-content:center}.future-feature h2{border:0;color:#000;text-align:center}.future-feature blockquote{padding:3rem;border:1px solid #000;background:#fff;color:#000;text-align:center;font-size:1.7rem;font-weight:800}.key{display:inline-flex;align-items:center;gap:.28rem;white-space:nowrap;vertical-align:middle}.key kbd{display:inline-flex;min-width:2.1rem;min-height:1.55rem;align-items:center;justify-content:center;padding:.12rem .48rem;border:1px solid #3a4652;border-bottom-width:3px;border-radius:.42rem;background:linear-gradient(#252c33,#171c22);color:#ffb30f;box-shadow:0 1px 2px #0006;font:800 .78em/1.2 inherit}.desk-key-number kbd{color:#edf3f6}.desk-key-clear kbd{border-color:#d6a600;border-bottom-color:#806000;background:linear-gradient(#493b05,#261d08);color:#f0c52f}.desk-key-record kbd{border-color:#ff6872;border-bottom-color:#70181f;background:linear-gradient(#421116,#21090c);color:#ff6872}.keyboard-key kbd{border-color:#cbd5e1;border-bottom-color:#7d8b94;background:linear-gradient(#fff,#e5e7eb);color:#17202a;font-family:ui-monospace,SFMono-Regular,Menlo,monospace}.key small{color:var(--muted);font-size:.62em;font-weight:800;text-transform:uppercase}.placeholder{padding:.05rem .25rem;border-radius:.25rem;background:#e3f3f1;color:#0f766e;font-family:ui-monospace,SFMono-Regular,Menlo,monospace}.no-results{max-width:50rem;margin:4rem auto;padding:2rem;text-align:center}[hidden]{display:none!important}.site-footer{padding:3rem;text-align:center;color:var(--muted)}
.key{margin-inline:.12rem}
@media(max-width:900px){.mobile-nav{display:block;position:fixed;right:1rem;top:1rem;z-index:40;padding:.65rem .8rem;border:0;border-radius:.45rem;background:var(--navy);color:#fff;box-shadow:0 5px 20px #0004}.sidebar{transform:translateX(-105%);transition:transform .18s ease;box-shadow:0 0 40px #0007}.sidebar.open{transform:none}.manual{margin-left:0}.hero{padding:7rem 1.4rem 4rem}.manual-page{margin:1rem;border-radius:0;padding:2rem 1.2rem}.manual-page h1{margin:-2rem -1.2rem 1.5rem;padding:1.5rem 1.2rem}}
@media print{.sidebar,.mobile-nav,.search,.skip-link{display:none!important}.manual{margin:0}.hero{min-height:0;break-after:page}.manual-page{max-width:none;margin:0;padding:1.4cm;box-shadow:none;break-before:page}.manual-page h1{margin:-1.4cm -1.4cm 1cm;padding:1cm 1.4cm}.manual-figure img{max-height:17cm}.site-footer{display:none}}
"""

JS = r"""
(()=>{const sidebar=document.querySelector('.sidebar'),toggle=document.querySelector('.mobile-nav'),search=document.querySelector('#manual-search'),status=document.querySelector('#search-status'),pages=[...document.querySelectorAll('.manual-page')],links=[...document.querySelectorAll('[data-page-link]')],groups=[...document.querySelectorAll('.nav-chapter')],noResults=document.querySelector('.no-results');function closeNav(){sidebar.classList.remove('open');toggle.setAttribute('aria-expanded','false')}toggle.addEventListener('click',()=>{const open=sidebar.classList.toggle('open');toggle.setAttribute('aria-expanded',String(open))});links.forEach(link=>link.addEventListener('click',closeNav));function activate(){const hash=location.hash.slice(1);let target=document.getElementById(hash);if(!target)return;const page=target.closest('.manual-page');if(!page)return;links.forEach(link=>{const active=link.dataset.pageLink===page.dataset.page;link.classList.toggle('active',active);active?link.setAttribute('aria-current','page'):link.removeAttribute('aria-current')})}addEventListener('hashchange',activate);activate();const observer=new IntersectionObserver(entries=>{if(location.hash||search.value)return;const visible=entries.filter(x=>x.isIntersecting).sort((a,b)=>b.intersectionRatio-a.intersectionRatio)[0];if(visible){history.replaceState(null,'','#'+visible.target.dataset.page);activate()}},{rootMargin:'-20% 0px -65%',threshold:[.05,.25,.5]});pages.forEach(page=>observer.observe(page));search.addEventListener('input',()=>{const query=search.value.trim().toLocaleLowerCase();let count=0;pages.forEach(page=>{const match=!query||page.textContent.toLocaleLowerCase().includes(query);page.hidden=!match;if(match)count++});links.forEach(link=>{const page=document.querySelector(`[data-page="${CSS.escape(link.dataset.pageLink)}"]`);link.hidden=Boolean(query)&&page.hidden});groups.forEach(group=>group.hidden=Boolean(query)&&![...group.querySelectorAll('.nav-page')].some(link=>!link.hidden));noResults.hidden=count!==0;status.textContent=query?`${count} matching page${count===1?'':'s'}`:`${pages.length} pages`;if(query&&count){pages.find(page=>!page.hidden)?.scrollIntoView({block:'start'})}});status.textContent=`${pages.length} pages`;document.addEventListener('keydown',event=>{if(event.key==='/'&&!/input|textarea/i.test(document.activeElement.tagName)){event.preventDefault();search.focus()}if(event.key==='Escape'){search.value='';search.dispatchEvent(new Event('input'));closeNav()}})})();
"""


def document_html(pages: list[SourcePage], articles: str, nav: str, version: str, logo_path: str) -> str:
    return f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="description" content="ToskLight operator manual for software version {html.escape(version)}">
<title>ToskLight Operator Manual v{html.escape(version)}</title><link rel="icon" href="assets/brand/icon.png"><style>{CSS}</style></head>
<body><a class="skip-link" href="#manual-content">Skip to manual</a><button class="mobile-nav" type="button" aria-controls="manual-navigation" aria-expanded="false">Contents</button>
<aside class="sidebar" id="manual-navigation"><div class="brand"><img src="{logo_path}" alt="ToskLight"><div><strong>ToskLight Manual</strong><span>Software v{html.escape(version)}</span></div></div>
<div class="search"><label for="manual-search">Search the manual</label><input id="manual-search" type="search" placeholder="Fixture sheet, OSC, Preload..." autocomplete="off"><small id="search-status" aria-live="polite"></small></div><nav aria-label="Manual contents">{nav}</nav></aside>
<main class="manual" id="manual-content"><header class="hero"><img src="{logo_path}" alt="ToskLight application logo"><h1>ToskLight</h1><p>Operator manual / software v{html.escape(version)}</p><div class="kicker">DESK SETUP / SHOW SETUP / PROGRAMMING / RUNNING A SHOW</div></header>{articles}<p class="no-results" hidden>No manual page matches this search.</p><footer class="site-footer">ToskLight v{html.escape(version)} · Operator Manual</footer></main><script>{JS}</script></body></html>"""


def deterministic_zip(site: Path, archive: Path) -> None:
    archive.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as output:
        for source in sorted(path for path in site.rglob("*") if path.is_file()):
            relative = source.relative_to(site).as_posix()
            info = zipfile.ZipInfo(relative, date_time=(2020, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            output.writestr(info, source.read_bytes(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)


def build(site: Path, archive: Path) -> None:
    pages = source_pages()
    validate_sources(pages)
    if site.exists():
        shutil.rmtree(site)
    site.mkdir(parents=True)
    logo = ROOT / "apps" / "control-ui" / "src-tauri" / "icons" / "icon.png"
    logo_destination = site / "assets" / "brand" / "icon.png"
    logo_destination.parent.mkdir(parents=True)
    shutil.copy2(logo, logo_destination)
    bookmarks = {page.relative: page.bookmark for page in pages}
    md = MarkdownIt("commonmark", {"html": False}).enable("table")
    md.renderer.rules["image"] = image_renderer
    copied_images: set[str] = set()
    articles = "".join(render_page(md, page, bookmarks, site, copied_images) for page in pages)
    version = workspace_version()
    (site / "index.html").write_text(document_html(pages, articles, navigation(pages), version, "assets/brand/icon.png"), encoding="utf-8")
    deterministic_zip(site, archive)
    print(f"Built {site / 'index.html'} and {archive} from {len(pages)} Markdown pages and {len(copied_images)} referenced images")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--site", type=Path, default=DEFAULT_SITE)
    parser.add_argument("--archive", type=Path, default=DEFAULT_ARCHIVE)
    args = parser.parse_args()
    try:
        build(args.site.resolve(), args.archive.resolve())
    except Exception as error:
        print(f"HTML manual build failed: {error}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
