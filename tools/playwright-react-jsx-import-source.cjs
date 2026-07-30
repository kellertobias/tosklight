"use strict";

/**
 * Playwright's TSX transform defaults JSX to its component-testing runtime.
 * ToskLight E2E helpers import production React TSX, so make that runtime
 * explicit before Playwright's built-in JSX transform visits the program.
 */
module.exports = function playwrightReactJsxImportSource() {
	return {
		name: "tosklight-playwright-react-jsx-import-source",
		pre(file) {
			file.ast.comments ??= [];
			if (
				file.ast.comments.some((comment) =>
					comment.value.includes("@jsxImportSource"),
				)
			)
				return;
			file.ast.comments.push({
				type: "CommentBlock",
				value: "* @jsxImportSource react ",
			});
		},
	};
};
