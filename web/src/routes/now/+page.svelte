<script lang="ts">
	import { Airplay, ChevronDown, Disc3, ExternalLink, ListMusic } from '@lucide/svelte';
	import Backdrop from '#lib/components/Backdrop.svelte';
	import Cover from '#lib/components/Cover.svelte';
	import OnAir from '#lib/components/OnAir.svelte';
	import Scrubber from '#lib/components/Scrubber.svelte';
	import SignalPathCard from '#lib/components/SignalPathCard.svelte';
	import Transport from '#lib/components/Transport.svelte';
	import { api } from '#lib/api.ts';
	import { artUrl, hhmm } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { isStream, radio } from '#lib/radio.svelte.ts';

	let item = $derived(hub.current);
	let shown = $derived(item ? radio.show(item) : null);
	let station = $derived(shown?.station ?? null);
	let stream = $derived(item ? isStream(item.uri) : false);
	let backdrop = $derived(hub.external ? null : (shown?.src ?? artUrl(item?.art, 300)));
	let upNext = $derived(hub.currentIndex >= 0 ? hub.queue.items.slice(hub.currentIndex + 1, hub.currentIndex + 4) : []);

	function close() {
		if (history.length > 1) history.back();
		else location.assign('/');
	}
</script>

<svelte:head><title>{shown ? `${shown.title} · ${shown.subtitle}` : 'Now playing'} · Virgil</title></svelte:head>

<div class="fixed inset-0 -z-10 overflow-hidden bg-bg">
	<Backdrop src={backdrop} playing={hub.playing} />
	<div class="absolute inset-0 bg-gradient-to-b from-bg/30 via-bg/60 to-bg"></div>
</div>

<div class="pt-safe pb-safe mx-auto flex min-h-dvh max-w-md flex-col px-6 lg:max-w-5xl lg:px-10">
	<header class="flex h-14 items-center justify-between">
		<button type="button" class="press -ml-2 grid size-10 place-items-center rounded-full hover:bg-fg/6" aria-label="Close" onclick={close}>
			<ChevronDown class="size-6" />
		</button>
		<span class="text-xs font-semibold tracking-wider text-muted uppercase">{hub.external ? 'AirPlay' : 'Now playing'}</span>
		<a href="/queue" class="press -mr-2 grid size-10 place-items-center rounded-full hover:bg-fg/6" aria-label="Queue">
			<ListMusic class="size-5" />
		</a>
	</header>

	<div class="flex flex-1 flex-col justify-center gap-8 py-4 lg:grid lg:grid-cols-[minmax(0,26rem)_1fr] lg:items-center lg:gap-14">
		{#if hub.external}
			<div class="grid aspect-square w-full place-items-center rounded-3xl bg-info-bg text-info shadow-cover">
				<Airplay class="size-1/3" strokeWidth={1.25} />
			</div>
		{:else if item}
			<Cover art={item.art} src={shown?.large} size={600} eager class="w-full rounded-3xl shadow-cover ring-1 ring-line transition-transform duration-500 {hub.playing ? 'scale-100' : 'scale-[0.94]'}" />
		{:else}
			<div class="grid aspect-square w-full place-items-center rounded-3xl bg-surface text-faint">
				<Disc3 class="size-1/3" strokeWidth={1} />
			</div>
		{/if}

		<div class="flex min-w-0 flex-col gap-6">
			{#if hub.external}
				<div>
					<h1 class="text-2xl font-bold tracking-tight">AirPlay</h1>
					<p class="mt-1 text-muted">Another device is streaming to this player. Its controls live on that device.</p>
				</div>
				<button type="button" class="press h-12 rounded-full bg-fg font-semibold text-bg" onclick={() => api.play()}>
					Play from library here
				</button>
			{:else}
				<div class="min-w-0">
					<h1 class="line-clamp-2 text-2xl font-bold tracking-tight lg:text-3xl">{shown?.title ?? 'Not playing'}</h1>
					{#if item?.albumId}
						<a href="/album/{item.albumId}" class="mt-1 block truncate text-lg text-muted hover:text-fg">
							{item.artist}{item.album ? ` — ${item.album}` : ''}
						</a>
					{:else if station}
						<a href="/radio" class="mt-1 block truncate text-lg text-muted hover:text-fg">{shown?.subtitle}</a>
					{:else}
						<p class="mt-1 truncate text-lg text-muted">{shown?.subtitle ?? 'Pick an album or a station'}</p>
					{/if}
				</div>
				{#if stream}
					<OnAir {station} />
				{:else}
					<Scrubber value={hub.elapsed} max={hub.player.duration} disabled={!item} onseek={(s) => hub.seek(s)} />
				{/if}
				<Transport size="lg" />
				{#if station}
					{@const show = station.now}
					<div class="flex flex-col gap-3 text-sm">
						{#if show}
							{#if show.genres.length}
								<div class="flex flex-wrap gap-1.5">
									{#each show.genres as genre (genre)}
										<span class="rounded-full bg-fg/6 px-2.5 py-1 text-xs font-medium">{genre}</span>
									{/each}
								</div>
							{/if}
							{#if show.description}<p class="line-clamp-3 text-muted">{show.description}</p>{/if}
							{#if station.next}
								<p class="truncate text-muted">
									<span class="font-semibold text-fg">Next on {station.name} at {hhmm(station.next.start)}</span>&ensp;{station.next.title}
								</p>
							{/if}
							{#if show.url}
								<a href={show.url} target="_blank" rel="noopener" class="inline-flex w-fit items-center gap-1.5 font-medium text-muted hover:text-fg">
									This show on nts.live <ExternalLink class="size-3.5" />
								</a>
							{/if}
						{:else if station.description ?? station.tagline}
							<p class="line-clamp-3 text-muted">{station.description ?? station.tagline}</p>
						{/if}
					</div>
				{/if}
			{/if}

			<SignalPathCard signal={hub.signal} details={false} />

			{#if upNext.length}
				<div>
					<h2 class="mb-1 text-xs font-semibold tracking-wider text-muted uppercase">Up next</h2>
					<ul>
						{#each upNext as next (next.id)}
							{@const shownNext = radio.show(next)}
							<li>
								<button type="button" class="press flex w-full items-center gap-3 rounded-xl py-1.5 text-left" onclick={() => api.playItem(next.id)}>
									<Cover art={shownNext.art} src={shownNext.src} class="size-10 shrink-0 rounded-md" />
									<span class="min-w-0">
										<span class="block truncate text-sm font-medium">{shownNext.title}</span>
										<span class="block truncate text-xs text-muted">{shownNext.subtitle}</span>
									</span>
								</button>
							</li>
						{/each}
					</ul>
				</div>
			{/if}
		</div>
	</div>
</div>
