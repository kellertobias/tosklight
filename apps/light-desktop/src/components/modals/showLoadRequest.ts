const SHOW_LOAD_REQUEST_EVENT = "light:open-show-load";

export function requestShowLoad() {
	window.dispatchEvent(new Event(SHOW_LOAD_REQUEST_EVENT));
}

export function subscribeShowLoadRequest(open: () => void) {
	window.addEventListener(SHOW_LOAD_REQUEST_EVENT, open);
	return () => window.removeEventListener(SHOW_LOAD_REQUEST_EVENT, open);
}
