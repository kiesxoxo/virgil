<script lang="ts">
	import { ChevronLeft, CircleCheck, Circle, Layers, ListChecks, ListEnd, ListPlus, Play, Tags } from '@lucide/svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import Cover from '#lib/components/Cover.svelte';
	import Equalizer from '#lib/components/Equalizer.svelte';
	import Menu from '#lib/components/Menu.svelte';
	import { trackHeader, trackItems } from '#lib/actions.ts';
	import { api, queue } from '#lib/api.ts';
	import { contextMenu } from '#lib/menu.svelte.ts';
	import { artUrl, clock, formatLong, formatShort, isHiRes, span } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { library } from '#lib/library.svelte.ts';
	import { tagEditor } from '#lib/tageditor.svelte.ts';
	import type { AlbumDetail } from '#lib/types/AlbumDetail.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	let detail = $state<AlbumDetail | null>(null);
	let missing = $state(false);

	$effect(() => {
		const pending = data.detail;
		detail = null;
		missing = false;
		pending.then(
			(d) => {
				if (data.detail === pending) detail = d;
			},
			() => {
				if (data.detail === pending) missing = true;
			}
		);
	});

	// Refetch the tracks when the library changes (e.g. after a tag edit),
	// keeping the current list on screen until the new one arrives.
	let seenRevision = hub.library.revision;
	$effect(() => {
		const revision = hub.library.revision;
		if (revision === seenRevision) return;
		seenRevision = revision;
		const id = data.id;
		api.album(id).then(
			(d) => {
				if (data.id === id) detail = d;
			},
			() => {}
		);
	});

	let album = $derived(detail?.album ?? library.get(data.id));
	let versions = $derived(library.versions(data.id));
	let tracks = $derived(detail?.tracks ?? []);
	let multiDisc = $derived(new Set(tracks.map((t) => t.disc ?? 1)).size > 1);
	let backdrop = $derived(artUrl(album?.art, 300));
	// Picking tracks to edit, like a selection in foobar2000's playlist.
	let selecting = $state(false);
	const selected = new SvelteSet<string>();
	let anchor = 0;

	$effect(() => {
		void data.id;
		selecting = false;
		selected.clear();
	});

	/** Click toggles; Shift-click selects the range from the last click. */
	function pick(i: number, range: boolean) {
		const uri = tracks[i].uri;
		if (range) {
			const [from, to] = anchor < i ? [anchor, i] : [i, anchor];
			for (const t of tracks.slice(from, to + 1)) selected.add(t.uri);
		} else if (selected.has(uri)) {
			selected.delete(uri);
		} else {
			selected.add(uri);
		}
		anchor = i;
	}

	function trackClick(e: MouseEvent, i: number) {
		if (!selecting && (e.shiftKey || e.metaKey || e.ctrlKey)) selecting = true;
		if (selecting) {
			e.preventDefault();
			pick(i, e.shiftKey);
		} else if (album) {
			queue.playAlbum(album.id, tracks[i].uri);
		}
	}

	function stopSelecting() {
		selecting = false;
		selected.clear();
	}

	function editSelected() {
		const uris = tracks.filter((t) => selected.has(t.uri)).map((t) => t.uri);
		tagEditor.show(uris, uris.length === 1 ? (tracks.find((t) => t.uri === uris[0])?.title ?? '') : (album?.title ?? ''));
	}

	let meta = $derived(
		album
			? [album.genre, album.year, `${album.tracks} ${album.tracks === 1 ? 'song' : 'songs'}`, span(album.duration)].filter(Boolean)
			: []
	);
</script>

<svelte:head><title>{album ? `${album.title} · ${album.artist}` : 'Album'} · Virgil</title></svelte:head>

