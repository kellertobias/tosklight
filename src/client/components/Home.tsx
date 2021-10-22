import React, { useEffect, useState } from 'react';
import API from '/client/api';
import { useEffectAsync } from '/client/utils/hooks';

export const Home = () => {
	const [data, setData] = useState({} as any);
	useEffectAsync(async () => {
		const result = await API.views.home();

		setData(result);
	}, []);
	return <div>{JSON.stringify(data)}</div>;
};
