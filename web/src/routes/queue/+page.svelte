<script lang="ts">
	import { ListMusic, Trash } from '@lucide/svelte';
	import Cover from '#lib/components/Cover.svelte';
	import Equalizer from '#lib/components/Equalizer.svelte';
	import Menu from '#lib/components/Menu.svelte';
	import { queueHeader, queueItems } from '#lib/actions.ts';
	import { api } from '#lib/api.ts';
	import { clock, span } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { contextMenu } from '#lib/menu.svelte.ts';
	import { toasts } from '#lib/toast.svelte.ts';
	import type { QueueItem } from '#lib/types/QueueItem.ts';

	let items = $derived(hub.queue.items);
	let cur = $derived(hub.currentIndex);
	let upNext = $derived(cur >= 0 ? items.slice(cur + 1) : items);
	let played = $derived(cur > 0 ? items.slice(0, cur) : []);
	let remaining = $derived(upNext.reduce((sum, i) => sum + i.duration, 0));

	let confirmClear = $state(false);
	let confirmTimer: ReturnType<typeof setTimeout> | undefined;

	function clear() {
		if (!confirmClear) {
			confirmClear = true;
			clearTimeout(confirmTimer);
			confirmTimer = setTimeout(() => (confirmClear = false), 3000);
			return;
		}
		confirmClear = false;
		api.clear().then(() => toasts.done(hub.current ? 'Queue cleared; this song keeps playing' : 'Queue cleared'));
	}

	const actions = (item: QueueItem) => queueItems(item, items.indexOf(item), cur);
</script>

<svelte:head><title>Queue · Virgil</title></svelte:head>

{#snippet row(item: QueueItem, dim: boolean)}
	<li
		class="group flex items-center rounded-xl hover:bg-fg/4 [contain-intrinsic-size:auto_60px] [content-visibility:auto] {dim
			? 'opacity-55 hover:opacity-100'
			: ''}"
		{@attach contextMenu(
			() => actions(item),
			() => queueHeader(item)
		)}
	>
		<button type="button" class="flex min-w-0 flex-1 items-center gap-3 py-2 pl-2 text-left" onclick={() => api.playItem(item.id)}>
			<Cover art={item.art} class="size-11 shrink-0 rounded-md" />
			<span class="min-w-0 flex-1">
				<span class="block truncate text-[15px] font-medium">{item.title}</span>
				<span class="block truncate text-[13px] text-muted">{[item.artist, item.album].filter(Boolean).join(' · ')}</span>
			</span>
			<span class="text-xs text-faint tabular-nums">{clock(item.duration)}</span>
		</button>
		<Menu class="mx-1" items={() => actions(item)} header={queueHeader(item)} />
	</li>
{/snippet}

<div class="pt-safe mx-auto max-w-3xl px-4 pt-4 lg:px-8 lg:pt-8">
	<div class="flex flex-wrap items-end justify-between gap-x-4 gap-y-3">
		<div>
			<h1 class="text-[28px] font-bold tracking-tight lg:text-[32px]">Queue</h1>
			{#if items.length}
				<p class="mt-0.5 text-sm whitespace-nowrap text-muted">
					{items.length} {items.length === 1 ? 'song' : 'songs'}{#if remaining > 0}&ensp;·&ensp;{span(remaining)} left{/if}
				</p>
			{/if}
		</div>
		{#if items.length}
			<div class="flex gap-2">
				<button
					type="button"
					class="press h-9 rounded-full px-3.5 text-[13px] font-semibold whitespace-nowrap {hub.player.consume
						? 'bg-fg text-bg'
						: 'bg-surface text-muted hover:text-fg'}"
					aria-pressed={hub.player.consume}
					title="Remove songs from the queue once played"
					onclick={() => api.modes({ consume: !hub.player.consume })}
				>
					Auto-remove
				</button>
				<button
					type="button"
					class="press inline-flex h-9 items-center gap-1.5 rounded-full px-3.5 text-[13px] font-semibold whitespace-nowrap {confirmClear
						? 'bg-accent text-accent-fg'
						: 'bg-surface text-muted hover:text-fg'}"
					title={hub.current ? 'Remove everything except the song that is playing' : 'Remove everything'}
					onclick={clear}
				>
					<Trash class="size-3.5" />
					{confirmClear ? 'Tap to confirm' : 'Clear'}
				</button>
			</div>
		{/if}
	</div>

	{#if items.length === 0}
		<div class="mx-auto mt-20 max-w-sm text-center">
			<div class="mx-auto grid size-16 place-items-center rounded-2xl bg-surface text-muted"><ListMusic class="size-7" /></div>
			<h2 class="mt-5 text-lg font-semibold">Nothing queued</h2>
			<p class="mt-1.5 text-sm text-muted">Play an album, or add songs with “Play next”.</p>
			<a href="/" class="press mt-5 inline-flex h-10 items-center rounded-full bg-accent px-5 text-sm font-semibold text-accent-fg">Browse albums</a>
		</div>
	{:else}
		{#if hub.current}
			{@const item = hub.current}
			<h2 class="mt-7 mb-2 text-xs font-semibold tracking-wider text-muted uppercase">Now playing</h2>
			<a href="/now" class="flex items-center gap-4 rounded-2xl bg-surface p-3">
				<Cover art={item.art} class="size-16 shrink-0 rounded-lg shadow-cover" />
				<span class="min-w-0 flex-1">
					<span class="block truncate font-semibold">{item.title}</span>
					<span class="block truncate text-sm text-muted">{[item.artist, item.album].filter(Boolean).join(' · ')}</span>
				</span>
				<Equalizer playing={hub.playing} class="mr-2" />
			</a>
		{/if}

		{#if upNext.length}
			<h2 class="mt-7 mb-1 text-xs font-semibold tracking-wider text-muted uppercase">{cur >= 0 ? 'Up next' : 'Queue'}</h2>
			<ul>
				{#each upNext as item (item.id)}
					{@render row(item, false)}
				{/each}
			</ul>
		{/if}

		{#if played.length}
			<h2 class="mt-7 mb-1 text-xs font-semibold tracking-wider text-muted uppercase">Played</h2>
			<ul>
				{#each played as item (item.id)}
					{@render row(item, true)}
				{/each}
			</ul>
		{/if}
	{/if}
</div>
