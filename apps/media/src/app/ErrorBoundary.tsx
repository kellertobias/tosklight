// The last line of defence. A rendering fault in one panel must not leave an operator staring at
// a blank browser window with no way back.

import { Button } from "@tosklight/ui/controls";
import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
	children: ReactNode;
}

interface State {
	failed: boolean;
}

export class ErrorBoundary extends Component<Props, State> {
	state: State = { failed: false };

	static getDerivedStateFromError(): State {
		return { failed: true };
	}

	componentDidCatch(error: Error, info: ErrorInfo): void {
		console.error("the administration interface failed to render", error, info);
	}

	render(): ReactNode {
		if (!this.state.failed) return this.props.children;
		return (
			<div className="media-state is-error" role="alert">
				<p>
					The administration interface stopped responding. Program output is
					unaffected — the server keeps running whatever the desk asked for.
				</p>
				<Button variant="primary" onClick={() => window.location.reload()}>
					Reload the interface
				</Button>
			</div>
		);
	}
}
