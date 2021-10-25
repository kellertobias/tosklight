import React from 'react';

export default {
	component: () => <div />,
	title: 'Intro',
	parameters: {
		docs: {
			page: null,
		},
	},
};

export const Intro = () => (
	<div>
		<h1>Welcome to ToskLight Storybook</h1>
		<p>
			This storybook contains the Components and widgets of this software's UI
		</p>
	</div>
);
