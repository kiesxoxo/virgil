// The library search, shared by the Albums, Artists and Genres pages, the
// phone's search bar at the bottom and the `/` shortcut.

import { tick } from 'svelte';
import { goto } from '$app/navigation';
import { page } from '$app/state';

/** The pages the search filters. */
export const isLibraryPage = (path: string) => path === '/' || path === '/artists' || path === '/genres';

class Search {
	query = $state('');
	/** Phones: the search bar at the bottom is open. */
	open = $state(false);
	/** The search field in the current library page's header (wide screens). */
	field: HTMLInputElement | null = null;

	/** Attachment for that header field. */
	register = (node: HTMLInputElement) => {
		this.field = node;
		return () => {
			if (this.field === node) this.field = null;
		};
	};

	/**
	 * Searching from a page the search doesn't filter (queue, settings) goes to
	 * the albums. `keepFocus`: typing in the bottom bar carries on there.
	 */
	async showResults(keepFocus = false) {
		if (!isLibraryPage(page.url.pathname)) await goto('/', { reset: !keepFocus });
	}

	/**
	 * The `/` shortcut: the header field, after going to the albums if needed.
	 * iOS only focuses during the key press itself, so when the field isn't on
	 * the page yet a stand-in takes the focus and hands it over afterwards.
	 */
	async focusField() {
		if (this.field && isLibraryPage(page.url.pathname)) {
			this.field.focus();
			this.field.select();
			return;
		}
		const standIn = document.createElement('input');
		standIn.setAttribute('aria-hidden', 'true');
		standIn.style.cssText = 'position:fixed;top:0;left:0;width:1px;height:1px;opacity:0;font-size:16px';
		document.body.append(standIn);
		standIn.focus();
		try {
			await this.showResults();
			await tick();
			this.field?.focus();
		} finally {
			standIn.remove();
		}
	}

	clear() {
		this.query = '';
	}
}

export const search = new Search();
