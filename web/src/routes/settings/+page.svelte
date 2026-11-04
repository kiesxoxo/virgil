<script lang="ts">
	import { Airplay, Check, House, ImageDown, LoaderCircle, Monitor, Moon, RefreshCw, RotateCw, Sun, TriangleAlert, Usb } from '@lucide/svelte';
	import SignalPathCard from '#lib/components/SignalPathCard.svelte';
	import { api } from '#lib/api.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { theme, type ThemeMode } from '#lib/theme.svelte.ts';
	import { toasts } from '#lib/toast.svelte.ts';

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

	let outputs = $derived(hub.outputs.outputs);
	let newCards = $derived(hub.outputs.newCards);
	let switching = $state<number | null>(null);
	let restarting = $state(false);

	async function selectOutput(id: number) {
		switching = id;
		try {
			await api.selectOutput(id);
		} finally {
			switching = null;
		}
	}

	async function restart() {
		restarting = true;
		try {
			await api.restart();
		} catch {
			restarting = false;
		}
	}

	// The restart is over once the connection has dropped and come back.
	let dropped = false;
	$effect(() => {
		if (!restarting) return;
		if (!hub.connected) dropped = true;
		else if (dropped) {
			dropped = false;
			restarting = false;
		}
	});

	let airplay = $derived(hub.source.available.includes('airplay'));

	let homekit = $derived(hub.homekit);
	let confirmReset = $state(false);
	let confirmTimer: ReturnType<typeof setTimeout> | undefined;

	function resetHomekit() {
		if (!confirmReset) {
			confirmReset = true;
			clearTimeout(confirmTimer);
			confirmTimer = setTimeout(() => (confirmReset = false), 3000);
			return;
		}
		confirmReset = false;
		api.resetHomekit().then(() => toasts.done('Virgil can be added to a home again'));
	}

	let download = $derived(hub.library.artDownload);
	let downloading = $derived(!!download && !download.error && download.done < download.total);
</script>

<svelte:head><title>Settings · Virgil</title></svelte:head>

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
				class="press flex h-11 items-center justify-center gap-2 rounded-xl text-sm font-semibold {theme.mode === mode.id
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
	{#if outputs.length > 1 || newCards.length}
		<div class="mb-2 divide-y divide-line rounded-2xl bg-surface" role="radiogroup" aria-label="Output device">
			{#each outputs as output (output.id)}
				<button
					type="button"
					role="radio"
					aria-checked={output.enabled}
					class="flex w-full items-center justify-between gap-4 px-4 py-3.5 text-left"
					disabled={switching !== null}
					onclick={() => !output.enabled && selectOutput(output.id)}
				>
					<span class="min-w-0">
						<span class="block truncate text-sm font-medium">{output.label}</span>
						{#if output.label !== output.name}
							<span class="block truncate text-xs text-muted">{output.name}</span>
						{/if}
					</span>
					{#if switching === output.id}
						<LoaderCircle class="size-4 shrink-0 text-muted" />
					{:else if output.enabled}
						<Check class="size-5 shrink-0 text-accent" />
					{/if}
				</button>
			{/each}
			{#if newCards.length}
				<div class="flex items-center justify-between gap-4 px-4 py-3.5">
					<div class="flex min-w-0 items-center gap-3">
						<Usb class="size-4 shrink-0 text-info" />
						<div class="min-w-0">
							<div class="truncate text-sm font-medium">{newCards.join(', ')}</div>
							<div class="text-xs text-muted">Connected since Virgil started. Restart Virgil to use it.</div>
						</div>
					</div>
					<button
						type="button"
						class="press inline-flex h-9 shrink-0 items-center gap-2 rounded-full bg-bg px-4 text-[13px] font-semibold ring-1 ring-line disabled:opacity-50"
						disabled={restarting}
						onclick={restart}
					>
						<RotateCw class="size-3.5" />
						{restarting ? 'Restarting…' : 'Restart'}
					</button>
				</div>
			{/if}
		</div>
	{/if}
	<SignalPathCard signal={hub.signal} />
	<div class="mt-2 divide-y divide-line rounded-2xl bg-surface">
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
					<LoaderCircle class="size-3 " /> Scanning and fetching artwork…
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
			<RefreshCw class="size-3.5" /> Rescan
		</button>
	</div>
	<div class="flex items-center justify-between gap-4 px-4 py-3.5">
		<div class="min-w-0">
			<div class="text-sm font-medium">Album artwork</div>
			<div class="mt-0.5 flex items-center gap-1.5 text-xs text-muted">
				{#if downloading && download}
					<LoaderCircle class="size-3 shrink-0 " />
					Downloading… {download.done} of {download.total}, {download.saved} saved
				{:else if download?.error}
					<span class="text-warn">Stopped: {download.error}</span>
				{:else if download?.failed}
					<span class="text-warn">
						Saved {download.saved}; {download.failed} couldn't be saved (is the album folder writable?)
					</span>
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
			disabled={downloading || !hub.library.missingArt}
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

	{#if homekit}
		{@render section('Apple Home')}
		<div class="divide-y divide-line rounded-2xl bg-surface">
			<div class="flex gap-4 px-4 py-3.5">
				<div class="grid size-10 shrink-0 place-items-center rounded-xl bg-info-bg text-info"><House class="size-5" /></div>
				<div class="min-w-0">
					{#if homekit.paired}
						<div class="text-sm font-medium">In your home</div>
						<p class="mt-0.5 text-[13px] text-muted">
							Say “Hey Siri, turn off Virgil” to stop playback, and “turn on Virgil” to resume. Rename it or give it
							a room in the Home app.
						</p>
					{:else}
						<div class="text-sm font-medium">Not in a home yet</div>
						<p class="mt-0.5 text-[13px] text-muted">
							Add Virgil to the Home app, then say “Hey Siri, turn off Virgil” to stop playback. The Home app calls it
							uncertified; add it anyway.
						</p>
						<div class="mt-3 font-mono text-2xl font-semibold tracking-wider tabular-nums select-all">{homekit.setupCode}</div>
						<a
							href={homekit.setupUri}
							class="press mt-3 inline-flex h-9 items-center gap-2 rounded-full bg-accent px-4 text-[13px] font-semibold text-accent-fg"
						>
							<House class="size-3.5" /> Add to Home
						</a>
						<p class="mt-2 text-xs text-muted">Or in the Home app: Add Accessory › More options…, then enter the code.</p>
					{/if}
					{#if !homekit.advertised}
						<p class="mt-2 flex items-start gap-1.5 text-xs text-warn">
							<TriangleAlert class="mt-px size-3.5 shrink-0" />
							Not visible on the network yet. Is avahi-daemon running on the Pi?
						</p>
					{/if}
				</div>
			</div>
			{#if homekit.paired}
				<div class="flex items-center justify-between gap-4 px-4 py-3.5">
					<div class="min-w-0 text-xs text-muted">
						Removed it in the Home app while Virgil was off? Reset so it can be added again.
					</div>
					<button
						type="button"
						class="press inline-flex h-9 shrink-0 items-center rounded-full px-4 text-[13px] font-semibold ring-1 ring-line {confirmReset
							? 'bg-accent text-accent-fg'
							: 'bg-bg'}"
						onclick={resetHomekit}
					>
						{confirmReset ? 'Tap to confirm' : 'Reset'}
					</button>
				</div>
			{/if}
		</div>
	{/if}
</div>
