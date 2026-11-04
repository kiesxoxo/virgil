<script lang="ts">
	import { Disc3 } from '@lucide/svelte';
	import { isDecoded, markDecoded, standIn } from '../artwork.ts';
	import { TILE, artUrl, type ArtSize } from '../format.ts';

	let {
		art,
		src: image = null,
		size = 300,
		class: className = '',
		eager = false
	}: {
		art: string | null | undefined;
		/** A full image URL (radio artwork), used instead of `art`. */
		src?: string | null;
		size?: ArtSize;
		class?: string;
		eager?: boolean;
	} = $props();

	/** The image that has come in and is ready to show, and the one standing in. */
	let loaded = $state<string | null>(null);
	let thumbLoaded = $state<string | null>(null);
	let src = $derived(image ?? artUrl(art, size));
	// Until it is in (or instead of it, where virgild has no large one yet), another size stands in:
	// the sharpest decoded lately (lib/artwork.ts), else the grid's, usually cached already.
	let thumb = $derived.by(() => {
		if (image) return null;
		return standIn(art, src) ?? (size > TILE ? artUrl(art, TILE) : null);
	});
	/** Decoded lately (a page coming back, the view switching): up at once, with no placeholder. */
	let known = $derived(isDecoded(src));
	let shown = $derived((!!src && loaded === src) || known);
	/** Something stands in, in place of the placeholder. */
	let covered = $derived(!!thumb && thumbLoaded === thumb);

	/** `url` came in, in `img`: ready to show once decoded, so that swapping it in for a stand-in leaves no frame with neither. */
	function decoded(img: EventTarget, url: string | null, then: (url: string) => void) {
		if (!url) return;
		(img as HTMLImageElement).decode().then(
			() => {
				markDecoded(url);
				then(url);
			},
			// It loaded: show it however it decodes.
			() => then(url)
		);
	}

	/** `img` is in already (cached): shown at once. */
	function cached(img: HTMLImageElement, url: string | null, then: (url: string) => void) {
		if (!url || !img.complete || !img.naturalWidth) return;
		markDecoded(url);
		then(url);
	}
</script>

<div class="relative aspect-square overflow-hidden bg-surface-2 {className}">
	{#if thumb && !shown}
		{#key thumb}
			<img
				src={thumb}
				alt=""
				decoding={isDecoded(thumb) ? 'sync' : 'async'}
				draggable="false"
				class="absolute inset-0 size-full object-cover"
				{@attach (img) => cached(img, thumb, (url) => (thumbLoaded = url))}
				onload={(e) => decoded(e.currentTarget, thumb, (url) => (thumbLoaded = url))}
				onerror={() => (thumbLoaded = null)}
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
				class="absolute inset-0 size-full object-cover"
				class:opacity-0={!shown}
				{@attach (img) => cached(img, src, (url) => (loaded = url))}
				onload={(e) => decoded(e.currentTarget, src, (url) => (loaded = url))}
				onerror={() => (loaded = null)}
			/>
		{/key}
	{/if}
	{#if !src || (!shown && !covered)}
		<div class="absolute inset-0 grid place-items-center text-faint">
			<Disc3 class="size-2/5" strokeWidth={1.25} />
		</div>
	{/if}
</div>
