<script lang="ts">
	import { Repeat, Repeat1, Shuffle } from '@lucide/svelte';
	import { api } from '../api.ts';
	import { hub } from '../hub.svelte.ts';
	import Glyph from './Glyph.svelte';

	let { size = 'md' }: { size?: 'md' | 'lg' } = $props();

	let big = $derived(size === 'lg');
	let RepeatIcon = $derived(hub.player.repeat && hub.player.single ? Repeat1 : Repeat);
	let disabled = $derived(!hub.player.online || hub.queue.items.length === 0);
	const cell = 'grid place-items-center border-l border-line first:border-l-0';
</script>

<div class="grid grid-cols-[1fr_1.15fr_1.5fr_1.15fr_1fr] {big ? 'h-[88px]' : 'h-10'}">
	<button
		type="button"
		class="press {cell} {hub.player.random ? 'text-accent' : 'text-muted hover:text-fg'}"
		aria-label="Shuffle"
		aria-pressed={hub.player.random}
		onclick={() => hub.toggleShuffle()}
	>
		<Shuffle class={big ? 'size-[22px]' : 'size-4'} />
	</button>
	<button type="button" class="press {cell} disabled:text-faint" aria-label="Previous" {disabled} onclick={() => api.previous()}>
		<Glyph name="previous" class={big ? 'size-7' : 'size-5'} />
	</button>
	<button
		type="button"
		class="press {cell} bg-accent text-accent-fg disabled:bg-surface-2 disabled:text-faint"
		aria-label={hub.playing ? 'Pause' : 'Play'}
		{disabled}
		onclick={() => hub.toggle()}
	>
		<Glyph name={hub.playing ? 'pause' : 'play'} class={big ? 'size-[34px]' : 'size-5'} />
	</button>
	<button type="button" class="press {cell} disabled:text-faint" aria-label="Next" {disabled} onclick={() => api.next()}>
		<Glyph name="next" class={big ? 'size-7' : 'size-5'} />
	</button>
	<button
		type="button"
		class="press {cell} {hub.player.repeat ? 'text-accent' : 'text-muted hover:text-fg'}"
		aria-label="Repeat"
		aria-pressed={hub.player.repeat}
		onclick={() => hub.cycleRepeat()}
	>
		<RepeatIcon class={big ? 'size-[22px]' : 'size-4'} />
	</button>
</div>
