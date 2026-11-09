<script lang="ts">
	import { flushSync } from 'svelte';
	import { goto } from '$app/navigation';
	import { keyboard } from '../keyboard.svelte.ts';
	import { reach } from '../reach.svelte.ts';
	import { search } from '../search.svelte.ts';
	import { followBackLink } from '../swipeback.ts';

	// The arc: a dial in the thumb's corner, in place of the tab bar. A thumb swings in an arc around
	// its base, so the keys lie in rings around the corner rather than in rows. Press the dial, slide
	// onto a key and let go; or tap the dial to leave it open and tap a key. Nothing animates: the
	// rings are there the moment the thumb is down.
	let { items, routeId }: { items: { href: string; label: string; active: (id: string) => boolean }[]; routeId: string } = $props();

	interface Key {
		label: string;
		run: () => void;
		lit?: boolean;
	}

	const KNOB = 76;
	const INNER = 158;
	const OUTER = 240;
	const SIZE = OUTER + 2;
	/** Ends a slide past the outer ring still counts as on it. */
	const SLACK = 70;

	let open = $state(false);
	let hover = $state<Key | null>(null);
	let dial: HTMLElement;
	let pressed: { x: number; y: number; wasOpen: boolean } | null = null;

	let library = $derived(routeId === '/' || routeId === '/artists' || routeId === '/genres');
	let detail = $derived(routeId.startsWith('/album/') || routeId.startsWith('/artist/') || routeId.startsWith('/genre/'));
	let here = $derived(items.find((item) => item.active(routeId))?.label ?? (routeId === '/now' ? 'Now' : ''));

	const destination = (label: string): Key => {
		const item = items.find((i) => i.label === label);
		return { label, run: () => item && goto(item.href), lit: item?.active(routeId) };
	};
	// The used most close by. One key changes: it sorts in the library, and goes back from a detail page.
	let context = $derived<Key>(
		detail
			? { label: 'Back', run: followBackLink }
			: library
				? { label: 'Sort', run: () => (reach.panel = !reach.panel), lit: reach.panel }
				: { label: 'Now', run: () => goto('/now') }
	);
	let rings = $derived<Key[][]>([
		[
			context,
			{
				label: 'Search',
				run: () => {
					search.open = true;
					// iOS only raises the keyboard for a focus() made during the tap itself.
					flushSync();
				}
			},
			destination('Queue'),
			destination('Radio')
		],
		[destination('Albums'), destination('Artists'), destination('Genres'), destination('Settings')]
	]);
	const radii = [
		[KNOB, INNER],
		[INNER, OUTER]
	];

	/** A point `r` from the corner, `angle` degrees up from along the bottom edge. */
	function point(r: number, angle: number) {
		const a = (angle * Math.PI) / 180;
		const x = r * Math.cos(a);
		return { x: reach.hand === 'right' ? SIZE - x : x, y: SIZE - r * Math.sin(a) };
	}

	function wedge(r0: number, r1: number, a0: number, a1: number) {
		const sweep = reach.hand === 'right' ? 1 : 0;
		const [p0, p1, p2, p3] = [point(r0, a0), point(r1, a0), point(r1, a1), point(r0, a1)];
		return `M${p0.x} ${p0.y}L${p1.x} ${p1.y}A${r1} ${r1} 0 0 ${sweep} ${p2.x} ${p2.y}L${p3.x} ${p3.y}A${r0} ${r0} 0 0 ${1 - sweep} ${p0.x} ${p0.y}Z`;
	}

	/** The key under a finger, by its distance and angle from the corner. */
	function keyAt(e: PointerEvent): Key | null {
		const box = dial.getBoundingClientRect();
		const dx = reach.hand === 'right' ? box.right - e.clientX : e.clientX - box.left;
		const dy = box.bottom - e.clientY;
		const r = Math.hypot(dx, dy);
		if (r < KNOB || r > OUTER + SLACK || dx < -SLACK || dy < -SLACK) return null;
		const angle = (Math.atan2(Math.max(dy, 0), Math.max(dx, 0)) * 180) / Math.PI;
		const ring = rings[r < INNER ? 0 : 1];
		return ring[Math.min(ring.length - 1, Math.floor(angle / (90 / ring.length)))];
	}

	function moved(e: PointerEvent) {
		hover = keyAt(e);
	}

	function lifted(e: PointerEvent) {
		window.removeEventListener('pointermove', moved);
		window.removeEventListener('pointerup', lifted);
		window.removeEventListener('pointercancel', cancelled);
		const key = keyAt(e);
		const from = pressed;
		pressed = null;
		hover = null;
		if (key) {
			open = false;
			key.run();
		} else if (from && Math.hypot(e.clientX - from.x, e.clientY - from.y) < 12) {
			// A tap on the dial: stays open for a second tap, or closes if it was open.
			open = !from.wasOpen;
		} else {
			open = false;
		}
	}

	function cancelled() {
		window.removeEventListener('pointermove', moved);
		window.removeEventListener('pointerup', lifted);
		window.removeEventListener('pointercancel', cancelled);
		[pressed, hover, open] = [null, null, false];
	}

	/** Holding still on the dial would bring up Safari's text magnifier, and a slide would scroll. */
	function holdStill(node: Element) {
		const stop = (e: Event) => e.preventDefault();
		node.addEventListener('touchstart', stop, { passive: false });
		return () => node.removeEventListener('touchstart', stop);
	}

	function down(e: PointerEvent) {
		e.preventDefault();
		pressed = { x: e.clientX, y: e.clientY, wasOpen: open };
		open = true;
		hover = keyAt(e);
		window.addEventListener('pointermove', moved);
		window.addEventListener('pointerup', lifted);
		window.addEventListener('pointercancel', cancelled);
	}

	let hidden = $derived(keyboard.height > 0 || search.open);
	let center = $derived(point(KNOB * 0.5, 45));
