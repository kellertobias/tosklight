import React from 'react';
import { BrowserRouter, Route, Switch } from 'react-router-dom'; // Pages

import { Home } from './layouts/Home';

export const App = () => {
	return (
		<BrowserRouter>
			<Switch>
				<Route exact path="/" component={Home} />
			</Switch>
		</BrowserRouter>
	);
};
