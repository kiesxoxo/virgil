<script lang="ts">
	import { Airplay, ListMusic, Maximize2 } from '@lucide/svelte';
	import { api } from '../api.ts';
	import { clock } from '../format.ts';
	import { hub } from '../hub.svelte.ts';
	import { isStream, radio } from '../radio.svelte.ts';
	import { keyboard } from '../keyboard.svelte.ts';
	import { search } from '../search.svelte.ts';
	import Cover from './Cover.svelte';
	import Glyph from './Glyph.svelte';
	import OnAir from './OnAir.svelte';
	import MobileSearch from './MobileSearch.svelte';
	import Scrubber from './Scrubber.svelte';
	import SignalBadge from './SignalBadge.svelte';
	import Transport from './Transport.svelte';

	let item = $derived(hub.current);
	let shown = $derived(item ? radio.show(item) : null);
	let stream = $derived(item ? isStream(item.uri) : false);
	// Radio: how far through the show on air.
	let showProgress = $derived(shown?.station ? radio.progress(shown.station) : null);
	let progress = $derived(
		showProgress !== null ? showProgress * 100 : hub.player.duration > 0 ? (hub.elapsed / hub.player.duration) * 100 : 0
	);
</script>

{#snippet airplayTile(cls: string)}
	<div class="grid shrink-0 place-items-center bg-surface text-info {cls}"><Airplay class="size-1/2" /></div>
{/snippet}

<!-- Phones and tablets: a flat bar on top of the tab bar, with search built in. While the keyboard
     is up, it sits on top of that instead. -->
<div
	class="fixed inset-x-0 z-30 flex h-16 border-t border-line bg-bg lg:hidden"
	style:bottom={keyboard.height > 0 ? `${keyboard.height}px` : 'calc(3.5rem + env(safe-area-inset-bottom))'}
>
	<div class="relative flex min-w-0 flex-1" class:hidden={search.open}>
		{#if hub.external}
			{@render airplayTile('size-16')}
			<div class="min-w-0 flex-1 self-center px-3">
				<div class="wide truncate text-[13px]">AirPlay</div>
				<div class="truncate text-xs text-muted">Another device is playing here</div>
			</div>
			<button type="button" class="press-lit wide shrink-0 bg-fg px-3.5 text-[11px] text-bg uppercase" onclick={() => api.play()}>
				Play here
			</button>
		{:else}
			<a href="/now" class="touch-menu flex min-w-0 flex-1 items-center">
				<Cover art={shown?.art} src={shown?.src} class="size-[62px] shrink-0" />
				<div class="min-w-0 px-3">
					<div class="wide truncate text-[13px] leading-4">{shown?.title ?? 'Not playing'}</div>
					<div class="mt-0.5 truncate text-xs leading-4 text-muted">{shown ? shown.subtitle : 'Pick an album or a station'}</div>
				</div>
			</a>
			<button
				type="button"
				class="press grid w-14 shrink-0 place-items-center border-l border-line disabled:text-faint"
				aria-label={hub.playing ? 'Pause' : 'Play'}
				disabled={!item}
				onclick={() => hub.toggle()}
			>
				<Glyph name={hub.playing ? 'pause' : 'play'} class="size-6" />
			</button>
			<button
				type="button"
				class="press grid w-14 shrink-0 place-items-center border-l border-line disabled:text-faint"
				aria-label="Next"
				disabled={!item}
				onclick={() => api.next()}
			>
				<Glyph name="next" class="size-[22px]" />
			</button>
		{/if}
	</div>
	<MobileSearch />
	{#if !hub.external && !search.open}
		<div class="absolute inset-x-0 -top-[1.5px] h-[3px] overflow-hidden">
			<div class="h-full bg-accent" style:transform="translateX({progress - 100}%)"></div>
		</div>
	{/if}
</div>

<!-- Desktop: a full-width bar in ruled segments. -->
<div class="fixed inset-x-0 bottom-0 z-30 hidden h-[72px] grid-cols-[minmax(0,1fr)_minmax(0,38rem)_minmax(0,1fr)] border-t border-line bg-bg lg:grid">
	<div class="flex min-w-0 items-center">
		{#if hub.external}
			{@render airplayTile('size-[70px]')}
			<div class="min-w-0 px-3">
				<div class="wide truncate text-[13px]">AirPlay</div>
				<div class="truncate text-xs text-muted">Another device is playing here</div>
			</div>
		{:else if item && shown}
			<a href="/now" class="shrink-0"><Cover art={shown.art} src={shown.src} class="size-[70px]" /></a>
			<div class="min-w-0 px-3">
				<a href="/now" class="wide block truncate text-[13px] leading-4 hover:underline">{shown.title}</a>
				{#if item.albumId}
					<a href="/album/{item.albumId}" class="mt-0.5 block truncate text-xs leading-4 text-muted hover:text-fg">{item.artist} — {item.album}</a>
				{:else if shown.station}
					<a href="/radio" class="mt-0.5 block truncate text-xs leading-4 text-muted hover:text-fg">{shown.subtitle}</a>
				{:else}
					<div class="mt-0.5 truncate text-xs leading-4 text-muted">{shown.subtitle}</div>
				{/if}
			</div>
		{:else}
			<div class="label px-3 text-[11px] text-muted">Not playing</div>
		{/if}
	</div>

	<div class="flex flex-col border-x border-line">
		{#if hub.external}
			<button type="button" class="press-lit wide h-full bg-fg text-[13px] text-bg uppercase" onclick={() => api.play()}>Play from library here</button>
		{:else}
			<Transport />
			<div class="flex h-[30px] items-center gap-3 border-t border-line px-3">
				{#if stream}
					<OnAir station={shown?.station ?? null} compact />
				{:else}
					<span class="label w-10 text-right text-[11px] text-muted">{clock(hub.elapsed)}</span>
					<div class="flex-1">
						<Scrubber value={hub.elapsed} max={hub.player.duration} disabled={!item} times={false} onseek={(s) => hub.seek(s)} />
					</div>
					<span class="label w-10 text-[11px] text-muted">{clock(hub.player.duration)}</span>
				{/if}
			</div>
		{/if}
	</div>

	<div class="flex items-stretch justify-end">
		{#if hub.signal.verdict !== 'idle'}
			<a href="/settings" class="press flex min-w-0 items-center overflow-hidden px-3"><SignalBadge signal={hub.signal} /></a>
		{/if}
		<a href="/queue" class="press grid w-14 shrink-0 place-items-center border-l border-line" aria-label="Queue">
			<ListMusic class="size-5" />
		</a>
		<a href="/now" class="press grid w-14 shrink-0 place-items-center border-l border-line" aria-label="Now playing">
			<Maximize2 class="size-[18px]" />
		</a>
	</div>
</div>
