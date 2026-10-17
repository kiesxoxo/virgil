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
		class="group relative flex h-5 touch-none items-center select-none"
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
		<div class="h-1 w-full overflow-hidden rounded-full bg-fg/12 transition-[height] group-hover:h-1.5" class:h-1.5={dragging}>
			<div class="h-full rounded-full bg-fg/85" style:width="{pct}%"></div>
		</div>
		{#if active}
			<div
				class="pointer-events-none absolute top-1/2 size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-fg shadow-md transition-transform duration-150"
				class:scale-0={!dragging}
				class:group-hover:scale-100={!dragging}
				style:left="{pct}%"
			></div>
		{/if}
	</div>
	{#if times}
		<div class="mt-1 flex justify-between text-[11px] font-medium text-muted tabular-nums">
			<span>{clock(shown)}</span>
			<span>{max > 0 ? `-${clock(max - shown)}` : '--:--'}</span>
		</div>
	{/if}
</div>
