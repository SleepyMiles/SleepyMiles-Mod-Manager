import { invoke } from '$lib/invoke';

/** See src-tauri/src/storage.rs. All sizes are bytes that would really be freed. */
export type GaleLeftovers = {
	size: number;
	paths: string[];
	installed: boolean;
	running: boolean;
};

export type StorageReport = {
	unusedSize: number;
	unusedModCount: number;
	leftoverProfileCount: number;
	cacheSize: number;
	cacheFreeable: number;
	gale: GaleLeftovers | null;
};

export const report = () => invoke<StorageReport>('storage_report');
export const freeUnused = () => invoke<number>('free_unused_space');
export const clearAllCache = () => invoke<number>('clear_all_cache');
export const deleteGaleLeftovers = (evenIfInstalled: boolean) =>
	invoke<number>('delete_gale_leftovers', { evenIfInstalled });
