<script lang="ts">
	import { Airplay, ChevronDown, Disc3, ListMusic } from '@lucide/svelte';
	import Cover from '#lib/components/Cover.svelte';
	import Scrubber from '#lib/components/Scrubber.svelte';
	import SignalPathCard from '#lib/components/SignalPathCard.svelte';
	import Transport from '#lib/components/Transport.svelte';
	import { api } from '#lib/api.ts';
	import { artUrl } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';

	let item = $derived(hub.current);
	let backdrop = $derived(hub.external ? null : artUrl(item?.art, 300));
	let upNext = $derived(hub.currentIndex >= 0 ? hub.queue.items.slice(hub.currentIndex + 1, hub.currentIndex + 4) : []);

	function close() {
		if (history.length > 1) history.back();
		else location.assign('/');
	}
</script>

<svelte:head><title>{item ? `${item.title} · ${item.artist}` : 'Now playing'} · hifi</title></svelte:head>

<div class="fixed inset-0 -z-10 overflow-hidden bg-bg">
	{#if backdrop}
		{#key backdrop}
			<img src={backdrop} alt="" aria-hidden="true" class="size-full scale-150 object-cover opacity-50 blur-3xl saturate-150 dark:opacity-40" />
		{/key}
	{/if}
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
			<Cover art={item.art} size={600} eager class="w-full rounded-3xl shadow-cover ring-1 ring-line transition-transform duration-500 {hub.playing ? 'scale-100' : 'scale-[0.94]'}" />
		{:else}
			<div class="grid aspect-square w-full place-items-center rounded-3xl bg-surface text-faint">
				<Disc3 class="size-1/3" strokeWidth={1} />
			</div>
		{/if}

		<div class="flex flex-col gap-6">
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
					<h1 class="truncate text-2xl font-bold tracking-tight lg:text-3xl">{item?.title ?? 'Not playing'}</h1>
					{#if item?.albumId}
						<a href="/album/{item.albumId}" class="mt-1 block truncate text-lg text-muted hover:text-fg">
							{item.artist}{item.album ? ` — ${item.album}` : ''}
						</a>
					{:else}
						<p class="mt-1 truncate text-lg text-muted">{item?.artist ?? 'Pick an album to start'}</p>
					{/if}
				</div>
				<Scrubber value={hub.elapsed} max={hub.player.duration} disabled={!item} onseek={(s) => hub.seek(s)} />
				<Transport size="lg" />
			{/if}

			<SignalPathCard signal={hub.signal} details={false} />

			{#if upNext.length}
				<div>
					<h2 class="mb-1 text-xs font-semibold tracking-wider text-muted uppercase">Up next</h2>
					<ul>
						{#each upNext as next (next.id)}
							<li>
								<button type="button" class="press flex w-full items-center gap-3 rounded-xl py-1.5 text-left" onclick={() => api.playItem(next.id)}>
									<Cover art={next.art} class="size-10 shrink-0 rounded-md" />
									<span class="min-w-0">
										<span class="block truncate text-sm font-medium">{next.title}</span>
										<span class="block truncate text-xs text-muted">{next.artist}</span>
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
