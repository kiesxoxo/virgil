// Swiping right goes back, as in an iOS app: on a page with a back link (marked data-back), a
// swipe from left to right anywhere on it follows that link, so each page goes back its own way
// (an album to the list it came from). Not from something that moves sideways itself: a row that
// scrolls across, or the tuner's dial. A page can also be pulled down
// from its top to go back (pullBack), as a sheet is.

/** At least this far across, or flicked this fast (px/ms) at least half as far. */
const DISTANCE = 80;
const FLICK = 0.5;

/** Whether `el` or an element around it scrolls (or swipes) sideways itself. */
function sideways(el: Element | null): boolean {
	for (; el && el !== document.body; el = el.parentElement) {
		if (el instanceof HTMLElement && el.dataset.swipe !== undefined) return true;
		if (el.scrollWidth > el.clientWidth + 1 && /auto|scroll/.test(getComputedStyle(el).overflowX)) return true;
	}
	return false;
}

/** The page's back link in sight, if it has one. */
function backLink(): HTMLElement | undefined {
	return [...document.querySelectorAll<HTMLElement>('[data-back]')].find((el) => el.getClientRects().length);
}

/** Listens for swipes back until the returned function is called. */
export function swipeBack(): () => void {
	let start: { x: number; y: number; t: number } | null = null;

	const begin = (e: TouchEvent) => {
		start = null;
		if (e.touches.length !== 1 || !backLink() || sideways(e.target as Element)) return;
		const touch = e.touches[0];
		start = { x: touch.clientX, y: touch.clientY, t: e.timeStamp };
	};
	const end = (e: TouchEvent) => {
		if (!start || e.touches.length) return;
		const touch = e.changedTouches[0];
		const [dx, dy] = [touch.clientX - start.x, touch.clientY - start.y];
		const speed = dx / Math.max(1, e.timeStamp - start.t);
		start = null;
		// Mostly across, not a scroll down the page that drifted.
		if (Math.abs(dy) > dx * 0.5) return;
		if (dx > DISTANCE || (speed > FLICK && dx > DISTANCE / 2)) backLink()?.click();
	};
	// A gesture of the browser's own (Safari's edge swipe goes back itself) cancels it.
	const cancel = () => (start = null);

	const options = { passive: true };
	addEventListener('touchstart', begin, options);
	addEventListener('touchend', end, options);
	addEventListener('touchcancel', cancel, options);
	return () => {
		removeEventListener('touchstart', begin);
		removeEventListener('touchend', end);
		removeEventListener('touchcancel', cancel);
	};
}

/** Pulled down this far (px), or flicked down this fast (px/ms), it goes back. */
const PULL = 120;

/**
 * Pulling `node` (a page) down from the top of the page goes back: the page follows the finger,
 * and let go far enough it follows the back link; not far enough, it is back in place at once. Only here, not on the whole window: a touchmove listener that
 * can stop scrolling makes every scroll it covers wait for it.
 */
export function pullBack(node: HTMLElement): () => void {
	let start: { x: number; y: number; t: number } | null = null;
	let pulling = false;

	const place = (y: number) => {
		node.style.transform = y ? `translateY(${y}px)` : '';
	};
	const begin = (e: TouchEvent) => {
		const touch = e.touches[0];
		start = e.touches.length === 1 && scrollY <= 0 && backLink() ? { x: touch.clientX, y: touch.clientY, t: e.timeStamp } : null;
		pulling = false;
	};
	const move = (e: TouchEvent) => {
		if (!start) return;
		const touch = e.touches[0];
		const [dx, dy] = [touch.clientX - start.x, touch.clientY - start.y];
		if (!pulling) {
			// Up or across: the page's scroll, or a swipe back.
			if (dy < 0 || Math.abs(dx) > dy) {
				if (Math.max(Math.abs(dx), Math.abs(dy)) > 10) start = null;
				return;
			}
			// Down at the top the page would only bounce: from here it is a pull.
			e.preventDefault();
			if (dy < 10) return;
			pulling = true;
		}
		e.preventDefault();
		place(Math.max(0, dy));
	};
	const end = (e: TouchEvent) => {
		if (!start || e.touches.length) return;
		const dy = e.changedTouches[0].clientY - start.y;
		const speed = dy / Math.max(1, e.timeStamp - start.t);
		start = null;
		if (!pulling) return;
		pulling = false;
		if (dy > PULL || speed > FLICK) backLink()?.click();
		else place(0);
	};
	const cancel = () => {
		if (pulling) place(0);
		[start, pulling] = [null, false];
	};

	node.addEventListener('touchstart', begin, { passive: true });
	node.addEventListener('touchmove', move, { passive: false });
	node.addEventListener('touchend', end, { passive: true });
	node.addEventListener('touchcancel', cancel, { passive: true });
	return () => {
		node.removeEventListener('touchstart', begin);
		node.removeEventListener('touchmove', move);
		node.removeEventListener('touchend', end);
		node.removeEventListener('touchcancel', cancel);
	};
}
