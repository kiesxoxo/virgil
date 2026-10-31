import type { Attachment } from 'svelte/attachments';

/**
 * Tilts the element towards the pointer (mouse or pen), or with the phone where
 * orientation needs no permission (Android; iOS would ask, so it stays flat). Sets
 * `--tilt-x`/`--tilt-y` (degrees) and `--sheen-x`/`--sheen-y` (where the light falls)
 * for the element's CSS to use, and `data-tilting` while it is tilted.
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
		// Held at about 45° is level; a 30° turn either way tilts all the way.
		const onorientation = (e: DeviceOrientationEvent) => {
			if (e.beta === null || e.gamma === null) return;
			set(clamp(e.gamma / 30), clamp((e.beta - 45) / 30));
		};
		// Safari has no DeviceOrientationEvent outside a secure context (plain http on the LAN).
		const sensor = window.DeviceOrientationEvent as unknown as { requestPermission?: unknown } | undefined;
		const phone = matchMedia('(pointer: coarse)').matches && !!sensor && typeof sensor.requestPermission !== 'function';

		node.addEventListener('pointermove', onmove);
		node.addEventListener('pointerleave', reset);
		if (phone) addEventListener('deviceorientation', onorientation);
		return () => {
			node.removeEventListener('pointermove', onmove);
			node.removeEventListener('pointerleave', reset);
			removeEventListener('deviceorientation', onorientation);
			reset();
		};
	};
}
