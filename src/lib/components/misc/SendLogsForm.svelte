<script lang="ts">
	// "Send logs to Miles": zips the game logs and posts them to Miles's Discord (src-tauri/src/send_logs.rs).
	// Used in the Send logs dialog (File/Help menu) and on the upload page.
	import Button from '$lib/components/ui/Button.svelte';
	import InputField from '$lib/components/ui/InputField.svelte';
	import ResizableInputField from '$lib/components/ui/ResizableInputField.svelte';
	import Spinner from '$lib/components/ui/Spinner.svelte';
	import Icon from '@iconify/svelte';
	import * as api from '$lib/api';
	import type { LogsInfo } from '$lib/api/logs';
	import { shortenFileSize } from '$lib/util';
	import { pushInfoToast } from '$lib/toast';

	type Props = { active?: boolean };
	let { active = true }: Props = $props();

	const NAME_KEY = 'sendLogs.name';

	let info = $state<LogsInfo | null>(null);
	let name = $state(readName());
	let message = $state('');
	let sending = $state(false);
	let saving = $state(false);
	let sent = $state(false);
	let cooldown = $state(0);
	let timer: ReturnType<typeof setInterval> | null = null;

	function readName() {
		try {
			return localStorage.getItem(NAME_KEY) ?? '';
		} catch {
			return '';
		}
	}

	function rememberName() {
		try {
			localStorage.setItem(NAME_KEY, name.trim());
		} catch {}
	}

	$effect(() => {
		if (active) {
			sent = false;
			info = null;
			api.logs.info().then((i) => (info = i));
		}
	});

	function ago(secs: number | null) {
		if (!secs) return '';
		const s = Math.max(0, Date.now() / 1000 - secs);
		if (s < 90) return 'just now';
		if (s < 3600) return `${Math.round(s / 60)} min ago`;
		if (s < 86400 * 2) return `${Math.round(s / 3600)} h ago`;
		return `${Math.round(s / 86400)} days ago`;
	}

	let found = $derived(info?.files.filter((f) => f.exists) ?? []);
	let canSend = $derived(
		!!info && found.length > 0 && name.trim().length > 0 && !sending && cooldown === 0
	);

	async function send() {
		sending = true;
		rememberName();
		try {
			await api.logs.send(name, message);
			sent = true;
			message = '';
			startCooldown();
			pushInfoToast({ message: 'Logs sent to Miles.' });
		} catch {
			// the error toast already says what went wrong
		} finally {
			sending = false;
		}
	}

	function startCooldown() {
		cooldown = 30;
		if (timer) clearInterval(timer);
		timer = setInterval(() => {
			cooldown -= 1;
			if (cooldown <= 0 && timer) {
				clearInterval(timer);
				timer = null;
			}
		}, 1000);
	}

	async function saveZip() {
		saving = true;
		rememberName();
		try {
			const path = await api.logs.save(name, message);
			pushInfoToast({ message: `Saved ${path.split(/[\\/]/).pop()} to your Downloads folder.` });
		} catch {
		} finally {
			saving = false;
		}
	}
</script>

	<p class="text-primary-600 dark:text-primary-300 mt-1">
		Sends the game's log files to Miles's Discord, so he can see what went wrong. Do this right
		after the problem happens (you can leave the game open, or close it first).
	</p>

	<div class="dark:bg-primary-900 bg-primary-100 mt-3 rounded-lg p-3">
		{#if !info}
			<div class="text-primary-600 dark:text-primary-300 flex items-center gap-2">
				<Spinner /> Finding your logs…
			</div>
		{:else}
			<p class="text-primary-700 dark:text-primary-200 mb-2 text-sm">
				Profile <b class="text-primary-900 dark:text-white">{info.profile}</b> · {info.modCount} mods
				(the mod list is included too)
			</p>
			{#each info.files as file}
				<div class="flex items-center gap-2 py-0.5 text-sm">
					{#if file.exists}
						<Icon icon="mdi:check-circle" class="shrink-0 text-green-500" />
						<span class="text-primary-800 dark:text-primary-100 grow truncate">{file.label}</span>
						<span class="text-primary-500 dark:text-primary-400 shrink-0"
							>{shortenFileSize(file.size)} · {ago(file.modified)}</span
						>
					{:else}
						<Icon icon="mdi:minus-circle" class="text-primary-500 shrink-0" />
						<span class="text-primary-500 dark:text-primary-400 grow truncate"
							>{file.label} (not found)</span
						>
					{/if}
				</div>
			{/each}
			{#if found.length === 0}
				<p class="mt-2 text-sm text-red-400">
					No logs yet. Launch the game with this profile once, then come back.
				</p>
			{/if}
		{/if}
	</div>

	<div class="mt-3 flex flex-col gap-2">
		<label class="text-primary-700 dark:text-primary-200 text-sm font-medium" for="sendlogs-name"
			>Your name</label
		>
		<InputField id="sendlogs-name" bind:value={name} placeholder="So Miles knows who it's from" />
		<label class="text-primary-700 dark:text-primary-200 mt-1 text-sm font-medium" for="sendlogs-msg"
			>What happened? (optional)</label
		>
		<ResizableInputField
			id="sendlogs-msg"
			bind:value={message}
			placeholder="e.g. the game froze when we landed on Rend"
			style="height: 6rem"
		/>
	</div>

	{#if sent}
		<p class="mt-3 flex items-center gap-2 text-green-500">
			<Icon icon="mdi:check-circle" /> Sent! Miles has your logs now.
		</p>
	{/if}

	<div class="mt-4 flex flex-wrap items-center justify-end gap-2">
		<Button
			color="primary"
			icon="mdi:folder-zip"
			onclick={saveZip}
			loading={saving}
			disabled={!info || found.length === 0}>Save as zip instead</Button
		>
		<Button icon="mdi:send" onclick={send} loading={sending} disabled={!canSend}
			>{cooldown > 0 ? `Send again in ${cooldown}s` : 'Send to Miles'}</Button
		>
	</div>
	<p class="text-primary-500 dark:text-primary-400 mt-2 text-xs">
		Logs can contain your Windows user name and Steam name. They only go to Miles's Discord channel.
	</p>
