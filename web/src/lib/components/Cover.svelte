<script lang="ts" module>
	import { TILE, artUrl, type ArtSize } from '../format.ts';

	/** Images shown already, so still in the browser's memory: shown again at once rather than faded in over the placeholder. */
	const seen = new Set<string>();
	/** Sharpest first. */
	const SIZES: ArtSize[] = [1000, 600, 300];
</script>

<script lang="ts">
	import { Disc3 } from '@lucide/svelte';
	import { flight } from '../flight.svelte.ts';

	let {
		art,
		src: image = null,
		size = 300,
		class: className = '',
		eager = false,
		slot
	}: {
		art: string | null | undefined;
		/** A full image URL (radio artwork), used instead of `art`. */
		src?: string | null;
		size?: ArtSize;
		class?: string;
		eager?: boolean;
		/**
		 * The same slot on two pages (`album:<id>`, `now`): the cover flies from one to the other as
		 * the page changes (lib/flight.svelte.ts), hidden here while it is in the air.
		 */
		slot?: string;
	} = $props();

	let loaded = $state(false);
	let thumbLoaded = $state(false);
	let src = $derived(image ?? artUrl(art, size));
	// Until it is in (or instead of it, where virgild has no large one yet), another size stands in:
	// one shown before (the list's, on the way to the grid), else the grid's, usually cached already.
	let thumb = $derived.by(() => {
		if (image) return null;
		const before = SIZES.map((other) => artUrl(art, other)).find((url) => url && url !== src && seen.has(url));
		return before ?? (size > TILE ? artUrl(art, TILE) : null);
	});
	/** Shown before (a page coming back, the view switching): up at once, with no placeholder and no fade. */
	let known = $derived(!!src && seen.has(src));
	let shown = $derived(loaded || known);

	function show() {
		loaded = true;
		if (src) seen.add(src);
	}
</script>

<div
	class="relative aspect-square overflow-hidden bg-surface-2 {className}"
	style:view-transition-name={slot && flight.name(slot)}
	style:opacity={slot && flight.hides(slot) ? 0 : undefined}
	style:transition={slot && flight.appearing.has(slot) ? 'opacity 150ms' : undefined}
	{@attach (el) => {
		if (slot) return flight.register(slot, el, () => art ?? image);
	}}
>
	{#if thumb && !shown}
		{#key thumb}
			<img
				src={thumb}
				alt=""
				decoding={seen.has(thumb) ? 'sync' : 'async'}
				draggable="false"
				class="absolute inset-0 size-full object-cover"
				{@attach (img) => {
					if (img.complete && img.naturalWidth) thumbLoaded = true;
				}}
				onload={() => (thumbLoaded = true)}
				onerror={() => (thumbLoaded = false)}
			/>
		{/key}
	{/if}
	{#if src}
		{#key src}
			<img
				{src}
				alt=""
				loading={eager || known ? 'eager' : 'lazy'}
				decoding={known ? 'sync' : 'async'}
				draggable="false"
				class="absolute inset-0 size-full object-cover transition-opacity duration-300"
				class:opacity-0={!shown}
				{@attach (img) => {
					// Already cached: show it at once instead of fading in again.
					if (img.complete && img.naturalWidth) show();
				}}
				onload={show}
				onerror={() => (loaded = false)}
			/>
		{/key}
	{/if}
	{#if !src || (!shown && !thumbLoaded)}
		<div class="absolute inset-0 grid place-items-center text-faint">
			<Disc3 class="size-2/5" strokeWidth={1.25} />
		</div>
	{/if}
</div>
