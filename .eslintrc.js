module.exports = {
	env: {
		node: true,
		mocha: true,
		browser: true,
		'jest/globals': true,
	},
	plugins: ['jest', 'prettier'],
	extends: [
		'airbnb-typescript',
		'plugin:@typescript-eslint/recommended',
		'plugin:eslint-comments/recommended',
		'plugin:promise/recommended',
		'plugin:unicorn/recommended',
		'prettier',
		'prettier/react',
		'prettier/@typescript-eslint',
	],
	parserOptions: {
		project: 'tsconfig.json',
		tsconfigRootDir: __dirname,
	},
	rules: {
		'arrow-body-style': 'off',
		'consistent-return': 'off',
		curly: 'error',
		'import/prefer-default-export': 'warn',
		'import/no-extraneous-dependencies': [
			'error',
			{
				devDependencies: [
					'**/*.stories.tsx',
					'**/*.stories.ts',
					'**/*.spec.tsx',
					'**/*.spec.ts',
				],
			},
		],
		'import/order': [
			'error',
			{
				'newlines-between': 'always',
				groups: [
					'builtin',
					'external',
					'internal',
					'parent',
					'sibling',
					'index',
				],
				pathGroups: [
					{
						pattern: '@/**',
						group: 'internal',
						position: 'after',
					},
				],
				pathGroupsExcludedImportTypes: ['builtin'],
			},
		],
		'import/prefer-default-export': 'off',
		'jsx-a11y/label-has-associated-control': 'warn',
		'no-console': 'off',
		'no-restricted-imports': [
			'error',
			{
				patterns: ['*.mocked*'],
			},
		],
		'no-underscore-dangle': [
			'error',
			{
				allow: [
					'__blinkit_branding_initial__',
					'__injected_globals__',
					'__community__',
					'__features__',
					'__changedDep',
					'__community__',
					'__dep',
					'_id',
				],
			},
		],
		'no-plusplus': [
			'error',
			{
				allowForLoopAfterthoughts: true,
			},
		],
		'no-undef': ['error'],
		'nonblock-statement-body-position': ['error', 'below'],
		'react/prop-types': 'off',
		'react/jsx-props-no-spreading': 'off',
		'unicorn/better-regex': 'warn',
		'unicorn/no-array-reduce': 'off',
		'unicorn/no-null': 'off',
		'unicorn/prevent-abbreviations': 'off',
		'unicorn/prefer-dom-node-remove': 'off',
		'unicorn/prefer-spread': 'warn',
		'@typescript-eslint/no-implied-eval': 'warn',
		'@typescript-eslint/no-unused-expressions': 'error',
		'@typescript-eslint/no-unused-vars': 'error',
		'class-methods-use-this': 'off',
		'jsx-a11y/click-events-have-key-events': 'off',
		'jsx-a11y/no-noninteractive-element-interactions': 'off',
		'jsx-a11y/no-static-element-interactions': 'off',
		'jsx-a11y/interactive-supports-focus': 'off',
	},
	overrides: [
		{
			files: ['**/*.spec.tsx', '**/*.stories.tsx'],
			rules: {
				'no-restricted-imports': 'off',
				'unicorn/no-array-callback-reference': 'off',
			},
		},
	],
	globals: {
		Meteor: true,
		check: true,
		NodeJS: true,
		T: true,
		JSX: true,
		__blinkit_branding_initial__: true,
		__community__: true,
	},
};
