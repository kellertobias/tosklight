import { useRef } from "react";
import { ShowObjectsStore } from "../showObjects/store";

export function useShowObjectsState() {
	const showObjectsStore = useRef(new ShowObjectsStore()).current;
	return { showObjectsStore };
}
