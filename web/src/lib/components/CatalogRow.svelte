<script lang="ts">
	// An album from Lidarr's catalogue, with how much of it the library has or how its download is going.
	import { downloads } from '../downloads.svelte.ts';
	import { badge, finished } from '../fetching.ts';
	import type { CatalogAlbum } from '../types/CatalogAlbum.ts';
	import Cover from './Cover.svelte';

	let { album, onclick }: { album: CatalogAlbum; onclick: () => void } = $props();

	let job = $derived(downloads.of(album.id));
	let active = $derived(job && !finished(job) ? job : null);
	let complete = $derived(album.total > 0 && album.have >= album.total);
	let status = $derived.by(() => {
		if (active) return badge(active);
		if (complete) return 'In library';
		if (album.have) return `${album.have} of ${album.total}`;
		return '';
	});
</script>

<button type="button" class="press flex w-full items-center gap-3 pr-3 text-left" {onclick}>
	<Cover art={null} src={album.cover} class="size-14 shrink-0" />
	<div class="min-w-0 flex-1">
		<div class="wide truncate text-[13px] leading-[18px]">{album.title}</div>
		<div class="truncate text-xs leading-4 text-muted">{[album.artist, album.year, album.kind].filter(Boolean).join(' · ')}</div>
	</div>
	{#if status}
		<span class="label shrink-0 text-[11px] {active ? 'text-accent' : 'text-muted'}">{status}</span>
	{/if}
</button>
