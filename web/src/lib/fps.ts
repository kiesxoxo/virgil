// A dropped-frame probe for measuring on phones: open any page with ?fps. While something
// scrolls or a finger moves, it counts frames that came late (a gap of more than 1.5 frames)
// and shows the share dropped in the last burst and since the page opened. ?fps&blur=0 also
// turns off every backdrop blur, to see what the frosted bars cost.

/** A burst of activity ends this long after its last scroll or touch. */
const QUIET_MS = 400;

export function probe(): () => void {
	const label = document.createElement('div');
	label.setAttribute('aria-hidden', 'true');
	label.style.cssText =
		'position:fixed;top:calc(env(safe-area-inset-top) + 4px);left:50%;translate:-50% 0;z-index:100;pointer-events:none;' +
		'padding:2px 6px;border-radius:6px;background:rgb(0 0 0 / 0.75);color:#fff;font:600 11px/1.4 ui-monospace,monospace;white-space:pre';
	document.body.append(label);

	let blur: HTMLStyleElement | undefined;
	if (new URLSearchParams(location.search).get('blur') === '0') {
		blur = document.createElement('style');
		blur.textContent = '*{backdrop-filter:none!important;-webkit-backdrop-filter:none!important}';
		document.head.append(blur);
	}

	const gaps: number[] = [];
	let nominal = 1000 / 60;
	let last = 0;
	let activeUntil = 0;
	let burst = { frames: 0, dropped: 0 };
	let total = { frames: 0, dropped: 0 };
	let shown = '';

	const pct = (c: { frames: number; dropped: number }) =>
		c.frames ? `${((100 * c.dropped) / (c.frames + c.dropped)).toFixed(1)}% (${c.dropped}/${c.frames + c.dropped})` : '–';

	const active = () => {
		const now = performance.now();
		if (now > activeUntil) burst = { frames: 0, dropped: 0 };
		activeUntil = now + QUIET_MS;
	};

	let raf = requestAnimationFrame(function frame(now) {
		raf = requestAnimationFrame(frame);
		if (last) {
			const gap = now - last;
			// The display's own frame interval: the median of recent gaps (60 or 120 Hz).
			gaps.push(gap);
			if (gaps.length > 120) gaps.shift();
			nominal = gaps.toSorted((a, b) => a - b)[gaps.length >> 1];
			if (now <= activeUntil + nominal) {
				const missed = gap > nominal * 1.5 ? Math.round(gap / nominal) - 1 : 0;
				burst.frames++;
				burst.dropped += missed;
				total.frames++;
				total.dropped += missed;
			}
		}
		last = now;
		const text = `${Math.round(1000 / nominal)} Hz${blur ? ' · no blur' : ''}\nlast ${pct(burst)}\nall  ${pct(total)}`;
		if (text !== shown) label.textContent = shown = text;
	});

	const options = { capture: true, passive: true };
	for (const type of ['scroll', 'touchmove', 'wheel'] as const) addEventListener(type, active, options);
	return () => {
		cancelAnimationFrame(raf);
		for (const type of ['scroll', 'touchmove', 'wheel'] as const) removeEventListener(type, active, options);
		label.remove();
		blur?.remove();
	};
}
