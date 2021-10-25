import React from 'react';
import clsx from 'clsx';

export type ButtonProps = {
	type?: 'default' | 'danger' | 'success' | 'primary';
	disabled?: boolean;
};

export const Button: React.FC<ButtonProps> = ({ type, disabled, children }) => {
	return (
		<button
			className={clsx({
				button: true,
				[`button-${type}`]: type !== undefined,
				'button-disabled': disabled,
			})}
		>
			{children}
		</button>
	);
};
