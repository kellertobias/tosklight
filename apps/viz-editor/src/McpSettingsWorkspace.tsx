import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

interface McpConfiguration {
	applicationPath: string;
	serverPath: string;
	codexCommand: string;
	jsonConfiguration: string;
}

export function McpSettingsWorkspace() {
	const [configuration, setConfiguration] = useState<McpConfiguration | null>(null);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		let current = true;
		invoke<McpConfiguration>("mcp_configuration")
			.then((value) => {
				if (current) setConfiguration(value);
			})
			.catch((reason) => {
				if (current) setError(String(reason));
			});
		return () => {
			current = false;
		};
	}, []);

	return (
		<section className="viz-mcp-settings">
			<div className="viz-mcp-settings-scroll">
				<header>
					<h1>MCP integration</h1>
					<p>
						Connect an AI tool to the rig open in this Architect. Architect must
						remain running; the bundled MCP bridge discovers its authenticated local
						endpoint automatically.
					</p>
				</header>
				{error ? (
					<p className="viz-mcp-settings-error" role="alert">
						{error}
					</p>
				) : configuration ? (
					<>
						<section>
							<h2>This installation</h2>
							<dl className="viz-mcp-paths">
								<div>
									<dt>Application</dt>
									<dd>{configuration.applicationPath}</dd>
								</div>
								<div>
									<dt>Bundled MCP server</dt>
									<dd>{configuration.serverPath}</dd>
								</div>
							</dl>
						</section>
						<section>
							<h2>Codex</h2>
							<p>
								Run this copy-ready command, then restart Codex so the tools become
								available:
							</p>
							<pre aria-label="Codex MCP configuration">
								<code>{configuration.codexCommand}</code>
							</pre>
						</section>
						<section>
							<h2>Other MCP clients</h2>
							<p>Use the same bundled stdio server and environment setting:</p>
							<pre aria-label="JSON MCP configuration">
								<code>{configuration.jsonConfiguration}</code>
							</pre>
						</section>
					</>
				) : (
					<p role="status">Locating this installation’s MCP bridge…</p>
				)}
				<aside>
					The bridge can search fixture profiles, add and remove fixtures, patch DMX,
					and edit fixture placement. It can only edit the show currently open here.
				</aside>
			</div>
		</section>
	);
}
