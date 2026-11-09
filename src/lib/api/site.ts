import { invoke } from '$lib/invoke';

/** A mod zip checked for publishing to SleepyMiles' site (see src-tauri/src/site_upload.rs). */
export type SiteModInfo = {
	path: string;
	fileName: string;
	size: number;
	name: string;
	version: string;
	description: string;
	websiteUrl: string;
	dependencies: string[];
	icon: string | null;
	tag: string;
	assetName: string;
	problems: string[];
	notes: string[];
	alreadyPublished: boolean;
};

export const inspectMod = (path: string) => invoke<SiteModInfo>('inspect_site_mod', { path });
/** Creates the GitHub release, uploads the zip and publishes it. Returns the release page URL. */
export const publishMod = (path: string) => invoke<string>('publish_site_mod', { path });
