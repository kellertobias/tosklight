import fs from "node:fs";
import path from "node:path";
import { narrateCall } from "./narration-catalog.mjs";
import { resultFor } from "./playwright-results.mjs";
import { loadTypeScriptAst } from "./typescript-ast.mjs";

export const semanticWorldMarker = "// @bench-semantic-world";
export const schemaVersion = 1;

export async function compileSemanticTestCatalog({
	root,
	inventoryFile = path.join(
		root,
		"docs/engineering/test-bench-migration-inventory.md",
	),
	results = new Map(),
	sourceFiles,
}) {
	const ast = await loadTypeScriptAst();
	const files = (
		sourceFiles ?? discoverMarkedSpecs(path.join(root, "tests"))
	).map((file) => path.resolve(file)).sort();
	const inventory = readMigrationInventory(inventoryFile);
	const scenarios = [];
	const api = new ast.API({ cwd: root });
	let snapshot;
	try {
		snapshot = api.updateSnapshot({ openFiles: files });
		for (const file of files) {
			const project = snapshot.getDefaultProjectForFile(file);
			const sourceFile = project?.program.getSourceFile(file);
			if (!sourceFile)
				throw new Error(`TypeScript did not return an AST for ${relative(root, file)}`);
			const syntaxDiagnostics = project.program.getSyntacticDiagnostics(file);
			if (syntaxDiagnostics.length)
				throw new Error(
					`TypeScript could not parse ${relative(root, file)}: ${syntaxDiagnostics
						.map((diagnostic) => diagnostic.messageText)
						.join("; ")}`,
				);
			scenarios.push(
				...compileSourceFile({
					ast,
					root,
					sourceFile,
					inventory,
					results,
				}),
			);
		}
	} finally {
		snapshot?.dispose();
		api.close();
	}

	const occurrenceKeys = new Set();
	for (const scenario of scenarios) {
		const key = `${scenario.source.file}:${scenario.source.line}`;
		if (occurrenceKeys.has(key))
			throw new Error(`Semantic scenario occurrence was compiled twice: ${key}`);
		occurrenceKeys.add(key);
	}
	return {
		schemaVersion,
		generatedBy: "tools/semantic-test-docs/cli.mjs",
		sourceMarker: semanticWorldMarker,
		scenarioCount: scenarios.length,
		scenarios,
	};
}

export function discoverMarkedSpecs(testsRoot) {
	return fs
		.readdirSync(testsRoot, { withFileTypes: true })
		.filter((entry) => entry.isFile() && entry.name.endsWith(".spec.ts"))
		.map((entry) => path.join(testsRoot, entry.name))
		.filter((file) => fs.readFileSync(file, "utf8").includes(semanticWorldMarker));
}

export function readMigrationInventory(file) {
	const rows = new Map();
	for (const line of fs.readFileSync(file, "utf8").split(/\r?\n/u)) {
		if (!line.startsWith("| ") || line.startsWith("| ---")) continue;
		const cells = splitMarkdownRow(line);
		if (cells.length !== 8 || cells[0] === "Source") continue;
		const identity = /^([A-Z][A-Z0-9-]+)\s+@bench\s+@ui\s+›\s+(.+)$/u.exec(
			cells[1],
		);
		if (!identity) continue;
		const key = inventoryKey(cells[0], identity[1], identity[2]);
		const row = {
			source: cells[0],
			id: identity[1],
			title: identity[2],
			contract: cells[2],
			surfaces: cells[3].split(/\s+/u).filter(Boolean),
			helperFamily: cells[4],
			artifacts: cells[5],
			constraint: cells[6],
			status: cells[7],
		};
		const existing = rows.get(key) ?? [];
		existing.push(row);
		rows.set(key, existing);
	}
	return rows;
}

function compileSourceFile({ ast, root, sourceFile, inventory, results }) {
	const scenarioNames = importedScenarioNames(ast, sourceFile);
	if (!scenarioNames.size)
		throw new Error(
			`${relative(root, sourceFile.fileName)} is marked but does not import scenario() from the semantic bench`,
		);
	const helpers = localFunctions(ast, sourceFile);
	const scenarios = [];
	walk(sourceFile, (node) => {
		if (!ast.isCallExpression(node)) return;
		if (!ast.isIdentifier(node.expression)) return;
		if (!scenarioNames.has(node.expression.text)) return;
		scenarios.push(
			compileScenario({
				ast,
				root,
				sourceFile,
				call: node,
				helpers,
				inventory,
				results,
			}),
		);
		return false;
	});
	return scenarios;
}

