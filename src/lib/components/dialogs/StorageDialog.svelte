<script lang="ts">
	// "Free up space" (src-tauri/src/storage.rs). Sizes are what each button would really free:
	// mods are hard-linked from the download cache into profiles, so a file in both only counts once.
	import Dialog from '$lib/components/ui/Dialog.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Spinner from '$lib/components/ui/Spinner.svelte';
	import Icon from '@iconify/svelte';
	import * as api from '$lib/api';
	import type { StorageReport } from '$lib/api/storage';
	import { shortenFileSize } from '$lib/util';
	import { pushInfoToast } from '$lib/toast';
	import { confirm } from '@tauri-apps/plugin-dialog';

	type Props = { open?: boolean };
	let { open = $bindable(false) }: Props = $props();

	let report = $state<StorageReport | null>(null);
	let busy = $state<'unused' | 'cache' | 'gale' | null>(null);

	$effect(() => {
		if (open) refresh();
	});

	async function refresh() {
		report = null;
		try {
			report = await api.storage.report();
		} catch {}
	}

	function size(n: number) {
		return n > 0 ? shortenFileSize(n) : '0MB';
	}

	async function run(kind: 'unused' | 'cache' | 'gale', action: () => Promise<number>, what: string) {
		busy = kind;
		try {
			const freed = await action();
			pushInfoToast({ message: `${what} Freed ${size(freed)}.` });
			await refresh();
		} catch {
		} finally {
			busy = null;
		}
	}

	function freeUnused() {
		run('unused', api.storage.freeUnused, 'Removed unused mod files.');
	}

	async function clearCache() {
		if (!report) return;
		const ok = await confirm(
			`Delete every downloaded mod file?\n\nYour profiles keep working: they have their own copy of their mods. ` +
				`This frees about ${size(report.cacheFreeable)} now. Mods download again the next time you install or update them.`,
			{ title: 'Clear the whole mod cache', kind: 'warning' }
		);
		if (ok) run('cache', api.storage.clearAllCache, 'Cleared the mod cache.');
	}

	async function deleteGale() {
		const gale = report?.gale;
		if (!gale) return;
		const message = gale.installed
			? `Gale is still installed. This deletes ALL of Gale's profiles, settings and downloaded mods, and Gale will start over like new.\n\nYour Miles Mod Manager profiles are not touched.\n\nDelete Gale's files anyway?`
			: `Delete everything Gale left behind (its old profiles, settings and downloaded mods)?\n\nYour Miles Mod Manager profiles are not touched.`;
		const ok = await confirm(message, { title: "Delete Gale's files", kind: 'warning' });
		if (ok)
			run('gale', () => api.storage.deleteGaleLeftovers(gale.installed), "Deleted Gale's files.");
	}
</script>

<Dialog bind:open title="Free up space">
	{#if !report}
		<div class="text-primary-600 dark:text-primary-300 mt-3 flex items-center gap-2">
			<Spinner /> Checking how much space your mods use…
		</div>
	{:else}
		<div class="mt-3 flex flex-col gap-3">
			<!-- unused mod files -->
			<div class="dark:bg-primary-900 bg-primary-100 rounded-lg p-4">
				<div class="flex items-start gap-3">
					<Icon icon="mdi:broom" class="text-accent-500 mt-0.5 shrink-0 text-2xl" />
					<div class="grow">
						<p class="text-primary-900 font-semibold dark:text-white">
							Unused mod files <span class="text-accent-500 ml-1">{size(report.unusedSize)}</span>
						</p>
						<p class="text-primary-600 dark:text-primary-300 mt-1 text-sm">
							Mod downloads that none of your profiles use anymore: old versions left after
							updates, mods you uninstalled, and mods from deleted profiles{report.leftoverProfileCount >
							0
								? `, plus ${report.leftoverProfileCount} leftover profile folder${report.leftoverProfileCount === 1 ? '' : 's'}`
								: ''}. Safe to delete. Nothing you play with changes.
						</p>
					</div>
					<Button
						icon="mdi:broom"
						onclick={freeUnused}
						loading={busy === 'unused'}
						disabled={busy !== null || report.unusedSize === 0}>Free up space</Button
					>
				</div>
			</div>

			<!-- whole cache -->
			<div class="dark:bg-primary-900 bg-primary-100 rounded-lg p-4">
				<div class="flex items-start gap-3">
					<Icon icon="mdi:package-variant" class="text-primary-500 mt-0.5 shrink-0 text-2xl" />
					<div class="grow">
						<p class="text-primary-900 font-semibold dark:text-white">
							Whole mod cache <span class="text-primary-500 ml-1 text-sm font-normal"
								>{size(report.cacheSize)} on paper · frees about {size(report.cacheFreeable)}</span
							>
						</p>
						<p class="text-primary-600 dark:text-primary-300 mt-1 text-sm">
							A copy of every mod you've downloaded, so adding a mod to another profile is
							instant. Your profiles share these files, so clearing it frees only the unused
							part, and mods have to download again next time. Use this only if a mod seems
							broken or corrupted.
						</p>
					</div>
					<Button
						color="primary"
						icon="mdi:delete-sweep"
						onclick={clearCache}
						loading={busy === 'cache'}
						disabled={busy !== null || report.cacheSize === 0}>Clear cache</Button
					>
				</div>
			</div>

			<!-- Gale -->
			{#if report.gale}
				<div class="dark:bg-primary-900 bg-primary-100 rounded-lg p-4">
					<div class="flex items-start gap-3">
						<Icon icon="mdi:weather-windy" class="text-primary-500 mt-0.5 shrink-0 text-2xl" />
						<div class="grow">
							<p class="text-primary-900 font-semibold dark:text-white">
								Gale's leftover files <span class="text-accent-500 ml-1"
									>{size(report.gale.size)}</span
								>
							</p>
							<p class="text-primary-600 dark:text-primary-300 mt-1 text-sm">
								{#if report.gale.installed}
									Gale is still installed, so these are Gale's profiles and mods. If you've
									moved to Miles Mod Manager for good, uninstall Gale first (Windows Settings →
									Apps), then delete these.
								{:else}
									Gale's old profiles, settings and downloaded mods. Gale isn't installed
									anymore, so nothing uses them.
								{/if}
							</p>
							{#if report.gale.running}
								<p class="mt-1 flex items-center gap-1 text-sm text-red-400">
									<Icon icon="mdi:alert" /> Gale is open. Close it first.
								</p>
							{/if}
						</div>
						<Button
							color={report.gale.installed ? 'red' : 'accent'}
							icon="mdi:delete"
							onclick={deleteGale}
							loading={busy === 'gale'}
							disabled={busy !== null || report.gale.running}>Delete</Button
						>
					</div>
				</div>
			{/if}

			<p class="text-primary-500 dark:text-primary-400 text-xs">
				Deleting a profile now also deletes its mods' downloads when no other profile uses them.
			</p>
		</div>
	{/if}
</Dialog>
