import type { Component } from 'svelte';
import type { Attachment } from 'svelte/attachments';

export interface MenuItem {
	label: string;
	icon: Component<{ class?: string }>;
	run: () => unknown;
	danger?: boolean;
}

/** Shown at the top of the menu (the phone sheet) so it's clear what it acts on. */
export interface MenuHeader {
	title: string;
	subtitle?: string;
	art?: string | null;
}

/**
 * The one action menu of the app. It is drawn by <ContextMenu> at the root of
 * the layout, so no row's transform or containment can clip or misplace it.
 */
class MenuState {
	open = $state(false);
	items = $state.raw<MenuItem[]>([]);
	header = $state.raw<MenuHeader | null>(null);
	/** Where the menu's top-left corner wants to be… */
	x = $state(0);
	y = $state(0);
	/** …and the y its bottom edge uses when there is no room below. */
	yAbove = $state(0);
	/** Anchored to a button: `x` is the right edge the menu lines up with. */
	alignRight = $state(false);
	openedAt = 0;

	/** Opens at a point (right-click, long-press) or below an element (⋯ button). */
	show(items: MenuItem[], at: { x: number; y: number } | DOMRect, header: MenuHeader | null = null) {
		if (at instanceof DOMRect) {
			this.x = at.right;
			this.y = at.bottom + 6;
			this.yAbove = at.top - 6;
			this.alignRight = true;
		} else {
			this.x = at.x;
			this.y = at.y;
			this.yAbove = at.y;
			this.alignRight = false;
		}
		this.items = items;
		this.header = header;
		this.open = true;
		this.openedAt = performance.now();
	}

	close() {
		this.open = false;
	}

	run(item: MenuItem) {
		this.open = false;
		item.run();
	}
}

export const menu = new MenuState();

const LONG_PRESS_MS = 450;

/**
 * Right-click (desktop), long-press (phones) or the platform context-menu key
 * open `items()` for the element. A long press swallows the click that follows,
 * so a long-pressed link doesn't also navigate.
 */
export function contextMenu(items: () => MenuItem[], header?: () => MenuHeader | null): Attachment<HTMLElement> {
	return (node) => {
		let timer: ReturnType<typeof setTimeout> | undefined;
		let start = { x: 0, y: 0 };
		let pressed = false;

		const open = (x: number, y: number) => menu.show(items(), { x, y }, header?.() ?? null);
		const cancel = () => clearTimeout(timer);

		const onContextMenu = (e: MouseEvent) => {
			e.preventDefault();
			// Android fires contextmenu for a long press too; it is already open.
			if (pressed) return;
			open(e.clientX, e.clientY);
		};
		const onPointerDown = (e: PointerEvent) => {
			if (e.pointerType !== 'touch') return;
			pressed = false;
			start = { x: e.clientX, y: e.clientY };
			cancel();
			timer = setTimeout(() => {
				pressed = true;
				navigator.vibrate?.(8);
				open(start.x, start.y);
			}, LONG_PRESS_MS);
		};
		const onPointerMove = (e: PointerEvent) => {
			if (Math.hypot(e.clientX - start.x, e.clientY - start.y) > 10) cancel();
		};
		const onClick = (e: MouseEvent) => {
			if (!pressed) return;
			pressed = false;
			e.preventDefault();
			e.stopPropagation();
		};

		node.classList.add('touch-menu');
		node.addEventListener('contextmenu', onContextMenu);
		node.addEventListener('pointerdown', onPointerDown);
		node.addEventListener('pointermove', onPointerMove);
		node.addEventListener('pointerup', cancel);
		node.addEventListener('pointercancel', cancel);
		node.addEventListener('click', onClick, true);
		return () => {
			cancel();
			node.removeEventListener('contextmenu', onContextMenu);
			node.removeEventListener('pointerdown', onPointerDown);
			node.removeEventListener('pointermove', onPointerMove);
			node.removeEventListener('pointerup', cancel);
			node.removeEventListener('pointercancel', cancel);
			node.removeEventListener('click', onClick, true);
		};
	};
}
