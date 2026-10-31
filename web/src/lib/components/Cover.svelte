<script lang="ts">
	import { Disc3 } from '@lucide/svelte';
	import { TILE, artUrl, type ArtSize } from '../format.ts';

	let {
		art,
		src: image = null,
		size = 300,
		class: className = '',
		eager = false,
		name
	}: {
		art: string | null | undefined;
		/** A full image URL (radio artwork), used instead of `art`. */
		src?: string | null;
		size?: ArtSize;
		class?: string;
		eager?: boolean;
		/** `view-transition-name`: the same name on two pages morphs one cover into the other. */
		name?: string;
	} = $props();

	let loaded = $state(false);
	let thumbLoaded = $state(false);
	let src = $derived(image ?? artUrl(art, size));
	// The grid's cover is usually cached already: show it until the large one is in (or instead of
	// it, where virgild has no large one yet).
	let thumb = $derived(!image && size > TILE ? artUrl(art, TILE) : null);
</script>

<div class="relative aspect-square overflow-hidden bg-surface-2 {className}" style:view-transition-name={name}>
	{#if thumb && !loaded}
		{#key thumb}
			<img
				src={thumb}
				alt=""
				decoding="async"
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
				loading={eager ? 'eager' : 'lazy'}
				decoding="async"
				draggable="false"
				class="absolute inset-0 size-full object-cover transition-opacity duration-300"
				class:opacity-0={!loaded}
				{@attach (img) => {
					// Already cached: show it at once instead of fading in again.
					if (img.complete && img.naturalWidth) loaded = true;
				}}
				onload={() => (loaded = true)}
				onerror={() => (loaded = false)}
			/>
		{/key}
	{/if}
	{#if !src || (!loaded && !thumbLoaded)}
		<div class="absolute inset-0 grid place-items-center text-faint">
			<Disc3 class="size-2/5" strokeWidth={1.25} />
		</div>
	{/if}
</div>
