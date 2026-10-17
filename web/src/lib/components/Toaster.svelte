<script lang="ts">
	import { CircleAlert } from '@lucide/svelte';
	import { fly } from 'svelte/transition';
	import { toasts } from '../toast.svelte.ts';
</script>

<div
	class="pointer-events-none fixed inset-x-0 z-50 flex flex-col items-center gap-2 px-4 lg:bottom-28"
	style:bottom="calc(9.5rem + env(safe-area-inset-bottom))"
	aria-live="polite"
>
	{#each toasts.items as toast (toast.id)}
		<button
			type="button"
			transition:fly={{ y: 12, duration: 180 }}
			class="pointer-events-auto flex max-w-sm items-center gap-2.5 rounded-full bg-fg py-2.5 pr-4 pl-3 text-sm font-medium text-bg shadow-float"
			onclick={() => toasts.dismiss(toast.id)}
		>
			<CircleAlert class="size-4 shrink-0 text-accent" />
			<span class="truncate">{toast.message}</span>
		</button>
	{/each}
</div>
