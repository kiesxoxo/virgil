<script lang="ts">
	import { Pause, Play, Repeat, Repeat1, Shuffle, SkipBack, SkipForward } from '@lucide/svelte';
	import { api } from '../api.ts';
	import { hub } from '../hub.svelte.ts';

	let { size = 'md' }: { size?: 'md' | 'lg' } = $props();

	let big = $derived(size === 'lg');
	let RepeatIcon = $derived(hub.player.repeat && hub.player.single ? Repeat1 : Repeat);
	let disabled = $derived(!hub.player.online || hub.queue.items.length === 0);
</script>

<div class="flex items-center {big ? 'justify-between' : 'justify-center gap-2'}">
	<button
		type="button"
		class="press grid place-items-center rounded-full {big ? 'size-11' : 'size-9'} {hub.player.random
			? 'text-accent'
			: 'text-muted hover:text-fg'}"
		aria-label="Shuffle"
		aria-pressed={hub.player.random}
		onclick={() => hub.toggleShuffle()}
	>
		<Shuffle class={big ? 'size-5' : 'size-4'} />
	</button>
	<button
		type="button"
		class="press grid place-items-center rounded-full text-fg disabled:opacity-35 {big ? 'size-14' : 'size-10'}"
		aria-label="Previous"
		{disabled}
		onclick={() => api.previous()}
	>
		<SkipBack class={big ? 'size-7' : 'size-5'} fill="currentColor" />
	</button>
	<button
		type="button"
		class="press grid place-items-center rounded-full bg-fg text-bg shadow-float disabled:opacity-35 {big ? 'size-[4.5rem]' : 'size-11'}"
		aria-label={hub.playing ? 'Pause' : 'Play'}
		{disabled}
		onclick={() => hub.toggle()}
	>
		{#if hub.playing}
			<Pause class={big ? 'size-8' : 'size-5'} fill="currentColor" strokeWidth={0} />
		{:else}
			<Play class="{big ? 'size-8' : 'size-5'} ml-[3px]" fill="currentColor" strokeWidth={0} />
		{/if}
	</button>
	<button
		type="button"
		class="press grid place-items-center rounded-full text-fg disabled:opacity-35 {big ? 'size-14' : 'size-10'}"
		aria-label="Next"
		{disabled}
		onclick={() => api.next()}
	>
		<SkipForward class={big ? 'size-7' : 'size-5'} fill="currentColor" />
	</button>
	<button
		type="button"
		class="press grid place-items-center rounded-full {big ? 'size-11' : 'size-9'} {hub.player.repeat
			? 'text-accent'
			: 'text-muted hover:text-fg'}"
		aria-label="Repeat"
		aria-pressed={hub.player.repeat}
		onclick={() => hub.cycleRepeat()}
	>
		<RepeatIcon class={big ? 'size-5' : 'size-4'} />
	</button>
</div>
