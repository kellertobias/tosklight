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
	const files = (sourceFiles ?? discoverMarkedSpecs(path.join(root, "tests")))
		.map((file) => path.resolve(file))
		.sort();
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
				throw new Error(
					`TypeScript did not return an AST for ${relative(root, file)}`,
				);
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
					project,
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
			throw new Error(
				`Semantic scenario occurrence was compiled twice: ${key}`,
			);
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
		.filter((file) =>
			fs.readFileSync(file, "utf8").includes(semanticWorldMarker),
		);
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

function compileSourceFile({
	ast,
	root,
	sourceFile,
	project,
	inventory,
	results,
}) {
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
				helpers: new Map(helpers),
				project,
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
	project,
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
		throw new Error(
			`${source.file}:${source.line} scenario world parameter is not static`,
		);

	const diagnostics = [];
	const state = {
		ast,
		sourceFile,
		root,
		helpers,
		project,
		checker: project.checker,
		worldNames: new Set([worldParameter.text]),
		aliases: new Map(),
		staticBindings: new Map(),
		helperStack: [],
		preconditions: [],
		steps: [],
		expectedOutcomes: [],
		contextLabels: [],
		sequence: { next: 0 },
		diagnostics,
		phase: "scenario",
	};
	visitExecution(callback.body, state);

	const matches =
		inventory.get(inventoryKey(source.file, idNode.text, titleNode.text)) ?? [];
	if (matches.length !== 1) {
		diagnostics.push({
			code: matches.length
				? "ambiguous-migration-status"
				: "missing-migration-status",
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
	const actualSurfaces = new Set(
		migration.surfaces.filter((surface) => !surface.startsWith("@")),
	);
	for (const item of [
		...state.preconditions,
		...state.steps,
		...state.expectedOutcomes,
	])
		for (const surface of item.surfaces) actualSurfaces.add(surface);
	return {
		id: idNode.text,
		title: titleNode.text,
		source,
		preconditions: state.preconditions,
		steps: state.steps,
		expectedOutcomes: state.expectedOutcomes,
		contextLabels: uniqueContextLabels(state.contextLabels),
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
		const condition = staticPrimitive(node.expression, state);
		if (typeof condition === "boolean") {
			const branch = condition ? node.thenStatement : node.elseStatement;
			if (branch) visitExecution(branch, state);
			return;
		}
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
		if (ast.isForOfStatement(node)) {
			const iterations = resolveStaticForOfValues(node.expression, state);
			if (iterations) {
				for (const [iteration, value] of iterations.entries()) {
					const iterationState = {
						...state,
						aliases: new Map(state.aliases),
						staticBindings: new Map(state.staticBindings),
						phase: `${state.phase}:iteration-${iteration + 1}`,
					};
					bindForOfInitializer(node.initializer, value, iterationState);
					visitExecution(node.statement, iterationState);
				}
				return;
			}
		}
		state.diagnostics.push({
			code: "static-control-flow",
			message: `Control flow ${compact(node.getText(state.sourceFile))} is narrated once; iteration values are not guessed.`,
			source: location(state.root, state.sourceFile, node),
		});
		if ("initializer" in node && node.initializer)
			registerLoopBinding(node, state);
		visitExecution(node.statement, state);
		return;
	}
	if (ast.isVariableDeclaration(node)) {
		if (ast.isIdentifier(node.name) && node.initializer) {
			if (
				ast.isArrowFunction(node.initializer) ||
				ast.isFunctionExpression(node.initializer)
			) {
				state.helpers.set(node.name.text, {
					node: node.initializer,
					capturesWorld: true,
				});
				return;
			}
			state.staticBindings.set(node.name.text, node.initializer);
			const initializerCall = unwrapCall(node.initializer, ast);
			if (initializerCall) {
				if (
					ast.isIdentifier(initializerCall.expression) &&
					state.helpers.has(initializerCall.expression.text)
				) {
					expandHelper(initializerCall, state);
					state.aliases.set(node.name.text, {
						origin: initializerCall.expression.text,
					});
					return;
				}
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
		if (emitSemanticCall(node, state)) {
			for (const argument of node.arguments)
				if (ast.isArrowFunction(argument) || ast.isFunctionExpression(argument))
					visitExecution(argument.body, {
						...state,
						phase: `${state.phase}:callback`,
					});
			return;
		}
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
	const source = location(state.root, state.sourceFile, node);
	const expressionDiagnostics = [];
	const args = chain.argumentNodes.map((argument) =>
		renderExpression(argument, state, expressionDiagnostics),
	);
	for (const diagnostic of expressionDiagnostics)
		state.diagnostics.push({
			code: diagnostic.code,
			message: diagnostic.message,
			expression: diagnostic.expression,
			relatedCall: chain.path,
			relatedSource: source,
			sourceCode: (diagnostic.node ?? node).getText(state.sourceFile),
			source: location(state.root, state.sourceFile, diagnostic.node ?? node),
		});
	const narration = narrateCall({
		path: chain.path,
		root: chain.root,
		arguments: args,
	});
	const raw = compact(node.getText(state.sourceFile));
	if (!narration) {
		state.diagnostics.push({
			code: "unknown-narration",
			message: `No narration catalog entry matches ${chain.path}; source is preserved as ${raw}.`,
			source,
		});
		const unresolved = {
			kind: chain.path.includes("expect") ? "expected-outcome" : "step",
			description: `Unresolved semantic call: ${raw}`,
			call: chain.path,
			arguments: args,
			expression: raw,
			sourceCode: node.getText(state.sourceFile),
			surfaces: ["Unresolved"],
			phase: state.phase,
			order: state.sequence.next++,
			source,
		};
		(chain.path.includes("expect") ? state.expectedOutcomes : state.steps).push(
			unresolved,
		);
		return { path: chain.path };
	}
	const item = {
		kind: narration.kind,
		description: narration.description,
		call: chain.path,
		arguments: args,
		presentation: narration.presentation,
		expression: raw,
		sourceCode: node.getText(state.sourceFile),
		surfaces: narration.surfaces,
		phase: state.phase,
		order: state.sequence.next++,
		source,
	};
	if (narration.kind === "precondition") state.preconditions.push(item);
	else if (narration.kind === "expected-outcome")
		state.expectedOutcomes.push(item);
	else state.steps.push(item);
	if (narration.contextLabel) state.contextLabels.push(narration.contextLabel);
	if (narration.expectedOutcome)
		state.expectedOutcomes.push({
			...item,
			kind: "expected-outcome",
			description: narration.expectedOutcome,
			phase: `${state.phase}:catalog-outcome`,
			order: state.sequence.next++,
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
	const helperEntry = state.helpers.get(name);
	const helper = helperEntry.node;
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
		if (
			argument &&
			state.ast.isIdentifier(argument) &&
			state.worldNames.has(argument.text)
		) {
			worldNames.add(parameter.name.text);
			receivesWorld = true;
		} else if (argument) {
			staticBindings.set(parameter.name.text, argument);
		}
	}
	if (!receivesWorld && !helperEntry.capturesWorld) {
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
	if (ast.isTemplateExpression(node)) {
		let value = node.head.text;
		for (const span of node.templateSpans) {
			value +=
				generatedValue(span.expression, ast) ??
				renderExpression(span.expression, state, diagnostics, seen);
			value += span.literal.text;
		}
		return JSON.stringify(value);
	}
	if (ast.isNumericLiteral(node)) return node.text;
	if (node.kind === ast.SyntaxKind.RegularExpressionLiteral)
		return node.getText(sourceFile);
	if (node.kind === ast.SyntaxKind.TrueKeyword) return "true";
	if (node.kind === ast.SyntaxKind.FalseKeyword) return "false";
	if (node.kind === ast.SyntaxKind.NullKeyword) return "null";
	if (ast.isIdentifier(node)) {
		if (node.text === "undefined") return "undefined";
		const alias = state.aliases.get(node.text);
		if (alias) return `$${node.text} (result of ${alias.origin})`;
		const binding = state.staticBindings.get(node.text);
		if (binding && !seen.has(node.text)) {
			if (binding.staticText) return binding.staticText;
			const nextSeen = new Set(seen).add(node.text);
			return `${node.text} = ${renderExpression(binding, state, diagnostics, nextSeen)}`;
		}
		diagnostics.push({
			code: "unresolved-expression",
			message: `Expression ${node.text} is not statically resolved.`,
			expression: node.text,
			node,
		});
		return `<unresolved: ${node.text}>`;
	}
	if (ast.isPropertyAccessExpression(node)) {
		const raw = compact(node.getText(sourceFile));
		const semanticConstant = staticSemanticProperty(raw);
		if (semanticConstant !== undefined) return semanticConstant;
		const root = leftmostIdentifier(node, ast);
		const alias = root ? state.aliases.get(root) : undefined;
		if (alias) return `$${raw} (from ${alias.origin})`;
		if (root && /^[A-Z]/u.test(root)) return raw;
		diagnostics.push({
			code: "unresolved-expression",
			message: `Property access ${raw} depends on a runtime value.`,
			expression: raw,
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
				expression: compact(property.getText(sourceFile)),
				node: property,
			});
			return `<unresolved: ${compact(property.getText(sourceFile))}>`;
		});
		return `{ ${values.join(", ")} }`;
	}
	if (ast.isSpreadElement(node))
		return `...${renderExpression(node.expression, state, diagnostics, seen)}`;
	if (ast.isElementAccessExpression(node)) {
		const collection = resolveStaticNode(node.expression, state);
		const index = staticPrimitive(node.argumentExpression, state);
		if (
			ast.isArrayLiteralExpression(collection) &&
			typeof index === "number" &&
			Number.isInteger(index) &&
			collection.elements[index]
		)
			return renderExpression(
				collection.elements[index],
				state,
				diagnostics,
				seen,
			);
	}
	if (ast.isConditionalExpression(node)) {
		const condition = staticPrimitive(node.condition, state);
		if (typeof condition === "boolean")
			return renderExpression(
				condition ? node.whenTrue : node.whenFalse,
				state,
				diagnostics,
				seen,
			);
	}
	if (ast.isArrowFunction(node) || ast.isFunctionExpression(node))
		return "<generated: callback>";
	if (ast.isCallExpression(node)) {
		const generated = generatedValue(node, ast);
		if (generated) return generated;
		const raw = compact(node.getText(sourceFile));
		const semanticCall = callChain(node, state);
		if (semanticCall?.path === "preset.routeReports.at")
			return `<observed: ${raw}>`;
		if (
			semanticCall &&
			(semanticCall.path === "expect.stringMatching" ||
				narrateCall({
					path: semanticCall.path,
					root: semanticCall.root,
					arguments: [],
				}))
		)
			return raw;
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
			expression: raw,
			node,
		});
		return `<unresolved: ${raw}>`;
	}
	const raw = compact(node.getText(sourceFile));
	diagnostics.push({
		code: "unresolved-expression",
		message: `Expression ${raw} is not statically resolved.`,
		expression: raw,
		node,
	});
	return `<unresolved: ${raw}>`;
}

function staticSemanticProperty(raw) {
	const speedGroup = /^t\.speedGroup\.([A-E])\.group$/u.exec(raw);
	return speedGroup ? JSON.stringify(speedGroup[1]) : undefined;
}

function generatedValue(node, ast) {
	if (
		!ast.isCallExpression(node) ||
		node.arguments.length ||
		!ast.isPropertyAccessExpression(node.expression) ||
		!ast.isIdentifier(node.expression.expression) ||
		node.expression.expression.text !== "crypto" ||
		node.expression.name.text !== "randomUUID"
	)
		return undefined;
	return "<generated: UUID>";
}

function importedScenarioNames(ast, sourceFile) {
	const names = new Set();
	for (const statement of sourceFile.statements) {
		if (!ast.isImportDeclaration(statement)) continue;
		if (!ast.isStringLiteral(statement.moduleSpecifier)) continue;
		if (!statement.moduleSpecifier.text.endsWith("/bench/core/scenario"))
			continue;
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
		if (
			ast.isFunctionDeclaration(statement) &&
			statement.name &&
			statement.body
		)
			functions.set(statement.name.text, {
				node: statement,
				capturesWorld: false,
			});
	return functions;
}

function registerLoopBinding(node, state) {
	const declarationList = node.initializer;
	if (!state.ast.isVariableDeclarationList(declarationList)) return;
	for (const declaration of declarationList.declarations)
		if (state.ast.isIdentifier(declaration.name))
			state.staticBindings.delete(declaration.name.text);
}

function resolveStaticForOfValues(expression, state) {
	const { ast } = state;
	const value = resolveStaticNode(expression, state);
	if (ast.isArrayLiteralExpression(value)) return [...value.elements];
	if (
		ast.isCallExpression(value) &&
		!value.arguments.length &&
		ast.isPropertyAccessExpression(value.expression) &&
		value.expression.name.text === "entries"
	) {
		const collection = resolveStaticNode(value.expression.expression, state);
		if (ast.isArrayLiteralExpression(collection))
			return collection.elements.map((element, index) => ({
				tupleValues: [{ staticText: String(index) }, element],
			}));
	}
	if (
		ast.isCallExpression(value) &&
		ast.isPropertyAccessExpression(value.expression) &&
		value.expression.expression.getText(state.sourceFile) === "Object" &&
		value.expression.name.text === "values" &&
		value.arguments.length === 1 &&
		ast.isIdentifier(value.arguments[0])
	) {
		const enumName = value.arguments[0].text;
		let symbol = state.checker.getSymbolAtLocation(value.arguments[0]);
		if (!symbol) return undefined;
		if (symbol.flags & state.ast.SymbolFlags.Alias)
			symbol = state.checker.getAliasedSymbol(symbol);
		const declarationHandle = symbol.valueDeclaration ?? symbol.declarations[0];
		const declaration = declarationHandle?.resolve(state.project);
		if (!declaration || !ast.isEnumDeclaration(declaration)) return undefined;
		return declaration.members.map((member) => ({
			staticText: `${enumName}.${member.name.getText(declaration.getSourceFile())}`,
		}));
	}
	return undefined;
}

function bindForOfInitializer(initializer, value, state) {
	const { ast } = state;
	if (!ast.isVariableDeclarationList(initializer)) return;
	const declaration = initializer.declarations[0];
	if (!declaration) return;
	bindStaticPattern(declaration.name, value, state);
}

function bindStaticPattern(pattern, value, state) {
	const { ast } = state;
	if (ast.isIdentifier(pattern)) {
		state.staticBindings.set(pattern.text, value);
		return;
	}
	if (!ast.isArrayBindingPattern(pattern)) return;
	if (value.tupleValues) {
		for (const [index, binding] of pattern.elements.entries()) {
			if (!ast.isBindingElement(binding)) continue;
			const elementValue = value.tupleValues[index];
			if (elementValue) bindStaticPattern(binding.name, elementValue, state);
		}
		return;
	}
	const unwrapped = value.staticText
		? undefined
		: unwrapStaticExpression(value, ast);
	if (!unwrapped || !ast.isArrayLiteralExpression(unwrapped)) return;
	for (const [index, binding] of pattern.elements.entries()) {
		if (!ast.isBindingElement(binding)) continue;
		const elementValue = unwrapped.elements[index];
		if (elementValue) bindStaticPattern(binding.name, elementValue, state);
	}
}

function unwrapStaticExpression(node, ast) {
	let current = node;
	while (
		ast.isAsExpression(current) ||
		ast.isSatisfiesExpression(current) ||
		ast.isParenthesizedExpression(current)
	)
		current = current.expression;
	return current;
}

function resolveStaticNode(node, state, seen = new Set()) {
	const value = unwrapStaticExpression(node, state.ast);
	if (
		state.ast.isIdentifier(value) &&
		value.text !== "undefined" &&
		!seen.has(value.text)
	) {
		const binding = state.staticBindings.get(value.text);
		if (binding)
			return resolveStaticNode(binding, state, new Set(seen).add(value.text));
	}
	return value;
}

function staticPrimitive(node, state, seen = new Set()) {
	if (!node) return undefined;
	const value = resolveStaticNode(node, state, seen);
	const { ast } = state;
	if (value.staticText && /^-?\d+(?:\.\d+)?$/u.test(value.staticText))
		return Number(value.staticText);
	if (ast.isNumericLiteral(value)) return Number(value.text);
	if (ast.isStringLiteral(value) || ast.isNoSubstitutionTemplateLiteral(value))
		return value.text;
	if (value.kind === ast.SyntaxKind.TrueKeyword) return true;
	if (value.kind === ast.SyntaxKind.FalseKeyword) return false;
	if (value.kind === ast.SyntaxKind.NullKeyword) return null;
	if (ast.isIdentifier(value) && value.text === "undefined")
		return staticUndefined;
	if (ast.isBinaryExpression(value)) {
		const left = staticPrimitive(value.left, state, seen);
		const right = staticPrimitive(value.right, state, seen);
		if (left === undefined || right === undefined) return undefined;
		switch (value.operatorToken.kind) {
			case ast.SyntaxKind.EqualsEqualsToken:
				return looselyEqualStatic(left, right);
			case ast.SyntaxKind.EqualsEqualsEqualsToken:
				return normalizeStatic(left) === normalizeStatic(right);
			case ast.SyntaxKind.ExclamationEqualsToken:
				return !looselyEqualStatic(left, right);
			case ast.SyntaxKind.ExclamationEqualsEqualsToken:
				return normalizeStatic(left) !== normalizeStatic(right);
			default:
				return undefined;
		}
	}
	return undefined;
}

const staticUndefined = Symbol("static undefined");

function normalizeStatic(value) {
	return value === staticUndefined ? undefined : value;
}

function looselyEqualStatic(left, right) {
	const normalizedLeft = normalizeStatic(left);
	const normalizedRight = normalizeStatic(right);
	const leftNullish = normalizedLeft === null || normalizedLeft === undefined;
	const rightNullish =
		normalizedRight === null || normalizedRight === undefined;
	if (leftNullish || rightNullish) return leftNullish && rightNullish;
	return normalizedLeft === normalizedRight;
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
	const position = sourceFile.getLineAndCharacterOfPosition(
		node.getStart(sourceFile),
	);
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

function uniqueContextLabels(labels) {
	const seen = new Set();
	return labels.filter((label) => {
		const key = `${label.kind}\0${label.label}`;
		if (seen.has(key)) return false;
		seen.add(key);
		return true;
	});
}
