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

export type DeviceLogin = {
	deviceCode: string;
	userCode: string;
	verificationUri: string;
	interval: number;
	expiresIn: number;
};

export type LoginPoll =
	| { status: 'pending'; interval: number }
	| { status: 'loggedIn'; login: string };

/** Starts "Log in with GitHub" (device flow). */
export const loginStart = () => invoke<DeviceLogin>('site_login_start');
/** One check whether the code was entered on GitHub. Only SleepyMiles' account is accepted. */
export const loginPoll = (deviceCode: string, interval: number) =>
	invoke<LoginPoll>('site_login_poll', { deviceCode, interval });
/** The logged-in GitHub account, or null. */
export const account = () => invoke<string | null>('site_account');
export const logout = () => invoke('site_logout');