{#if missing && !album}
	<div class="mx-auto mt-24 max-w-sm px-4 text-center">
		<h1 class="text-lg font-semibold">Album not found</h1>
		<p class="mt-1 text-sm text-muted">It may have been removed in the last rescan.</p>
		<a href="/" class="mt-4 inline-block text-sm font-semibold text-accent">Back to albums</a>
	</div>
{:else}
	<!-- clip, not hidden: it can't be scrolled sideways by focus, and the selection bar can stick. -->
	<div class="relative isolate overflow-clip">
		{#if backdrop}
			<img
				src={backdrop}
				alt=""
				aria-hidden="true"
				class="absolute inset-x-0 top-0 -z-10 h-[30rem] w-full scale-125 object-cover opacity-40 blur-3xl saturate-150 dark:opacity-30"
				style="mask-image: linear-gradient(to bottom, black 20%, transparent); -webkit-mask-image: linear-gradient(to bottom, black 20%, transparent)"
			/>
		{/if}

		<div class="pt-safe mx-auto max-w-6xl px-4 lg:px-8">
			<a href="/" class="-ml-1.5 inline-flex h-11 items-center gap-0.5 text-[15px] font-medium text-accent lg:hidden">
				<ChevronLeft class="size-5" /> Albums
			</a>

			<header class="flex flex-col items-center gap-6 pt-2 text-center md:flex-row md:items-end md:gap-8 md:pt-10 md:text-left">
				<Cover art={album?.art} size={600} eager class="w-[min(68vw,18rem)] shrink-0 rounded-2xl shadow-cover ring-1 ring-line md:w-60" />
				<div class="min-w-0 flex-1">
					<h1 class="text-2xl font-bold tracking-tight text-balance md:text-4xl">{album?.title ?? ''}</h1>
					<p class="mt-1 text-lg font-semibold text-accent md:text-xl">
						{#if album}<a href="/artist/{encodeURIComponent(album.artist)}" class="hover:underline">{album.artist}</a>{/if}
					</p>
					<p class="mt-1.5 text-sm text-muted">{meta.join(' · ')}</p>
					{#if album?.format}
						<p class="mt-2.5 flex items-center justify-center gap-2 md:justify-start">
							{#if isHiRes(album.format)}
								<span class="rounded-md bg-fg px-1.5 py-0.5 text-[10px] font-bold tracking-wider text-bg">HI-RES</span>
							{/if}
							<span class="text-xs font-medium text-muted tabular-nums" title={formatLong(album.format)}>{formatLong(album.format)}</span>
						</p>
					{/if}
					{#if versions.length > 1}
						<div class="mt-3 flex flex-wrap items-center justify-center gap-1.5 md:justify-start" role="radiogroup" aria-label="Versions">
							<span class="mr-1 inline-flex items-center gap-1 text-xs font-semibold text-muted"><Layers class="size-3.5" /> Versions</span>
							{#each versions as version (version.id)}
								{@const active = version.id === data.id}
								<a
									href="/album/{version.id}"
									role="radio"
									aria-checked={active}
									title={version.title}
									class="press inline-flex h-8 items-center gap-1 rounded-full px-3 text-xs font-semibold whitespace-nowrap tabular-nums {active
										? 'bg-fg text-bg'
										: 'bg-surface text-muted hover:text-fg'}"
								>
									{version.format ? formatShort(version.format) : 'Mixed'}
									<span class="font-medium opacity-70">· {version.tracks} {version.tracks === 1 ? 'track' : 'tracks'}</span>
								</a>
							{/each}
						</div>
					{/if}
					<div class="mt-5 flex items-center justify-center gap-2.5 md:justify-start">
						<button
							type="button"
							class="press inline-flex h-11 flex-1 items-center justify-center gap-2 rounded-full bg-accent px-7 text-[15px] font-semibold text-accent-fg shadow-float sm:flex-none"
							disabled={!album}
							onclick={() => album && queue.playAlbum(album.id)}
						>
							<Play class="size-[18px]" fill="currentColor" strokeWidth={0} /> Play
						</button>
						<button
							type="button"
							class="press inline-flex h-11 items-center gap-2 rounded-full bg-surface px-4 text-sm font-semibold hover:bg-surface-2"
							disabled={!album}
							onclick={() => album && queue.albumNext(album.id)}
						>
							<ListPlus class="size-[18px]" /> <span class="max-sm:hidden">Play next</span>
						</button>
						<button
							type="button"
							class="press inline-flex h-11 items-center gap-2 rounded-full bg-surface px-4 text-sm font-semibold hover:bg-surface-2"
							disabled={!album}
							onclick={() => album && queue.albumLast(album.id)}
						>
							<ListEnd class="size-[18px]" /> <span class="max-sm:hidden">Add to queue</span>
						</button>
						<button
							type="button"
							class="press inline-flex h-11 items-center gap-2 rounded-full px-4 text-sm font-semibold {selecting
								? 'bg-fg text-bg'
								: 'bg-surface hover:bg-surface-2'}"
							disabled={!detail}
							aria-pressed={selecting}
							title="Select tracks to edit their tags"
							onclick={() => (selecting ? stopSelecting() : (selecting = true))}
						>
							<ListChecks class="size-[18px]" /> <span class="max-sm:hidden">Select</span>
						</button>
					</div>
				</div>
			</header>

			{#if selecting}
				<div class="glass pt-safe sticky top-0 z-10 -mx-4 mt-6 flex items-center gap-2 border-b border-line px-4 py-2 lg:-mx-8 lg:px-8">
					<span class="flex-1 text-sm font-semibold tabular-nums">{selected.size} selected</span>
					<button
						type="button"
						class="press h-9 rounded-full px-3 text-[13px] font-semibold text-accent"
						onclick={() => (selected.size === tracks.length ? selected.clear() : tracks.forEach((t) => selected.add(t.uri)))}
					>
						{selected.size === tracks.length ? 'None' : 'All'}
					</button>
					<button
						type="button"
						class="press inline-flex h-9 items-center gap-1.5 rounded-full bg-accent px-4 text-[13px] font-semibold text-accent-fg disabled:opacity-40"
						disabled={!selected.size}
						onclick={editSelected}
					>
						<Tags class="size-4" /> Edit tags
					</button>
					<button type="button" class="press h-9 rounded-full bg-surface px-3.5 text-[13px] font-semibold" onclick={stopSelecting}>Done</button>
				</div>
			{/if}

			<ol class={selecting ? "mt-2" : "mt-8 md:mt-10"}>
				{#if !detail}
					{#each { length: Math.min(album?.tracks ?? 8, 12) } as _, i (i)}
						<li class="flex h-14 animate-pulse items-center gap-4 px-3">
							<div class="h-3 w-4 rounded bg-surface"></div>
							<div class="h-3 flex-1 rounded bg-surface"></div>
						</li>
					{/each}
				{/if}
				{#each tracks as track, i (track.uri)}
					{@const current = hub.current?.uri === track.uri}
					{@const picked = selecting && selected.has(track.uri)}
					{#if multiDisc && (i === 0 || tracks[i - 1].disc !== track.disc)}
						<li class="px-3 pt-5 pb-1.5 text-xs font-semibold tracking-wider text-muted uppercase">Disc {track.disc ?? 1}</li>
					{/if}
					<li
						class="group flex items-center rounded-xl hover:bg-fg/4 {picked ? 'bg-accent/10' : current ? 'bg-accent/6' : ''}"
						{@attach contextMenu(
							() => trackItems(track, data.id),
							() => trackHeader(track, album?.art)
						)}
					>
						<button
							type="button"
							class="grid min-w-0 flex-1 grid-cols-[2rem_1fr_auto] items-center gap-3 py-2.5 pl-3 text-left select-none"
							aria-pressed={selecting ? picked : undefined}
							onclick={(e) => trackClick(e, i)}
						>
							<span class="flex justify-center text-sm text-muted tabular-nums">
								{#if selecting}
									{#if picked}<CircleCheck class="size-5 text-accent" />{:else}<Circle class="size-5 text-faint" />{/if}
								{:else if current}<Equalizer playing={hub.playing} />{:else}{track.track ?? i + 1}{/if}
							</span>
							<span class="min-w-0">
								<span class="block truncate text-[15px] font-medium {current ? 'text-accent' : ''}">{track.title}</span>
								{#if album && track.artist && track.artist !== album.artist}
									<span class="block truncate text-[13px] text-muted">{track.artist}</span>
								{/if}
							</span>
							<span class="flex items-center gap-3 text-sm text-muted tabular-nums">
								{#if track.format && !album?.format}
									<span class="text-[11px] max-sm:hidden">{formatShort(track.format)}</span>
								{/if}
								{clock(track.duration)}
							</span>
						</button>
						<Menu class="mr-1 ml-1" items={() => trackItems(track, data.id)} header={trackHeader(track, album?.art)} />
					</li>
				{/each}
			</ol>
		</div>
	</div>
{/if}
