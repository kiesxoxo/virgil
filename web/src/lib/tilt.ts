import type { Attachment } from 'svelte/attachments';
import { motion } from './motion.svelte.ts';

/**
 * Tilts the element towards the pointer (mouse or pen), or as the phone moves (lib/motion.svelte.ts).
 * Sets `--tilt-x`/`--tilt-y` (degrees) and `--sheen-x`/`--sheen-y` (where the light falls) for the
 * element's CSS to use, and `data-tilting` while it is tilted.
 */
export function tilt(max = 6): Attachment<HTMLElement> {
	return (node) => {
		if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;

		const set = (x: number, y: number) => {
			// x and y run -1…1 from the centre.
			node.style.setProperty('--tilt-x', `${-y * max}deg`);
			node.style.setProperty('--tilt-y', `${x * max}deg`);
			node.style.setProperty('--sheen-x', `${50 + x * 40}%`);
			node.style.setProperty('--sheen-y', `${50 + y * 40}%`);
			node.dataset.tilting = '';
		};
		const reset = () => {
			for (const name of ['--tilt-x', '--tilt-y', '--sheen-x', '--sheen-y']) node.style.removeProperty(name);
			delete node.dataset.tilting;
		};
		const clamp = (v: number) => Math.max(-1, Math.min(1, v));

		const onmove = (e: PointerEvent) => {
			if (e.pointerType === 'touch') return;
			const r = node.getBoundingClientRect();
			set(clamp(((e.clientX - r.left) / r.width) * 2 - 1), clamp(((e.clientY - r.top) / r.height) * 2 - 1));
		};
		// A finger lifting is no reason to drop the phone's tilt.
		const onleave = (e: PointerEvent) => e.pointerType !== 'touch' && reset();

		node.addEventListener('pointermove', onmove);
		node.addEventListener('pointerleave', onleave);
		const unsubscribe = motion.subscribe((x, y) => (x || y ? set(x, y) : reset()));
		return () => {
			node.removeEventListener('pointermove', onmove);
			node.removeEventListener('pointerleave', onleave);
			unsubscribe();
			reset();
		};
	};
}
