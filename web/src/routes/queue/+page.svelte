<script lang="ts">
	import { Radio as RadioIcon } from '@lucide/svelte';
	import Cover from '#lib/components/Cover.svelte';
	import Equalizer from '#lib/components/Equalizer.svelte';
	import Masthead from '#lib/components/Masthead.svelte';
	import Menu from '#lib/components/Menu.svelte';
	import { queueHeader, queueItems } from '#lib/actions.ts';
	import { api } from '#lib/api.ts';
	import { Confirm } from '#lib/confirm.svelte.ts';
	import { clock, plural, span } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { contextMenu } from '#lib/menu.svelte.ts';
	import { isStream, radio } from '#lib/radio.svelte.ts';
	import { toasts } from '#lib/toast.svelte.ts';
	import type { QueueItem } from '#lib/types/QueueItem.ts';

	let items = $derived(hub.queue.items);
	let cur = $derived(hub.currentIndex);
	let upNext = $derived(cur >= 0 ? items.slice(cur + 1) : items);
	let played = $derived(cur > 0 ? items.slice(0, cur) : []);
	let remaining = $derived(upNext.reduce((sum, i) => sum + i.duration, 0));

	const clearing = new Confirm();
	const clear = () => api.clear().then(() => toasts.done(hub.current ? 'Queue cleared; this song keeps playing' : 'Queue cleared'));

	const actions = (item: QueueItem) => queueItems(item, items.indexOf(item), cur);
	/** A song's artist and album; a station's show and channel. */
	const subtitle = (item: QueueItem) => (isStream(item.uri) ? radio.show(item).subtitle : [item.artist, item.album].filter(Boolean).join(' · '));
</script>

<svelte:head><title>Queue · Virgil</title></svelte:head>

{#snippet row(item: QueueItem, dim: boolean)}
	{@const shown = radio.show(item)}
	<li
		class="flex items-stretch border-b border-line hover:bg-surface [contain-intrinsic-size:auto_56px] [content-visibility:auto] {dim ? 'opacity-55 hover:opacity-100' : ''}"
		{@attach contextMenu(
			() => actions(item),
			() => queueHeader(item)
		)}
	>
		<button type="button" class="press flex min-w-0 flex-1 items-center gap-3 text-left" onclick={() => api.playItem(item.id)}>
			<Cover art={shown.art} src={shown.src} class="size-14 shrink-0" />
			<span class="min-w-0 flex-1">
				<span class="wide block truncate text-[13px] leading-[18px]">{shown.title}</span>
				<span class="block truncate text-xs leading-4 text-muted">{subtitle(item)}</span>
			</span>
			{#if isStream(item.uri)}
				<RadioIcon class="size-4 text-muted" aria-label="Radio" />
			{:else}
				<span class="label text-[11px] text-muted">{clock(item.duration)}</span>
			{/if}
		</button>
		<Menu items={() => actions(item)} header={queueHeader(item)} />
	</li>
{/snippet}

<div class="pt-safe">
	<Masthead word="QUEUE" />

	{#if items.length === 0}
		<div class="border-t border-line px-3 pt-8">
			<h2 class="wide text-2xl">Nothing queued</h2>
			<p class="mt-2 text-sm text-muted">Play an album, or add songs with “Play next”.</p>
			<a href="/" class="press wide mt-5 flex h-14 items-center justify-center bg-accent text-[15px] text-accent-fg uppercase">Browse albums</a>
		</div>
	{:else}
		<div class="flex h-11 items-stretch border-y border-line">
			<p class="label flex min-w-0 flex-1 items-center px-3 text-[10.5px] whitespace-nowrap text-muted">
				{cur >= 0 ? `${upNext.length} up next` : plural(items.length, 'song')}{#if remaining > 0}&nbsp;/ {span(remaining)}{/if}
			</p>
			<button
				type="button"
				class="label border-l border-line px-3 text-[10.5px] whitespace-nowrap {hub.player.consume ? 'press-lit bg-fg text-bg' : 'press text-muted'}"
				aria-pressed={hub.player.consume}
				title="Remove songs from the queue once played"
				onclick={() => api.modes({ consume: !hub.player.consume })}
			>
				Auto-remove
			</button>
			<button
				type="button"
				class="label border-l border-line px-3 text-[10.5px] whitespace-nowrap {clearing.armed ? 'press bg-accent text-accent-fg' : 'press text-muted'}"
				title={hub.current ? 'Remove everything except the song that is playing' : 'Remove everything'}
				onclick={() => clearing.press(clear)}
			>
				{clearing.armed ? 'Confirm' : 'Clear'}
			</button>
		</div>

		{#if hub.current}
			{@const item = hub.current}
			{@const shown = radio.show(item)}
			<h2 class="label flex h-9 items-center border-b border-line px-3 text-[11px] text-accent">Now playing</h2>
			<a href="/now" class="flex items-center gap-3 border-b border-line pr-3">
				<Cover art={shown.art} src={shown.src} class="size-20 shrink-0" />
				<span class="min-w-0 flex-1">
					<span class="wide line-clamp-2 text-[15px] leading-[18px]">{shown.title}</span>
					<span class="mt-0.5 block truncate text-xs leading-4 text-muted">{subtitle(item)}</span>
				</span>
				<Equalizer playing={hub.playing} class="text-accent" />
			</a>
		{/if}

		{#if upNext.length}
			<h2 class="label flex h-9 items-center border-b border-line px-3 text-[11px] text-muted">{cur >= 0 ? 'Up next' : 'Queue'}</h2>
			<ul>
				{#each upNext as item (item.id)}
					{@render row(item, false)}
				{/each}
			</ul>
		{/if}

		{#if played.length}
			<h2 class="label flex h-9 items-center border-b border-line px-3 text-[11px] text-muted">Played</h2>
			<ul>
				{#each played as item (item.id)}
					{@render row(item, true)}
				{/each}
			</ul>
		{/if}
	{/if}
</div>
