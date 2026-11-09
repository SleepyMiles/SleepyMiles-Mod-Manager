<script lang="ts">
	// Publishing mods to SleepyMiles' site, Thunderstore-style.
	// Needs "Log in with GitHub"; only the SleepyMiles account is accepted (checked in site_upload.rs).
	import Button from '$lib/components/ui/Button.svelte';
	import Link from '$lib/components/ui/Link.svelte';
	import Spinner from '$lib/components/ui/Spinner.svelte';
	import Icon from '@iconify/svelte';
	import { open as openFile } from '@tauri-apps/plugin-dialog';
	import { open as shellOpen } from '@tauri-apps/plugin-shell';
	import { writeText } from '@tauri-apps/plugin-clipboard-manager';
	import { onDestroy, onMount } from 'svelte';
	import * as api from '$lib/api';
	import type { DeviceLogin, SiteModInfo } from '$lib/api/site';
	import { shortenFileSize, SLEEPYMILES_SITE_URL } from '$lib/util';
	import { pushInfoToast } from '$lib/toast';

	let account = $state<string | null>(null);
	let checkingAccount = $state(true);

	let device = $state<DeviceLogin | null>(null);
	let loginError = $state<string | null>(null);
	let pollTimer: ReturnType<typeof setTimeout> | null = null;

	let info = $state<SiteModInfo | null>(null);
	let checking = $state(false);
	let publishing = $state(false);
	let publishedUrl = $state<string | null>(null);
	let error = $state<string | null>(null);

	onMount(async () => {
		try {
			account = await api.site.account();
		} finally {
			checkingAccount = false;
		}
	});

	onDestroy(stopPolling);

	function stopPolling() {
		if (pollTimer) clearTimeout(pollTimer);
		pollTimer = null;
	}

	async function login() {
		loginError = null;
		stopPolling();
		try {
			device = await api.site.loginStart();
		} catch (e: any) {
			loginError = e?.message ?? String(e);
			return;
		}
		await writeText(device.userCode).catch(() => {});
		await shellOpen(device.verificationUri).catch(() => {});
		schedulePoll(device.interval);
	}

	function schedulePoll(seconds: number) {
		pollTimer = setTimeout(poll, seconds * 1000);
	}

	async function poll() {
		if (!device) return;
		try {
			const result = await api.site.loginPoll(device.deviceCode, device.interval);
			if (result.status === 'loggedIn') {
				account = result.login;
				device = null;
				if (info) await check(info.path);
			} else {
				device.interval = result.interval;
				schedulePoll(result.interval);
			}
		} catch (e: any) {
			loginError = e?.message ?? String(e);
			device = null;
		}
	}

	function cancelLogin() {
		stopPolling();
		device = null;
	}

	async function logout() {
		await api.site.logout();
		account = null;
	}

	async function chooseZip() {
		const path = await openFile({
			title: 'Choose the mod zip to publish',
			filters: [{ name: 'Mod zip', extensions: ['zip'] }]
		});
		if (path === null) return;
		await check(path);
	}

	async function check(path: string) {
		checking = true;
		error = null;
		publishedUrl = null;
		try {
			info = await api.site.inspectMod(path);
		} catch (e: any) {
			info = null;
			error = e?.message ?? String(e);
		} finally {
			checking = false;
		}
	}

	async function publish() {
		if (!info) return;
		publishing = true;
		error = null;
		try {
			publishedUrl = await api.site.publishMod(info.path);
			pushInfoToast({ message: `Published ${info.name} ${info.version} to your site.` });
		} catch (e: any) {
			error = e?.message ?? String(e);
		} finally {
			publishing = false;
		}
	}

	function depName(dep: string) {
		const parts = dep.split('-');
		return parts.length === 3 ? `${parts[1].replace(/_/g, ' ')} ${parts[2]}` : dep;
	}

	let canPublish = $derived(
		!!info && info.problems.length === 0 && !!account && !publishing && !publishedUrl
	);
</script>

