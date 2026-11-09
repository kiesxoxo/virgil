<script lang="ts">
	// An album from Lidarr's catalogue: download it whole, or (with slskdpp) one song of it.
	import { ArrowDownToLine, LoaderCircle } from '@lucide/svelte';
	import { api } from '../api.ts';
	import { downloads } from '../downloads.svelte.ts';
	import { badge, describe, finished, retryable } from '../fetching.ts';
	import { clock } from '../format.ts';
	import type { CatalogAlbum } from '../types/CatalogAlbum.ts';
	import type { CatalogTrack } from '../types/CatalogTrack.ts';
	import Cover from './Cover.svelte';
	import Sheet from './Sheet.svelte';

	let album = $derived(downloads.sheet);
	let job = $derived(album ? downloads.of(album.id) : undefined);
	let active = $derived(job && !finished(job) ? job : null);
	let complete = $derived(!!album && album.total > 0 && album.have >= album.total);
	let reading = $state(false);
	let starting = $state(false);

	// What Lidarr has of it, with its tracks (only once Lidarr has the album).
	let fetched = '';
	$effect(() => {
		const id = album?.id;
		// Closed: next time it opens, ask again (a download may have come in meanwhile).
		if (!id) {
			fetched = '';
			return;
		}
		if (id === fetched) return;
		fetched = id;
		api.catalogAlbum(id).then(
			(a) => {
				if (downloads.sheet?.id === id) downloads.sheet = a;
			},
			() => {}
		);
	});

	/** Lidarr reads the tracklist once it has the album: adds it (wanting nothing) and asks again until it is in. */
	async function readTracks() {
		if (!album) return;
		const id = album.id;
		reading = true;
		try {
			for (let tries = 0; tries < 15; tries++) {
				const a = await api.catalogTracks(id);
				if (downloads.sheet?.id !== id) return;
				downloads.sheet = a;
				if (a.tracks.length) return;
				await new Promise((r) => setTimeout(r, 2000));
			}
		} finally {
			reading = false;
		}
	}

	async function getAlbum() {
		if (!album) return;
		starting = true;
		try {
			await downloads.album(album.id);
		} finally {
			starting = false;
		}
	}

	const songJob = (track: CatalogTrack) => (album ? downloads.of(album.id, track.id) : undefined);
</script>

{#if album}
	<Sheet onclose={() => downloads.close()} label="Download {album.title}">
		<div class="flex gap-3 border-b border-line p-3">
			<Cover art={null} src={album.cover} class="size-20 shrink-0" />
			<div class="min-w-0 flex-1">
				<p class="label text-[10.5px] text-muted">{[album.kind, album.year].filter(Boolean).join(' · ')}</p>
				<h2 class="wide mt-1 text-[20px] leading-tight text-balance">{album.title}</h2>
				<p class="truncate text-sm text-muted">{album.artist}</p>
				{#if album.total}
					<p class="label mt-1 text-[10.5px] text-muted">
						{complete ? 'In the library' : album.have ? `${album.have} of ${album.total} songs in the library` : `${album.total} songs`}
					</p>
				{/if}
			</div>
		</div>

		{#if complete && album.albumId}
			<a href="/album/{album.albumId}" onclick={() => downloads.close()} class="press wide flex h-14 items-center justify-center bg-accent text-[15px] text-accent-fg uppercase">
				Open in library
			</a>
		{:else if active}
			<div class="border-b border-line px-3 py-2.5">
				<div class="label text-[11px] text-accent">{badge(active)}</div>
				<div class="mt-0.5 text-[13px] text-muted">{describe(active)}</div>
				{#if active.progress != null}
					<div class="mt-2 h-[3px] bg-surface-2"><div class="h-full bg-accent" style:width="{active.progress * 100}%"></div></div>
				{/if}
			</div>
		{:else}
			{#if job && retryable(job)}
				<p class="border-b border-line px-3 py-2 text-[13px] text-muted">{describe(job)}</p>
			{/if}
			<button
				type="button"
				class="press wide flex h-14 w-full items-center justify-center gap-2.5 bg-accent text-[15px] text-accent-fg uppercase disabled:bg-surface-2 disabled:text-faint"
				disabled={starting}
				onclick={getAlbum}
			>
				<ArrowDownToLine class="size-4" strokeWidth={2.5} />
				{album.have ? 'Download the rest' : 'Download album'}
			</button>
		{/if}

		{#if downloads.songs && !complete}
			{#if album.tracks.length}
				<h3 class="label flex h-9 items-center border-b border-line bg-surface px-3 text-[11px] text-muted">Or just one song</h3>
				<ol>
					{#each album.tracks as track (track.id)}
						{@const song = songJob(track)}
						{@const busy = song && !finished(song)}
						<li class="flex min-h-12 items-stretch border-b border-line">
							<span class="label grid w-11 shrink-0 place-items-center text-[11px] text-muted">{String(track.number).padStart(2, '0')}</span>
							<span class="min-w-0 flex-1 self-center py-2">
								<span class="wide block text-[13px] leading-[17px] {track.have ? 'text-muted' : ''}">{track.title}</span>
								{#if song && !track.have}<span class="label block text-[10px] {busy ? 'text-accent' : 'text-muted'}">{describe(song)}</span>{/if}
							</span>
							<span class="label self-center px-3 text-[11px] text-muted">{clock(track.duration)}</span>
							{#if track.have}
								<span class="label grid w-14 shrink-0 place-items-center border-l border-line text-[10px] text-muted">In</span>
							{:else if busy}
								<span class="label grid w-14 shrink-0 place-items-center border-l border-line text-[10px] text-accent">{badge(song)}</span>
							{:else}
								<button
									type="button"
									class="press grid w-14 shrink-0 place-items-center border-l border-line"
									title="Download {track.title}"
									aria-label="Download {track.title}"
									onclick={() => album && downloads.song(album.id, track.id)}
								>
									<ArrowDownToLine class="size-[18px]" />
								</button>
							{/if}
						</li>
					{/each}
				</ol>
			{:else}
				<button type="button" class="press flex min-h-14 w-full flex-col items-center justify-center gap-0.5 border-b border-line px-3 py-2 disabled:text-faint" disabled={reading} onclick={readTracks}>
					<span class="label flex items-center gap-2 text-[11px]">
						{#if reading}<LoaderCircle class="size-3.5" /> Lidarr is reading the tracklist{:else}Choose one song{/if}
					</span>
					{#if !album.total}<span class="text-xs text-muted">Lidarr adds the album, wanting nothing, to read its tracks</span>{/if}
				</button>
			{/if}
		{/if}

		<button type="button" class="press label h-11 w-full text-[11px] text-muted" onclick={() => downloads.close()}>Close</button>
	</Sheet>
{/if}