</script>

{#if open}
	<div class="touch-menu fixed inset-0 z-40 touch-none bg-black/70" role="presentation" onpointerdown={down} {@attach holdStill}></div>
{/if}

<div
	bind:this={dial}
	class="touch-menu pointer-events-none fixed z-40"
	class:hidden
	style:width="{SIZE}px"
	style:height="{SIZE}px"
	style:bottom="env(safe-area-inset-bottom)"
	style:left={reach.hand === 'left' ? '0' : 'auto'}
	style:right={reach.hand === 'right' ? '0' : 'auto'}
>
	<svg width={SIZE} height={SIZE} class="block overflow-visible" aria-hidden={!open}>
		{#if open}
			{#each rings as ring, r (r)}
				{#each ring as key, i (key.label)}
					{@const step = 90 / ring.length}
					{@const at = point((radii[r][0] + radii[r][1]) / 2, step * (i + 0.5))}
					{@const on = hover === key || hover?.label === key.label}
					<path d={wedge(radii[r][0], radii[r][1], step * i, step * (i + 1))} style:fill={on ? 'var(--fg)' : 'var(--bg)'} style:stroke="var(--line)" stroke-width="1.5" />
					<text
						x={at.x}
						y={at.y}
						text-anchor="middle"
						dominant-baseline="central"
						class={r === 0 ? 'label text-[10px]' : 'wide text-[10.5px] uppercase'}
						style:fill={on ? 'var(--bg)' : key.lit ? 'var(--accent)' : 'var(--fg)'}
					>
						{key.label}
					</text>
				{/each}
			{/each}
		{/if}
		<!-- The dial: says where you are, and is the one thing here that takes a touch until it opens. -->
		<path
			d="M{reach.hand === 'right' ? SIZE : 0} {SIZE}L{point(KNOB, 0).x} {SIZE}A{KNOB} {KNOB} 0 0 {reach.hand === 'right' ? 1 : 0} {point(KNOB, 90).x} {SIZE - KNOB}Z"
			class="pointer-events-auto touch-none"
			style:fill={open ? 'var(--accent)' : 'var(--fg)'}
			style:stroke="var(--bg)"
			stroke-width="1.5"
			role="button"
			tabindex="0"
			aria-label="Open the dial"
			onpointerdown={down}
			{@attach holdStill}
		/>
		<text x={center.x} y={center.y} text-anchor="middle" dominant-baseline="central" class="label pointer-events-none text-[9.5px]" style:fill={open ? 'var(--accent-fg)' : 'var(--bg)'}>
			{here}
		</text>
	</svg>
</div>
