<script lang="ts">
	import { clock } from '../format.ts';

	let {
		value,
		max,
		onseek,
		disabled = false,
		times = true
	}: { value: number; max: number; onseek: (seconds: number) => void; disabled?: boolean; times?: boolean } =
		$props();

	let track: HTMLDivElement;
	let dragging = $state(false);
	let dragValue = $state(0);
	let shown = $derived(dragging ? dragValue : value);
	let pct = $derived(max > 0 ? Math.min(100, Math.max(0, (shown / max) * 100)) : 0);
	let active = $derived(!disabled && max > 0);

	function at(e: PointerEvent) {
		const r = track.getBoundingClientRect();
		return (Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)) || 0) * max;
	}

	function down(e: PointerEvent) {
		if (!active) return;
		dragging = true;
		track.setPointerCapture(e.pointerId);
		dragValue = at(e);
	}

	function move(e: PointerEvent) {
		if (dragging) dragValue = at(e);
	}

	function up(e: PointerEvent) {
		if (!dragging) return;
		dragging = false;
		onseek(at(e));
	}

	function key(e: KeyboardEvent) {
		if (!active) return;
		const step = e.key === 'ArrowRight' ? 5 : e.key === 'ArrowLeft' ? -5 : 0;
		if (!step) return;
		e.preventDefault();
		onseek(Math.min(max, Math.max(0, value + step)));
	}
</script>

<div class="w-full">
	<div
		bind:this={track}
		class="flex touch-none items-center select-none {times ? 'h-11' : 'h-6'}"
		class:cursor-pointer={active}
		role="slider"
		tabindex={active ? 0 : -1}
		aria-label="Seek"
		aria-valuemin={0}
		aria-valuemax={Math.round(max)}
		aria-valuenow={Math.round(shown)}
		aria-valuetext={clock(shown)}
		onpointerdown={down}
		onpointermove={move}
		onpointerup={up}
		onpointercancel={() => (dragging = false)}
		onkeydown={key}
	>
		<div class="w-full overflow-hidden border {times ? 'h-3.5' : 'h-2.5'} {active ? 'border-fg' : 'border-line'}">
			<div class="h-full bg-fg" style:transform="translateX({pct - 100}%)"></div>
		</div>
	</div>
	{#if times}
		<div class="label -mt-1 flex justify-between text-[11px] text-muted">
			<span class="text-fg">{clock(shown)}</span>
			<span>{max > 0 ? `-${clock(max - shown)}` : '--:--'}</span>
		</div>
	{/if}
</div>
