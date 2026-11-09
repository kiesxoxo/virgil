<script lang="ts">
	// A sheet from the bottom of the screen (centred on a larger one), over a dimmed page.
	import type { Snippet } from 'svelte';

	let { onclose, label, children }: { onclose: () => void; label: string; children: Snippet } = $props();
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="fixed inset-0 z-50 bg-black/60" role="presentation" onclick={onclose}></div>
<div
	class="pb-safe fixed inset-x-0 bottom-0 z-50 max-h-[calc(100dvh-var(--top)-2rem)] overflow-y-auto overscroll-contain border-t border-line bg-bg sm:inset-x-auto sm:left-1/2 sm:w-[28rem] sm:-translate-x-1/2 sm:border-x"
	role="dialog"
	aria-modal="true"
	aria-label={label}
>
	{@render children()}
</div>
