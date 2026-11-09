// The way back. Each history entry remembers the route it was opened from (page.state.opener), so
// going back does the same however the page came up: from a link, by coming back to it through
// history, or after a reload. With somewhere in the app to go back to it's the browser's own back,
// which puts that page back as it was left (the grid scrolled); opened from outside, it's `fallback`.

import { afterNavigate, goto } from '$app/navigation';
import { page } from '$app/state';

/** Notes where each new history entry was opened from. Call once, from the root layout. */
export function trackOpeners() {
	afterNavigate(({ from, type }) => {
		// Coming from outside there's nothing to note; through history, the entry has it already.
		// Noting it is a navigation itself, which ends up here with the opener set.
		if (type === 'enter' || type === 'popstate' || page.state.opener !== undefined) return;
		goto(page.url.href, { shallow: true, replace: true, state: { opener: from?.route.id ?? null }, persistState: true });
	});
}

export function goBack(fallback: string) {
	if (page.state.opener) history.back();
	else goto(fallback);
}

/** Goes to `href` in this page's place in history: it's still opened from the same place. */
export function swapTo(href: string) {
	return goto(href, { replace: true, reset: false, state: { opener: page.state.opener ?? null }, persistState: true });
}