function compileScenario({
	ast,
	root,
	sourceFile,
	call,
	helpers,
	inventory,
	results,
}) {
	const [idNode, titleNode, callback] = call.arguments;
	const source = location(root, sourceFile, call);
	if (!ast.isStringLiteral(idNode) || !ast.isStringLiteral(titleNode))
		throw new Error(
			`${source.file}:${source.line} scenario ID and title must be string literals for static documentation`,
		);
	if (!ast.isArrowFunction(callback) && !ast.isFunctionExpression(callback))
		throw new Error(
			`${source.file}:${source.line} scenario callback must be an inline function`,
		);
	const worldParameter = callback.parameters[0]?.name;
	if (!worldParameter || !ast.isIdentifier(worldParameter))
		throw new Error(`${source.file}:${source.line} scenario world parameter is not static`);

	const diagnostics = [];
	const state = {
		ast,
		sourceFile,
		root,
		helpers,
		worldNames: new Set([worldParameter.text]),
		aliases: new Map(),
		staticBindings: new Map(),
		helperStack: [],
		steps: [],
		expectedOutcomes: [],
		diagnostics,
		phase: "scenario",
	};
	visitExecution(callback.body, state);

	const matches =
		inventory.get(inventoryKey(source.file, idNode.text, titleNode.text)) ?? [];
	if (matches.length !== 1) {
		diagnostics.push({
			code: matches.length ? "ambiguous-migration-status" : "missing-migration-status",
			message: matches.length
				? `Migration inventory contains ${matches.length} matching rows; status is unresolved.`
				: "No exact source, ID, and title match exists in the generated migration inventory.",
			source,
		});
	}
	const migration = (matches.length === 1 ? matches[0] : undefined) ?? {
		status: "unresolved",
		contract: "unresolved",
		surfaces: [],
		helperFamily: "unresolved",
		artifacts: "unresolved",
		constraint: "unresolved",
	};
	const actualSurfaces = new Set(migration.surfaces);
	for (const item of [...state.steps, ...state.expectedOutcomes])
		for (const surface of item.surfaces) actualSurfaces.add(surface);
	return {
		id: idNode.text,
		title: titleNode.text,
		source,
		steps: state.steps,
		expectedOutcomes: state.expectedOutcomes,
		testedSurfaces: [...actualSurfaces].sort(),
		migration: {
			status: migration.status,
			contract: migration.contract,
			helperFamily: migration.helperFamily,
			artifacts: migration.artifacts,
			constraint: migration.constraint,
		},
		lastRun: resultFor(results, source.file, idNode.text, titleNode.text),
		diagnostics,
	};
}

