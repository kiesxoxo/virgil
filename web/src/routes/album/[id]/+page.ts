import { api } from '#lib/api.ts';
import type { PageLoad } from './$types';

// Not awaited: the header renders at once from the cached album list while the tracks load.
export const load: PageLoad = ({ params }) => ({ id: params.id, detail: api.album(params.id) });
