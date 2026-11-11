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
	import Checkbox from '$lib/components/ui/Checkbox.svelte';
	import type { ConfigCleanupScan } from '$lib/api/storage';

	type Props = { open?: boolean };
	let { open = $bindable(false) }: Props = $props();

	let report = $state<StorageReport | null>(null);
	let busy = $state<'unused' | 'cache' | 'gale' | null>(null);

	// unused config (active profile)
	let cfg = $state<ConfigCleanupScan | null>(null);
	let cfgChecking = $state(false);
	let cfgRemoving = $state(false);
	let keepFiles = $state<Record<string, boolean>>({});
	let keepSections = $state<Record<string, boolean>>({});

	const sectionKey = (s: { path: string; section: string }) => `${s.path}\n${s.section}`;

	$effect(() => {
		if (open) {
			refresh();
			cfg = null;
		}
	});

	async function checkConfig() {
		cfgChecking = true;
		keepFiles = {};
		keepSections = {};
		try {
			cfg = await api.storage.scanUnusedConfig();
		} catch {
		} finally {
			cfgChecking = false;
		}
	}

	let cfgChosenFiles = $derived(cfg ? cfg.files.filter((f) => !keepFiles[f.path]) : []);
	let cfgChosenSections = $derived(
		cfg ? cfg.sections.filter((s) => !keepSections[sectionKey(s)]) : []
	);

	async function removeConfig() {
		if (!cfg) return;
		cfgRemoving = true;
		try {
			const result = await api.storage.removeUnusedConfig(
				cfgChosenFiles.map((f) => f.path),
				cfgChosenSections
			);
			pushInfoToast({
				message: `Removed ${result.filesRemoved} config file${result.filesRemoved === 1 ? '' : 's'} and ${result.sectionsRemoved} unused section${result.sectionsRemoved === 1 ? '' : 's'}. A backup was saved first.`
			});
			await checkConfig();
		} catch {
		} finally {
			cfgRemoving = false;
		}
	}

	function fileName(path: string) {
		return path.split('/').pop() ?? path;
	}

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

			<!-- unused config -->
			<div class="dark:bg-primary-900 bg-primary-100 rounded-lg p-4">
				<div class="flex items-start gap-3">
					<Icon icon="mdi:file-cog" class="text-primary-500 mt-0.5 shrink-0 text-2xl" />
					<div class="grow">
						<p class="text-primary-900 font-semibold dark:text-white">
							Unused config {#if cfg}<span class="text-primary-500 ml-1 text-sm font-normal"
									>profile {cfg.profile}</span
								>{/if}
						</p>
						<p class="text-primary-600 dark:text-primary-300 mt-1 text-sm">
							Config files of mods that are no longer in this profile, and LethalLevelLoader
							settings for interiors and moons you removed. Settings of mods you still have are
							never touched, and a backup is made first. Config files are small, so this is
							mostly about tidiness.
						</p>
					</div>
					<Button
						color="primary"
						icon="mdi:magnify"
						onclick={checkConfig}
						loading={cfgChecking}
						disabled={busy !== null || cfgRemoving}>{cfg ? 'Check again' : 'Check'}</Button
					>
				</div>

				{#if cfg}
					{#each cfg.notes as note}
						<p class="text-primary-600 dark:text-primary-300 mt-3 flex items-start gap-2 text-sm">
							<Icon icon="mdi:information" class="mt-0.5 shrink-0" />{note}
						</p>
					{/each}

					{#if cfg.files.length === 0 && cfg.sections.length === 0}
						<p class="mt-3 flex items-center gap-2 text-sm text-green-500">
							<Icon icon="mdi:check-circle" /> Nothing unused in {cfg.checkedFiles} config files.
						</p>
					{:else}
						<div class="mt-3 max-h-72 overflow-y-auto pr-1 text-sm">
							{#if cfg.files.length > 0}
								<p class="text-primary-700 dark:text-primary-200 mb-1 font-medium">
									Files of removed mods
								</p>
								{#each cfg.files as file (file.path)}
									<label class="flex items-center gap-2 py-0.5">
										<Checkbox
											checked={!keepFiles[file.path]}
											onCheckedChange={(v) => (keepFiles[file.path] = !v)}
										/>
										<span class="text-primary-800 dark:text-primary-100 truncate">{file.plugin}</span>
										<span class="text-primary-500 dark:text-primary-400 ml-auto shrink-0 truncate"
											>{fileName(file.path)}</span
										>
									</label>
								{/each}
							{/if}
							{#if cfg.sections.length > 0}
								<p class="text-primary-700 dark:text-primary-200 mt-2 mb-1 font-medium">
									Removed interiors and moons (LethalLevelLoader.cfg)
								</p>
								{#each cfg.sections as section (sectionKey(section))}
									<label class="flex items-center gap-2 py-0.5">
										<Checkbox
											checked={!keepSections[sectionKey(section)]}
											onCheckedChange={(v) => (keepSections[sectionKey(section)] = !v)}
										/>
										<span class="text-primary-800 dark:text-primary-100 truncate"
											>{section.section}</span
										>
										<span class="text-primary-500 dark:text-primary-400 ml-auto shrink-0"
											>{section.settings} setting{section.settings === 1 ? "" : "s"}</span
										>
									</label>
								{/each}
								<p class="text-primary-500 dark:text-primary-400 mt-1 text-xs">
									If one of these is from a mod you only turned off for now, untick it, or its
									LethalLevelLoader settings go back to default when you turn it on again.
								</p>
							{/if}
						</div>
						<div class="mt-3 flex justify-end">
							<Button
								icon="mdi:delete-sweep"
								onclick={removeConfig}
								loading={cfgRemoving}
								disabled={cfgChosenFiles.length + cfgChosenSections.length === 0}
								>Remove {cfgChosenFiles.length + cfgChosenSections.length} selected</Button
							>
						</div>
					{/if}
				{/if}
			</div>

			<p class="text-primary-500 dark:text-primary-400 text-xs">
				Deleting a profile now also deletes its mods' downloads when no other profile uses them.
			</p>
		</div>
	{/if}
</Dialog>
