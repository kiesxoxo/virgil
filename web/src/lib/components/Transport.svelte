<script lang="ts">
	import { Repeat, Repeat1, Shuffle } from '@lucide/svelte';
	import { api } from '../api.ts';
	import { hub } from '../hub.svelte.ts';
	import { line } from '../party.svelte.ts';
	import Glyph from './Glyph.svelte';

	let { size = 'md' }: { size?: 'md' | 'lg' } = $props();

	let big = $derived(size === 'lg');
	let RepeatIcon = $derived(hub.player.repeat && hub.player.single ? Repeat1 : Repeat);
	let disabled = $derived(!hub.player.online || hub.queue.items.length === 0);
	// Guests on the request line: no going back, shuffling or repeating; Next may be a vote.
	let guest = $derived(!line.host);
	let vote = $derived(line.skipKey === 'vote');
	const cell = 'grid place-items-center border-l border-line first:border-l-0';
</script>

<div class="grid grid-cols-[1fr_1.15fr_1.5fr_1.15fr_1fr] {big ? 'h-[88px]' : 'h-10'}">
	<button
		type="button"
		class="press {cell} disabled:text-faint {hub.player.random ? 'text-accent' : 'text-muted hover:text-fg'}"
		aria-label="Shuffle"
		aria-pressed={hub.player.random}
		disabled={guest}
		onclick={() => hub.toggleShuffle()}
	>
		<Shuffle class={big ? 'size-[22px]' : 'size-4'} />
	</button>
	<button type="button" class="press {cell} disabled:text-faint" aria-label="Previous" disabled={disabled || guest} onclick={() => api.previous()}>
		<Glyph name="previous" class={big ? 'size-7' : 'size-5'} />
	</button>
	<button
		type="button"
		class="press {cell} bg-accent text-accent-fg disabled:bg-surface-2 disabled:text-faint"
		aria-label={hub.playing ? 'Pause' : 'Play'}
		disabled={disabled || !line.canPause}
		onclick={() => hub.toggle()}
	>
		<Glyph name={hub.playing ? 'pause' : 'play'} class={big ? 'size-[34px]' : 'size-5'} />
	</button>
	<button
		type="button"
		class="{cell} disabled:text-faint {vote && line.votedSkip ? 'press-lit bg-fg text-bg' : 'press'}"
		aria-label={vote ? 'Vote to skip' : 'Next'}
		aria-pressed={vote ? line.votedSkip : undefined}
		disabled={disabled || line.skipKey === 'none'}
		onclick={() => line.skip()}
	>
		<span class="grid justify-items-center gap-0.5">
			<Glyph name="next" class={big ? 'size-7' : 'size-5'} />
			{#if vote}<span class="label text-[10px] leading-none">Skip {hub.party.votes.skip.length}/{hub.party.votes.need}</span>{/if}
		</span>
	</button>
	<button
		type="button"
		class="press {cell} disabled:text-faint {hub.player.repeat ? 'text-accent' : 'text-muted hover:text-fg'}"
		aria-label="Repeat"
		aria-pressed={hub.player.repeat}
		disabled={guest}
		onclick={() => hub.cycleRepeat()}
	>
		<RepeatIcon class={big ? 'size-[22px]' : 'size-4'} />
	</button>
</div>
