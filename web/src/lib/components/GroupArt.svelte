<script lang="ts">
	import { covers, type Group } from '../browse.ts';
	import Cover from './Cover.svelte';

	// An artist: one round cover. A genre: a 2×2 mosaic of its albums (one
	// cover when it has fewer than four with art).
	let { group, kind, size = 300, class: className = '' }: { group: Group; kind: 'artist' | 'genre'; size?: 300 | 600; class?: string } =
		$props();

	let arts = $derived(covers(group));
	let mosaic = $derived(kind === 'genre' && arts.filter(Boolean).length >= 4);
</script>

{#if mosaic}
	<div class="grid aspect-square grid-cols-2 overflow-hidden {className}">
		{#each arts as art, i (i)}
			<Cover {art} />
		{/each}
	</div>
{:else}
	<Cover art={arts[0]} {size} class={className} />
{/if}
