<script lang="ts">
	// Publishing mods to SleepyMiles' site, Thunderstore-style. Opened with Ctrl+Alt+U.
	// Only works with a GitHub token that can write to SleepyMiles/miles-lc-mods, so it's
	// harmless if someone else opens it.
	import Dialog from '$lib/components/ui/Dialog.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import InputField from '$lib/components/ui/InputField.svelte';
	import Link from '$lib/components/ui/Link.svelte';
	import Icon from '@iconify/svelte';
	import { open as openFile } from '@tauri-apps/plugin-dialog';
	import * as api from '$lib/api';
	import type { SiteModInfo } from '$lib/api/site';
	import { Backend } from '$lib/types';
	import { shortenFileSize, SLEEPYMILES_SITE_URL } from '$lib/util';
	import { pushInfoToast } from '$lib/toast';

	type Props = { open: boolean };
	let { open = $bindable() }: Props = $props();

	const TOKEN_URL = 'https://github.com/settings/personal-access-tokens/new';

	let hasToken = $state<boolean | null>(null);
	let editingToken = $state(false);
	let tokenInput = $state('');
	let savingToken = $state(false);

	let info = $state<SiteModInfo | null>(null);
	let checking = $state(false);
	let publishing = $state(false);
	let publishedUrl = $state<string | null>(null);
	let error = $state<string | null>(null);

	$effect(() => {
		if (open) {
			api.thunderstore.hasToken(Backend.SleepyMiles).then((has) => {
				hasToken = has;
				editingToken = !has;
			});
		} else {
			info = null;
			publishedUrl = null;
			error = null;
		}
	});

	async function saveToken() {
		if (tokenInput.trim().length === 0) return;
		savingToken = true;
		try {
			await api.thunderstore.setToken(Backend.SleepyMiles, tokenInput.trim());
			tokenInput = '';
			hasToken = true;
			editingToken = false;
			if (info) await check(info.path);
		} finally {
			savingToken = false;
		}
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
		!!info && info.problems.length === 0 && hasToken === true && !publishing && !publishedUrl
	);
</script>

<Dialog title="Upload mod to my site" bind:open large>
	<div class="text-primary-300 flex flex-col gap-4">
		{#if editingToken}
			<div class="bg-primary-800 rounded-lg p-4">
				<p class="text-primary-100 font-semibold">GitHub token</p>
				<p class="mt-1 text-sm">
					The app publishes to <b>SleepyMiles/miles-lc-mods</b> with a GitHub token. Make one at
					<Link href={TOKEN_URL}>github.com/settings/personal-access-tokens/new</Link>: Repository
					access <b>Only select repositories → miles-lc-mods</b>, Permissions
					<b>Contents → Read and write</b>. It's stored in Windows Credential Manager.
				</p>
				<div class="mt-3 flex gap-2">
					<InputField
						class="grow"
						placeholder="github_pat_..."
						bind:value={tokenInput}
						onsubmit={saveToken}
					/>
					<Button icon="mdi:key" onclick={saveToken} loading={savingToken}>Save token</Button>
				</div>
				{#if hasToken}
					<button class="mt-2 text-sm underline" onclick={() => (editingToken = false)}
						>Keep the current token</button
					>
				{/if}
			</div>
		{/if}

		<div class="flex items-center gap-2">
			<Button icon="mdi:folder-zip" color="primary" onclick={chooseZip} loading={checking}
				>{info ? 'Choose a different zip' : 'Choose mod zip'}</Button
			>
			{#if hasToken && !editingToken}
				<button class="ml-auto text-sm underline" onclick={() => (editingToken = true)}
					>Change token</button
				>
			{/if}
		</div>

		{#if info}
			<div class="bg-primary-800 flex gap-4 rounded-lg p-4">
				{#if info.icon}
					<img src={info.icon} alt="" class="size-24 shrink-0 rounded-md" />
				{:else}
					<div class="bg-primary-700 size-24 shrink-0 rounded-md"></div>
				{/if}
				<div class="min-w-0">
					<p class="text-primary-100 text-xl font-semibold">
						{info.name || info.fileName}
						{#if info.version}<span class="text-accent-400 ml-2 text-base">v{info.version}</span
							>{/if}
					</p>
					<p class="mt-1">{info.description}</p>
					<p class="mt-2 text-sm">
						{shortenFileSize(info.size)}
						{#if info.tag}
							· publishes as <b>{info.assetName}</b>{/if}
					</p>
					{#if info.dependencies.length > 0}
						<p class="mt-1 text-sm">
							Needs: {info.dependencies.map(depName).join(', ')}
						</p>
					{/if}
				</div>
			</div>

			{#each info.problems as problem}
				<p class="flex items-start gap-2 text-red-400">
					<Icon icon="mdi:close-circle" class="mt-0.5 shrink-0" />{problem}
				</p>
			{/each}
			{#each info.notes as note}
				<p class="flex items-start gap-2 text-sm">
					<Icon icon="mdi:information" class="mt-0.5 shrink-0" />{note}
				</p>
			{/each}
			{#if info.problems.length === 0 && !publishedUrl}
				<p class="flex items-start gap-2 text-green-400">
					<Icon icon="mdi:check-circle" class="mt-0.5 shrink-0" />Ready to publish.
				</p>
			{/if}
		{/if}

		{#if error}
			<p class="flex items-start gap-2 text-red-400">
				<Icon icon="mdi:alert" class="mt-0.5 shrink-0" />{error}
			</p>
		{/if}

		{#if publishedUrl}
			<div class="bg-primary-800 rounded-lg p-4">
				<p class="font-semibold text-green-400">Published.</p>
				<p class="mt-1 text-sm">
					Your site and the app's mod list update in a minute or two (the site's publish workflow is
					running now).
				</p>
				<div class="mt-2 flex gap-4 text-sm">
					<Link href={publishedUrl}>Open the release</Link>
					<Link href={SLEEPYMILES_SITE_URL}>Open your site</Link>
					<Link href="https://github.com/SleepyMiles/miles-lc-mods/actions">Check the workflow</Link
					>
				</div>
			</div>
		{/if}

		<div class="flex justify-end gap-2">
			<Button color="primary" onclick={() => (open = false)}>Close</Button>
			<Button icon="mdi:upload" onclick={publish} disabled={!canPublish} loading={publishing}
				>Publish</Button
			>
		</div>
	</div>
</Dialog>