function visitExecution(node, state) {
	const { ast } = state;
	if (ast.isFunctionDeclaration(node)) return;
	if (ast.isIfStatement(node)) {
		addControlFlowDiagnostic(node.expression, "if/else", state);
		visitExecution(node.thenStatement, {
			...state,
			phase: `${state.phase}:if-true`,
		});
		if (node.elseStatement)
			visitExecution(node.elseStatement, {
				...state,
				phase: `${state.phase}:if-false`,
			});
		return;
	}
	if (ast.isSwitchStatement(node)) {
		addControlFlowDiagnostic(node.expression, "switch", state);
		for (const clause of node.caseBlock.clauses)
			for (const statement of clause.statements)
				visitExecution(statement, {
					...state,
					phase: `${state.phase}:switch-branch`,
				});
		return;
	}
	if (ast.isConditionalExpression(node)) {
		addControlFlowDiagnostic(node.condition, "conditional expression", state);
		visitExecution(node.whenTrue, {
			...state,
			phase: `${state.phase}:condition-true`,
		});
		visitExecution(node.whenFalse, {
			...state,
			phase: `${state.phase}:condition-false`,
		});
		return;
	}
	if (
		ast.isBinaryExpression(node) &&
		[
			ast.SyntaxKind.AmpersandAmpersandToken,
			ast.SyntaxKind.BarBarToken,
			ast.SyntaxKind.QuestionQuestionToken,
		].includes(node.operatorToken.kind)
	) {
		addControlFlowDiagnostic(node, "short-circuit expression", state);
		visitExecution(node.left, state);
		visitExecution(node.right, {
			...state,
			phase: `${state.phase}:conditional-right`,
		});
		return;
	}
	if (ast.isTryStatement(node)) {
		visitExecution(node.tryBlock, state);
		if (node.catchClause) visitExecution(node.catchClause.block, state);
		if (node.finallyBlock)
			visitExecution(node.finallyBlock, { ...state, phase: "cleanup" });
		return;
	}
	if (
		ast.isForOfStatement(node) ||
		ast.isForInStatement(node) ||
		ast.isForStatement(node) ||
		ast.isWhileStatement(node) ||
		ast.isDoStatement(node)
	) {
		state.diagnostics.push({
			code: "static-control-flow",
			message: `Control flow ${compact(node.getText(state.sourceFile))} is narrated once; iteration values are not guessed.`,
			source: location(state.root, state.sourceFile, node),
		});
		if ("initializer" in node && node.initializer) registerLoopBinding(node, state);
		visitExecution(node.statement, state);
		return;
	}
	if (ast.isVariableDeclaration(node)) {
		if (ast.isIdentifier(node.name) && node.initializer) {
			state.staticBindings.set(node.name.text, node.initializer);
			const initializerCall = unwrapCall(node.initializer, ast);
			if (initializerCall) {
				const emitted = emitSemanticCall(initializerCall, state);
				if (emitted)
					state.aliases.set(node.name.text, {
						type: emitted.resultType,
						origin: emitted.path,
					});
				if (emitted) return;
			}
		}
		node.initializer?.forEachChild((child) => visitExecution(child, state));
		return;
	}
	if (ast.isCallExpression(node)) {
		if (emitSemanticCall(node, state)) return;
		if (
			ast.isIdentifier(node.expression) &&
			state.helpers.has(node.expression.text)
		) {
			expandHelper(node, state);
			return;
		}
	}
	node.forEachChild((child) => visitExecution(child, state));
}

function emitSemanticCall(node, state) {
	const chain = callChain(node, state);
	if (!chain) return undefined;
	const expressionDiagnostics = [];
	const args = chain.argumentNodes.map((argument) =>
		renderExpression(argument, state, expressionDiagnostics),
	);
	for (const diagnostic of expressionDiagnostics)
		state.diagnostics.push({
			code: diagnostic.code,
			message: diagnostic.message,
			source: location(state.root, state.sourceFile, diagnostic.node ?? node),
		});
	const narration = narrateCall({
		path: chain.path,
		root: chain.root,
		arguments: args,
	});
	const source = location(state.root, state.sourceFile, node);
	const raw = compact(node.getText(state.sourceFile));
	if (!narration) {
		state.diagnostics.push({
			code: "unknown-narration",
			message: `No narration catalog entry matches ${chain.path}; source is preserved as ${raw}.`,
			source,
		});
		const unresolved = {
			description: `Unresolved semantic call: ${raw}`,
			call: chain.path,
			expression: raw,
			surfaces: ["Unresolved"],
			phase: state.phase,
			source,
		};
		(chain.path.includes("expect")
			? state.expectedOutcomes
			: state.steps
		).push(unresolved);
		return { path: chain.path };
	}
	const item = {
		description: narration.description,
		call: chain.path,
		expression: raw,
		surfaces: narration.surfaces,
		phase: state.phase,
		source,
	};
	(narration.kind === "expected-outcome"
		? state.expectedOutcomes
		: state.steps
	).push(item);
	if (narration.expectedOutcome)
		state.expectedOutcomes.push({
			...item,
			description: narration.expectedOutcome,
			phase: `${state.phase}:catalog-outcome`,
		});
	return { path: chain.path, resultType: narration.resultType };
}

function callChain(call, state) {
	const parsed = parseCallee(call.expression, state);
	if (!parsed) return undefined;
	parsed.argumentNodes.push(...call.arguments);
	return {
		...parsed,
		path: parsed.segments.join("."),
	};
}

