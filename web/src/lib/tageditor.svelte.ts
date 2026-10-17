// The tag editor's state. Like the context menu, there is one editor for the
// whole app, drawn by <TagEditor> at the root of the layout.

import { api } from './api.ts';
import {
	autoNumber,
	buildRows,
	changed,
	edits,
	fillFromName,
	newRow,
	recase,
	validName,
	type Case,
	type Row
} from './tagedit.ts';
import { toasts } from './toast.svelte.ts';
import type { TaggedFile } from './types/TaggedFile.ts';

class TagEditor {
	open = $state(false);
	loading = $state(false);
	saving = $state(false);
	/** What is being edited, e.g. the album title. */
	title = $state('');
	/** Files that can be edited, in the order they were given (album order). */
	files = $state.raw<TaggedFile[]>([]);
	/** Files that can't be read, with why. */
	unreadable = $state.raw<TaggedFile[]>([]);
	rows = $state<Row[]>([]);

	dirty = $derived(this.rows.some(changed));
	/** Why saving isn't possible right now, if it isn't. */
	problem = $derived.by(() => {
		const live = this.rows.filter((r) => !r.removed);
		const bad = live.find((r) => !validName(r.name));
		if (bad) return bad.name ? `"${bad.name}" isn't a valid field name` : 'A field needs a name';
		const names = live.map((r) => r.name.toUpperCase());
		const twice = names.find((n, i) => names.indexOf(n) !== i);
		return twice ? `${twice} is listed twice` : null;
	});

	#request = 0;

	/** Opens the editor for `uris` (or a promise of them, e.g. an album's tracks). */
	async show(uris: string[] | Promise<string[]>, title: string) {
		const request = ++this.#request;
		this.open = true;
		this.loading = true;
		this.title = title;
		this.files = [];
		this.unreadable = [];
		this.rows = [];
		try {
			const all = await api.readTags(await uris);
			if (request !== this.#request) return;
			this.files = all.filter((f) => !f.error);
			this.unreadable = all.filter((f) => f.error);
			this.rows = buildRows(this.files);
		} catch {
			// The failure was already shown as a toast.
			if (request === this.#request) this.open = false;
		} finally {
			if (request === this.#request) this.loading = false;
		}
	}

	close(force = false) {
		if (this.saving) return;
		if (!force && this.dirty && !confirm('Discard your changes?')) return;
		this.open = false;
		this.#request++;
	}

	/** Sets the same text in every file. */
	setAll(row: Row, text: string) {
		row.text = row.text.map(() => text);
	}

	add(name = ''): Row {
		this.rows.push(newRow(name, this.files.length));
		return this.rows[this.rows.length - 1];
	}

	remove(row: Row) {
		if (row.original === undefined) this.rows = this.rows.filter((r) => r.id !== row.id);
		else row.removed = true;
	}

	recase(row: Row, mode: Case) {
		row.text = row.text.map((t) => recase(t, mode));
	}

	/** The row for a field, added when missing (or brought back when removed). */
	field(name: string): Row {
		const row = this.rows.find((r) => r.name.toUpperCase() === name.toUpperCase());
		if (!row) return this.add(name);
		row.removed = false;
		return row;
	}

	autoNumber() {
		const { number, total } = autoNumber(this.files.length);
		this.field('TRACKNUMBER').text = number;
		this.field('TOTALTRACKS').text = number.map(() => total);
	}

	/** Fills fields from file names; returns how many files matched. */
	fill(pattern: string): number {
		let matched = 0;
		this.files.forEach((file, i) => {
			const values = fillFromName(pattern, file.uri);
			if (!values) return;
			matched++;
			for (const [name, value] of Object.entries(values)) this.field(name).text[i] = value;
		});
		return matched;
	}

	async save() {
		const list = edits(this.files, this.rows);
		if (!list.length) return this.close(true);
		this.saving = true;
		try {
			const result = await api.writeTags(list);
			if (result.failed.length) {
				const first = result.failed[0];
				const more = result.failed.length > 1 ? ` (and ${result.failed.length - 1} more)` : '';
				toasts.show(`Couldn't save ${first.uri.split('/').pop()}: ${first.error}${more}`, 'error', 8000);
			} else {
				toasts.done(result.written === 1 ? 'Tags saved' : `Tags saved to ${result.written} files`);
			}
			this.saving = false;
			if (!result.failed.length) this.close(true);
		} catch {
			this.saving = false;
		}
	}
}

export const tagEditor = new TagEditor();