<div class="relative mx-auto flex w-full max-w-4xl flex-col gap-4 overflow-y-auto px-6 py-4">
	<div>
		<h1 class="text-primary-900 text-2xl font-semibold dark:text-white">
			Upload to SleepyMiles' site
		</h1>
		<p class="text-primary-600 dark:text-primary-300 mt-1">
			Publish a new mod, a fork, or an update. It shows up on
			<Link href={SLEEPYMILES_SITE_URL}>the site</Link> and in everyone's mod list a minute or two later.
		</p>
	</div>

	<!-- GitHub account -->
	<div class="dark:bg-primary-900 bg-primary-100 rounded-lg p-4">
		{#if checkingAccount}
			<div class="text-primary-600 dark:text-primary-300 flex items-center gap-2">
				<Spinner /> Checking your GitHub login…
			</div>
		{:else if account}
			<div class="flex items-center gap-3">
				<Icon icon="mdi:github" class="text-primary-900 text-3xl dark:text-white" />
				<p class="text-primary-700 dark:text-primary-200 grow">
					Logged in as <b class="text-primary-900 dark:text-white">{account}</b>
				</p>
				<Button color="primary" icon="mdi:logout" onclick={logout}>Log out</Button>
			</div>
		{:else if device}
			<p class="text-primary-700 dark:text-primary-200">
				Enter this code on the GitHub page that just opened (it's already copied):
			</p>
			<div class="mt-3 flex flex-wrap items-center gap-3">
				<span
					class="text-primary-900 bg-primary-100 dark:bg-primary-900 rounded-lg px-4 py-2 font-mono text-3xl font-bold tracking-widest dark:text-white"
					>{device.userCode}</span
				>
				<Button
					color="primary"
					icon="mdi:open-in-new"
					onclick={() => device && shellOpen(device.verificationUri)}>Open GitHub again</Button
				>
				<Button color="primary" onclick={cancelLogin}>Cancel</Button>
			</div>
			<div class="text-primary-600 dark:text-primary-300 mt-3 flex items-center gap-2 text-sm">
				<Spinner /> Waiting for you to approve it on GitHub…
			</div>
		{:else}
			<div class="flex flex-wrap items-center gap-3">
				<Icon icon="mdi:github" class="text-primary-900 text-3xl dark:text-white" />
				<p class="text-primary-700 dark:text-primary-200 grow">
					Log in with GitHub to publish. Only SleepyMiles' account can upload.
				</p>
				<Button icon="mdi:github" onclick={login}>Log in with GitHub</Button>
			</div>
		{/if}
		{#if loginError}
			<p class="mt-3 flex items-start gap-2 text-red-400">
				<Icon icon="mdi:alert" class="mt-0.5 shrink-0" />{loginError}
			</p>
		{/if}
	</div>

	<!-- Mod zip -->
	<div class="flex items-center gap-2">
		<Button icon="mdi:folder-zip" color="primary" onclick={chooseZip} loading={checking}
			>{info ? 'Choose a different zip' : 'Choose mod zip'}</Button
		>
	</div>

	{#if info}
		<div class="dark:bg-primary-900 bg-primary-100 flex gap-4 rounded-lg p-4">
			{#if info.icon}
				<img src={info.icon} alt="" class="size-28 shrink-0 rounded-md" />
			{:else}
				<div class="dark:bg-primary-700 bg-primary-300 size-28 shrink-0 rounded-md"></div>
			{/if}
			<div class="min-w-0">
				<p class="text-primary-900 text-xl font-semibold dark:text-white">
					{info.name || info.fileName}
					{#if info.version}<span class="text-accent-500 dark:text-accent-400 ml-2 text-base"
							>v{info.version}</span
						>{/if}
				</p>
				<p class="text-primary-700 dark:text-primary-200 mt-1">{info.description}</p>
				<p class="text-primary-600 dark:text-primary-300 mt-2 text-sm">
					{shortenFileSize(info.size)}
					{#if info.tag}
						· publishes as <b>{info.assetName}</b>{/if}
				</p>
				{#if info.dependencies.length > 0}
					<p class="text-primary-600 dark:text-primary-300 mt-1 text-sm">
						Needs: {info.dependencies.map(depName).join(', ')}
					</p>
				{/if}
			</div>
		</div>

		<div class="flex flex-col gap-1.5">
			{#each info.problems as problem}
				<p class="flex items-start gap-2 text-red-400">
					<Icon icon="mdi:close-circle" class="mt-0.5 shrink-0" />{problem}
				</p>
			{/each}
			{#each info.notes as note}
				<p class="text-primary-600 dark:text-primary-300 flex items-start gap-2 text-sm">
					<Icon icon="mdi:information" class="mt-0.5 shrink-0" />{note}
				</p>
			{/each}
			{#if info.problems.length === 0 && !publishedUrl}
				<p class="flex items-start gap-2 text-green-500">
					<Icon icon="mdi:check-circle" class="mt-0.5 shrink-0" />{account
						? 'Ready to publish.'
						: 'Looks good. Log in with GitHub to publish.'}
				</p>
			{/if}
		</div>
	{/if}

	{#if error}
		<p class="flex items-start gap-2 text-red-400">
			<Icon icon="mdi:alert" class="mt-0.5 shrink-0" />{error}
		</p>
	{/if}

	{#if publishedUrl}
		<div class="dark:bg-primary-900 bg-primary-100 rounded-lg p-4">
			<p class="font-semibold text-green-500">Published.</p>
			<p class="text-primary-700 dark:text-primary-200 mt-1 text-sm">
				The site's publish workflow is running now. Your site and everyone's mod list update in a
				minute or two.
			</p>
			<div class="mt-2 flex flex-wrap gap-4 text-sm">
				<Link href={publishedUrl}>Open the release</Link>
				<Link href={SLEEPYMILES_SITE_URL}>Open your site</Link>
				<Link href="https://github.com/SleepyMiles/miles-lc-mods/actions">Check the workflow</Link>
			</div>
		</div>
	{/if}

	<div class="flex justify-end">
		<Button
			icon="mdi:upload"
			size="lg"
			onclick={publish}
			disabled={!canPublish}
			loading={publishing}>Publish</Button
		>
	</div>
</div>
