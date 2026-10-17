<script lang="ts">
	import { ArrowRight } from '@lucide/svelte';
	import { alsaFormat, formatLong, khz } from '../format.ts';
	import type { SignalPath } from '../types/SignalPath.ts';
	import SignalBadge from './SignalBadge.svelte';

	/** `details`: the sentence explaining the verdict (Settings shows it, Now Playing doesn't). */
	let { signal, details = true }: { signal: SignalPath; details?: boolean } = $props();

	const explain = {
		bitPerfect: 'Every bit of the file reaches the DAC unchanged.',
		resampled: 'The sample rate is being converted before the DAC.',
		converted: 'Samples are being altered before the DAC.',
		external: 'Another device is streaming to this player.',
		unknown: 'The output could not be verified.',
		idle: 'The output is closed.'
	} as const;
</script>

<div class="rounded-2xl bg-surface p-4">
	<div class="flex items-center justify-between gap-3">
		<span class="text-sm font-semibold">Signal path</span>
		<SignalBadge {signal} />
	</div>
	{#if details}
		<p class="mt-2 text-sm text-muted">{signal.note && signal.verdict !== 'external' ? `${explain[signal.verdict]} ${signal.note}.` : explain[signal.verdict]}</p>
	{/if}
	{#if signal.source || signal.output}
		<div class="mt-3 grid grid-cols-[1fr_auto_1fr] items-center gap-2 text-xs">
			<div class="rounded-xl bg-bg px-3 py-2">
				<div class="text-faint">Source</div>
				<div class="mt-0.5 font-medium tabular-nums">{signal.source ? formatLong(signal.source) : '—'}</div>
			</div>
			<ArrowRight class="size-4 text-faint" />
			<div class="min-w-0 rounded-xl bg-bg px-3 py-2">
				<div class="truncate text-faint" title={signal.device ?? undefined}>{signal.device ?? 'DAC'}</div>
				<div class="mt-0.5 font-medium tabular-nums" title={signal.output?.format}>
					{signal.output ? `${alsaFormat(signal.output.format)} · ${khz(signal.output.rate)} kHz` : '—'}
				</div>
			</div>
		</div>
	{/if}
</div>
