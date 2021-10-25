import { Story } from '@storybook/react';
import React from 'react';
import { Button as ButtonComponent, ButtonProps } from './button';

export default {
	title: 'Components/Button',
};

export const Button = () => (
	<div>
		<ButtonComponent>Text</ButtonComponent>
	</div>
);

// Button.args = {
// 	children: 'Button Text',
// };

// Button.argTypes = {
// 	type: {
// 		control: {
// 			type: 'select',
// 			options: ['default', 'danger', 'success'],
// 		},
// 	},
// 	disabled: {
// 		control: 'boolean',
// 	},
// };
