// A dropped-frame probe for measuring on phones: open any page with ?fps. While something
// scrolls or a finger moves, and for a moment after a tap (a page or a view changing), it counts
// frames that came late (a gap of more than 1.5 frames) and shows the share dropped in the last
// burst and since the page opened, and the last burst's longest gap: after a tap, mostly the wait
// before anything shows. Under it, where the burst went (`/ → /album`) and when its frames were
// dropped, in ms from its start.

/** A burst of activity ends this long after its last scroll or touch. */
const QUIET_MS = 400;
/** What a tap sets off is over by this long after it. */
const TAP_MS = 1000;

/** The page's first path segment: enough to tell the albums, an album and Now Playing apart. */
const where = () => '/' + (location.pathname.split('/')[1] ?? '');

interface Burst {
	frames: number;
	dropped: number;
	worst: number;
	start: number;
	from: string;
	/** When each late frame came, in ms from the start. */
	drops: number[];
}

const burstAt = (start: number): Burst => ({ frames: 0, dropped: 0, worst: 0, start, from: where(), drops: [] });

export function probe(): () => void {
	const label = document.createElement('div');
	label.setAttribute('aria-hidden', 'true');
	label.style.cssText =
		'position:fixed;top:calc(env(safe-area-inset-top) + 4px);left:50%;translate:-50% 0;z-index:100;pointer-events:none;' +
		'padding:2px 6px;border-radius:6px;background:rgb(0 0 0 / 0.75);color:#fff;font:600 11px/1.4 ui-monospace,monospace;white-space:pre';
	document.body.append(label);


	const gaps: number[] = [];
	let nominal = 1000 / 60;
	let last = 0;
	let activeUntil = 0;
	let burst = burstAt(0);
	let total = { frames: 0, dropped: 0 };
	let shown = '';

	const pct = (c: { frames: number; dropped: number }) =>
		c.frames ? `${((100 * c.dropped) / (c.frames + c.dropped)).toFixed(1)}% (${c.dropped}/${c.frames + c.dropped})` : '–';

	const active = (e: Event) => {
		const now = performance.now();
		if (now > activeUntil) burst = burstAt(now);
		activeUntil = Math.max(activeUntil, now + (e.type === 'click' ? TAP_MS : QUIET_MS));
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
				burst.worst = Math.max(burst.worst, gap);
				if (missed && burst.drops.length < 8) burst.drops.push(Math.round(now - burst.start));
				total.frames++;
				total.dropped += missed;
			}
		}
		last = now;
		const worst = burst.frames ? ` · worst ${Math.round(burst.worst)} ms` : '';
		const to = where();
		const route = burst.from === to ? to : `${burst.from} → ${to}`;
		const drops = burst.drops.length ? ` · drops at ${burst.drops.join(' ')} ms` : '';
		const detail = burst.frames ? `\n     ${route}${drops}` : '';
		const text = `${Math.round(1000 / nominal)} Hz\nlast ${pct(burst)}${worst}${detail}\nall  ${pct(total)}`;
		if (text !== shown) label.textContent = shown = text;
	});

	const options = { capture: true, passive: true };
	const types = ['scroll', 'touchmove', 'wheel', 'click'] as const;
	for (const type of types) addEventListener(type, active, options);
	return () => {
		cancelAnimationFrame(raf);
		for (const type of types) removeEventListener(type, active, options);
		label.remove();
	};
}
