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

<div>
	<div class="flex min-h-11 items-center justify-between gap-3 px-3">
		<span class="label text-[11px] text-muted">Signal path</span>
		<SignalBadge {signal} />
	</div>
	{#if details}
		<p class="px-3 pb-3 text-sm text-muted">{signal.note && signal.verdict !== 'external' ? `${explain[signal.verdict]} ${signal.note}.` : explain[signal.verdict]}</p>
	{/if}
	{#if signal.source || signal.output}
		<div class="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-stretch border-t border-line">
			<div class="px-3 py-2.5">
				<div class="label text-[10px] text-muted">Source</div>
				<div class="mt-1 text-[13px] font-medium">{signal.source ? formatLong(signal.source) : '—'}</div>
			</div>
			<div class="grid place-items-center border-x border-line px-2 text-muted"><ArrowRight class="size-4" /></div>
			<div class="min-w-0 px-3 py-2.5">
				<div class="label truncate text-[10px] text-muted" title={signal.device ?? undefined}>{signal.device ?? 'DAC'}</div>
				<div class="mt-1 text-[13px] font-medium" title={signal.output?.format}>
					{signal.output ? `${alsaFormat(signal.output.format)} · ${khz(signal.output.rate)} kHz` : '—'}
				</div>
			</div>
		</div>
	{/if}
</div>
