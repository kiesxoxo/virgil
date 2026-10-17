<script lang="ts">
	import { Disc3 } from '@lucide/svelte';
	import { artUrl } from '../format.ts';

	let {
		art,
		size = 300,
		class: className = '',
		eager = false
	}: { art: string | null | undefined; size?: 300 | 600; class?: string; eager?: boolean } = $props();

	let loaded = $state(false);
	let src = $derived(artUrl(art, size));
</script>

<div class="relative aspect-square overflow-hidden bg-surface-2 {className}">
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
	{#if !src || !loaded}
		<div class="absolute inset-0 grid place-items-center text-faint">
			<Disc3 class="size-2/5" strokeWidth={1.25} />
		</div>
	{/if}
</div>