function parseCallee(expression, state) {
	const { ast } = state;
	if (ast.isIdentifier(expression)) {
		if (state.worldNames.has(expression.text))
			return { root: "t", segments: [], argumentNodes: [] };
		if (expression.text === "expect")
			return { root: "expect", segments: ["expect"], argumentNodes: [] };
		const alias = state.aliases.get(expression.text);
		if (alias?.type)
			return {
				root: "alias",
				segments: [alias.type],
				argumentNodes: [],
			};
		return undefined;
	}
	if (ast.isPropertyAccessExpression(expression)) {
		const receiver = parseCallee(expression.expression, state);
		if (!receiver) return undefined;
		receiver.segments.push(expression.name.text);
		return receiver;
	}
	if (ast.isCallExpression(expression)) {
		const receiver = parseCallee(expression.expression, state);
		if (!receiver) return undefined;
		receiver.argumentNodes.push(...expression.arguments);
		return receiver;
	}
	if (ast.isParenthesizedExpression(expression))
		return parseCallee(expression.expression, state);
	return undefined;
}

function expandHelper(call, state) {
	const name = call.expression.text;
	const helper = state.helpers.get(name);
	const source = location(state.root, state.sourceFile, call);
	if (state.helperStack.includes(name)) {
		state.diagnostics.push({
			code: "recursive-helper",
			message: `Local helper ${name} is recursive; nested execution was not expanded.`,
			source,
		});
		return;
	}
	const worldNames = new Set(state.worldNames);
	const staticBindings = new Map(state.staticBindings);
	let receivesWorld = false;
	for (const [index, parameter] of helper.parameters.entries()) {
		if (!state.ast.isIdentifier(parameter.name)) continue;
		const argument = call.arguments[index];
		if (argument && state.ast.isIdentifier(argument) && state.worldNames.has(argument.text)) {
			worldNames.add(parameter.name.text);
			receivesWorld = true;
		} else if (argument) {
			staticBindings.set(parameter.name.text, argument);
		}
	}
	if (!receivesWorld) {
		state.diagnostics.push({
			code: "unresolved-local-helper",
			message: `Local helper call ${compact(call.getText(state.sourceFile))} could not be tied to the scenario world.`,
			source,
		});
		return;
	}
	visitExecution(helper.body, {
		...state,
		worldNames,
		aliases: new Map(state.aliases),
		staticBindings,
		helperStack: [...state.helperStack, name],
		phase: `helper:${name}`,
	});
}

