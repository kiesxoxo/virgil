<script lang="ts">
	import { formatShort } from '../format.ts';
	import type { SignalPath } from '../types/SignalPath.ts';

	let { signal, class: className = '' }: { signal: SignalPath; class?: string } = $props();

	const tone = {
		bitPerfect: 'text-good',
		resampled: 'text-warn',
		converted: 'text-warn',
		external: 'text-info',
		unknown: 'text-muted',
		idle: 'text-muted'
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
	class="label inline-flex items-center gap-1.5 text-[10px] whitespace-nowrap {tone[signal.verdict]} {className}"
	title={signal.note ?? undefined}
>
	<span class="size-1.5 rounded-full bg-current"></span>
	{label[signal.verdict]}
	{#if detail}<span>{detail}</span>{/if}
</span>
