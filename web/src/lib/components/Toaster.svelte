<script lang="ts">
	import { Check, CircleAlert } from '@lucide/svelte';
	import { toasts } from '../toast.svelte.ts';
</script>

<div
	class="pointer-events-none fixed inset-x-0 above-player z-[60] flex flex-col items-stretch lg:right-auto lg:bottom-[72px] lg:left-60 lg:w-96"
	aria-live="polite"
>
	{#each toasts.items as toast (toast.id)}
		<button
			type="button"
			class="pointer-events-auto flex min-h-11 w-full items-center gap-2.5 border-t border-bg bg-fg px-3 py-2 text-left text-sm font-medium text-bg"
			onclick={() => toasts.dismiss(toast.id)}
		>
			{#if toast.kind === 'done'}
				<Check class="size-4 shrink-0" />
			{:else}
				<CircleAlert class="size-4 shrink-0 text-accent" />
			{/if}
			<span class="truncate">{toast.message}</span>
		</button>
	{/each}
</div>