function renderExpression(node, state, diagnostics, seen = new Set()) {
	const { ast, sourceFile } = state;
	if (ast.isStringLiteral(node) || ast.isNoSubstitutionTemplateLiteral(node))
		return JSON.stringify(node.text);
	if (ast.isNumericLiteral(node)) return node.text;
	if (node.kind === ast.SyntaxKind.TrueKeyword) return "true";
	if (node.kind === ast.SyntaxKind.FalseKeyword) return "false";
	if (node.kind === ast.SyntaxKind.NullKeyword) return "null";
	if (ast.isIdentifier(node)) {
		const alias = state.aliases.get(node.text);
		if (alias) return `$${node.text} (result of ${alias.origin})`;
		const binding = state.staticBindings.get(node.text);
		if (binding && !seen.has(node.text)) {
			const nextSeen = new Set(seen).add(node.text);
			return `${node.text} = ${renderExpression(binding, state, diagnostics, nextSeen)}`;
		}
		diagnostics.push({
			code: "unresolved-expression",
			message: `Expression ${node.text} is not statically resolved.`,
			node,
		});
		return `<unresolved: ${node.text}>`;
	}
	if (ast.isPropertyAccessExpression(node)) {
		const raw = compact(node.getText(sourceFile));
		const root = leftmostIdentifier(node, ast);
		if (root && /^[A-Z]/u.test(root)) return raw;
		diagnostics.push({
			code: "unresolved-expression",
			message: `Property access ${raw} depends on a runtime value.`,
			node,
		});
		return `<unresolved: ${raw}>`;
	}
	if (ast.isArrayLiteralExpression(node))
		return `[${node.elements
			.map((element) => renderExpression(element, state, diagnostics, seen))
			.join(", ")}]`;
	if (ast.isObjectLiteralExpression(node)) {
		const values = node.properties.map((property) => {
			if (ast.isPropertyAssignment(property))
				return `${property.name.getText(sourceFile)}: ${renderExpression(property.initializer, state, diagnostics, seen)}`;
			if (ast.isShorthandPropertyAssignment(property))
				return `${property.name.text}: ${renderExpression(property.name, state, diagnostics, seen)}`;
			diagnostics.push({
				code: "unresolved-expression",
				message: `Object member ${compact(property.getText(sourceFile))} is not statically resolved.`,
				node: property,
			});
			return `<unresolved: ${compact(property.getText(sourceFile))}>`;
		});
		return `{ ${values.join(", ")} }`;
	}
	if (ast.isSpreadElement(node))
		return `...${renderExpression(node.expression, state, diagnostics, seen)}`;
	if (ast.isCallExpression(node)) {
		const raw = compact(node.getText(sourceFile));
		const root = raw.split(/[.(]/u)[0];
		if (
			[
				"currentPagePlayback",
				"dereferencedGroup",
				"dmxFixture",
				"explicitPagePlayback",
				"fixture",
				"fixtureRange",
				"group",
				"groupRange",
				"Object",
			].includes(root)
		)
			return raw;
		diagnostics.push({
			code: "unresolved-expression",
			message: `Call expression ${raw} is dynamic and was not executed.`,
			node,
		});
		return `<unresolved: ${raw}>`;
	}
	const raw = compact(node.getText(sourceFile));
	diagnostics.push({
		code: "unresolved-expression",
		message: `Expression ${raw} is not statically resolved.`,
		node,
	});
	return `<unresolved: ${raw}>`;
}

function importedScenarioNames(ast, sourceFile) {
	const names = new Set();
	for (const statement of sourceFile.statements) {
		if (!ast.isImportDeclaration(statement)) continue;
		if (!ast.isStringLiteral(statement.moduleSpecifier)) continue;
		if (
			!statement.moduleSpecifier.text.endsWith("/e2e/bench/scenario") &&
			!statement.moduleSpecifier.text.endsWith("/e2e/bench/core/scenario")
		) continue;
		const bindings = statement.importClause?.namedBindings;
		if (!bindings || !ast.isNamedImports(bindings)) continue;
		for (const element of bindings.elements)
			if ((element.propertyName ?? element.name).text === "scenario")
				names.add(element.name.text);
	}
	return names;
}

function localFunctions(ast, sourceFile) {
	const functions = new Map();
	for (const statement of sourceFile.statements)
		if (ast.isFunctionDeclaration(statement) && statement.name && statement.body)
			functions.set(statement.name.text, statement);
	return functions;
}

function registerLoopBinding(node, state) {
	const declarationList = node.initializer;
	if (!state.ast.isVariableDeclarationList(declarationList)) return;
	for (const declaration of declarationList.declarations)
		if (state.ast.isIdentifier(declaration.name))
			state.staticBindings.delete(declaration.name.text);
}

function addControlFlowDiagnostic(expression, kind, state) {
	state.diagnostics.push({
		code: "static-control-flow",
		message: `The ${kind} ${compact(expression.getText(state.sourceFile))} has conditional execution; branches are documented separately and no branch is assumed.`,
		source: location(state.root, state.sourceFile, expression),
	});
}

function leftmostIdentifier(node, ast) {
	let current = node;
	while (ast.isPropertyAccessExpression(current)) current = current.expression;
	return ast.isIdentifier(current) ? current.text : undefined;
}

function unwrapCall(node, ast) {
	let current = node;
	while (
		ast.isAwaitExpression(current) ||
		ast.isParenthesizedExpression(current) ||
		ast.isAsExpression(current) ||
		ast.isSatisfiesExpression(current)
	)
		current = current.expression;
	return ast.isCallExpression(current) ? current : undefined;
}

function walk(node, visitor) {
	const result = visitor(node);
	if (result === false) return;
	node.forEachChild((child) => walk(child, visitor));
}

function location(root, sourceFile, node) {
	const position = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
	return {
		file: relative(root, sourceFile.fileName),
		line: position.line + 1,
		column: position.character + 1,
	};
}

function relative(root, file) {
	return path.relative(root, file).split(path.sep).join("/");
}

function inventoryKey(source, id, title) {
	return `${path.basename(source)}\0${id}\0${title}`;
}

function splitMarkdownRow(line) {
	return line
		.slice(1, -1)
		.split(/(?<!\\)\|/u)
		.map((cell) => cell.trim().replaceAll("\\|", "|"));
}

function compact(value) {
	const text = String(value).replace(/\s+/gu, " ").trim();
	return text.length > 180 ? `${text.slice(0, 177)}…` : text;
}
