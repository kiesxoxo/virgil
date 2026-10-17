<script lang="ts">
	import { Airplay, ImageDown, LoaderCircle, Monitor, Moon, RefreshCw, Sun } from '@lucide/svelte';
	import SignalPathCard from '#lib/components/SignalPathCard.svelte';
	import { api } from '#lib/api.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { theme, type ThemeMode } from '#lib/theme.svelte.ts';

	const modes: { id: ThemeMode; label: string; icon: typeof Sun }[] = [
		{ id: 'system', label: 'System', icon: Monitor },
		{ id: 'light', label: 'Light', icon: Sun },
		{ id: 'dark', label: 'Dark', icon: Moon }
	];

	let rescanning = $state(false);

	async function rescan() {
		rescanning = true;
		try {
			await api.rescan();
		} finally {
			setTimeout(() => (rescanning = false), 1200);
		}
	}

	let airplay = $derived(hub.source.available.includes('airplay'));
	let download = $derived(hub.library.artDownload);
</script>

<svelte:head><title>Settings · hifi</title></svelte:head>

{#snippet section(title: string)}
	<h2 class="mt-8 mb-2 px-1 text-xs font-semibold tracking-wider text-muted uppercase">{title}</h2>
{/snippet}

<div class="pt-safe mx-auto max-w-2xl px-4 pt-4 lg:px-8 lg:pt-8">
	<h1 class="text-[28px] font-bold tracking-tight lg:text-[32px]">Settings</h1>

	{@render section('Appearance')}
	<div class="grid grid-cols-3 gap-1 rounded-2xl bg-surface p-1">
		{#each modes as mode (mode.id)}
			<button
				type="button"
				class="press flex h-11 items-center justify-center gap-2 rounded-xl text-sm font-semibold transition-colors {theme.mode === mode.id
					? 'bg-bg text-fg shadow-sm ring-1 ring-line'
					: 'text-muted hover:text-fg'}"
				aria-pressed={theme.mode === mode.id}
				onclick={() => theme.set(mode.id)}
			>
				<mode.icon class="size-4" />
				{mode.label}
			</button>
		{/each}
	</div>

	{@render section('Output')}
	<SignalPathCard signal={hub.signal} />
	<div class="mt-2 divide-y divide-line rounded-2xl bg-surface">
		<div class="flex items-center justify-between gap-4 px-4 py-3.5">
			<span class="text-sm font-medium">Volume</span>
			<span class="text-right text-sm text-muted">
				{#if hub.player.volume == null}
					Fixed — set it on your DAC or amp
				{:else}
					{hub.player.volume}% <span class="text-warn">(MPD mixer active)</span>
				{/if}
			</span>
		</div>
		<div class="flex items-center justify-between gap-4 px-4 py-3.5">
			<span class="text-sm font-medium">Playback engine</span>
			<span class="flex items-center gap-2 text-sm text-muted">
				<span class="size-2 rounded-full {hub.player.online ? 'bg-good' : 'bg-warn'}"></span>
				{hub.player.online ? 'MPD connected' : 'MPD unreachable'}
			</span>
		</div>
	</div>

	{@render section('Library')}
	<div class="divide-y divide-line rounded-2xl bg-surface">
	<div class="flex items-center justify-between gap-4 px-4 py-3.5">
		<div>
			<div class="text-sm font-medium">
				{hub.library.albums.toLocaleString()} albums · {hub.library.tracks.toLocaleString()} songs
			</div>
			<div class="mt-0.5 flex items-center gap-1.5 text-xs text-muted">
				{#if hub.library.scanning}
					<LoaderCircle class="size-3 animate-spin" /> Scanning and fetching artwork…
				{:else}
					Up to date with MPD's database
				{/if}
			</div>
		</div>
		<button
			type="button"
			class="press inline-flex h-9 shrink-0 items-center gap-2 rounded-full bg-bg px-4 text-[13px] font-semibold ring-1 ring-line disabled:opacity-50"
			disabled={rescanning || !hub.player.online}
			onclick={rescan}
		>
			<RefreshCw class="size-3.5 {rescanning ? 'animate-spin' : ''}" /> Rescan
		</button>
	</div>
	<div class="flex items-center justify-between gap-4 px-4 py-3.5">
		<div class="min-w-0">
			<div class="text-sm font-medium">Album artwork</div>
			<div class="mt-0.5 flex items-center gap-1.5 text-xs text-muted">
				{#if download && !download.error}
					<LoaderCircle class="size-3 shrink-0 animate-spin" />
					Downloading… {download.done} of {download.total}, {download.saved} saved
				{:else if download?.error}
					<span class="text-warn">Stopped: {download.error}</span>
				{:else if hub.library.missingArt}
					{hub.library.missingArt.toLocaleString()}
					{hub.library.missingArt === 1 ? 'album has' : 'albums have'} no artwork
				{:else}
					Every album has artwork
				{/if}
			</div>
		</div>
		<button
			type="button"
			class="press inline-flex h-9 shrink-0 items-center gap-2 rounded-full bg-bg px-4 text-[13px] font-semibold ring-1 ring-line disabled:opacity-50"
			disabled={(!!download && !download.error) || !hub.library.missingArt}
			title="Saves cover.jpg in each album folder that has no embedded or folder art"
			onclick={() => api.downloadMissingArt()}
		>
			<ImageDown class="size-3.5" /> Download missing
		</button>
	</div>
	</div>

	{#if airplay}
		{@render section('AirPlay')}
		<div class="flex gap-4 rounded-2xl bg-surface px-4 py-3.5">
			<div class="grid size-10 shrink-0 place-items-center rounded-xl bg-info-bg text-info"><Airplay class="size-5" /></div>
			<div>
				<div class="text-sm font-medium">{hub.external ? 'Streaming now' : 'Ready'}</div>
				<p class="mt-0.5 text-[13px] text-muted">
					This player is an AirPlay 2 speaker. Group it with other rooms from Control Center on an iPhone or Mac.
					Starting playback here takes the DAC back from AirPlay.
				</p>
			</div>
		</div>
	{/if}
</div>
