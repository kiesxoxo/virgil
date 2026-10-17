<script lang="ts">
	import { formatShort } from '../format.ts';
	import type { SignalPath } from '../types/SignalPath.ts';

	let { signal, class: className = '' }: { signal: SignalPath; class?: string } = $props();

	const tone = {
		bitPerfect: 'bg-good-bg text-good',
		resampled: 'bg-warn-bg text-warn',
		converted: 'bg-warn-bg text-warn',
		external: 'bg-info-bg text-info',
		unknown: 'bg-surface-2 text-muted',
		idle: 'bg-surface-2 text-muted'
	} as const;

	const label = {
		bitPerfect: 'Bit-perfect',
		resampled: 'Resampled',
		converted: 'Converted',
		external: 'AirPlay',
		unknown: 'Unverified',
		idle: 'Idle'
	} as const;

	let detail = $derived(
		signal.verdict === 'external' ? '' : signal.dop ? `${formatShort(signal.source)} DoP` : formatShort(signal.source)
	);
</script>

<span
	class="inline-flex h-6 items-center gap-1.5 rounded-full px-2.5 text-[11px] font-semibold tracking-wide whitespace-nowrap {tone[
		signal.verdict
	]} {className}"
	title={signal.note ?? undefined}
>
	<span class="size-1.5 rounded-full bg-current"></span>
	{label[signal.verdict]}
	{#if detail}<span class="font-medium opacity-75 tabular-nums">{detail}</span>{/if}
</span>
