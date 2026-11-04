<script lang="ts">
	import { ArrowDown, ArrowUp } from '@lucide/svelte';

	// One word in the sort row. It reaches past the row, to 44px tall for a finger.
	let { label, active, desc, onclick }: { label: string; active: boolean; desc: boolean; onclick: () => void } = $props();
</script>

<button
	type="button"
	class="label relative flex shrink-0 items-center gap-1 text-[10.5px] after:absolute after:-inset-x-2 after:top-0 after:-bottom-2 {active
		? 'text-accent'
		: 'text-muted hover:text-fg'}"
	aria-pressed={active}
	{onclick}
>
	{label}
	{#if active}
		{#if desc}<ArrowDown class="size-3" />{:else}<ArrowUp class="size-3" />{/if}
	{/if}
</button>
