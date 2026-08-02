<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { isMac, compactViewMode, statusMessage, onSearchPage, userPreferences, syncStatus, showIconGrid } from '$lib/stores';
	import {
		documentsShown,
		searchInProgress,
		dbCreationInProgress,
		windowBlurred
	} from '$lib/stores';
	import { selectAllRows } from '$lib/utils/fileUtils';
	import { invoke } from '@tauri-apps/api/core';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { trackEvent } from '@aptabase/web';
	import { appStatistics, type AppStatistics } from '$lib/stores';
	
	let darkMode = false;
	let fileSyncFinished = false;
	let syncCoolingPeriod = false;
	let userAskedToDisable = false;
	let appMode = 'menubar';
	let numFiles: number = 0;
	let showingResults: boolean = false;
	let dbReady = false;
	let filesAddedCount = 0;
	let scanSpeed = 0; // files per second
	let lastFilesCount = 0;
	let lastFilesTimestamp = 0;
	let parseProgress = 0; // files parsed in the text/OCR phase
	let parseTotal = 0; // total files to parse in that phase
	let stats: AppStatistics | null = null;
	let countdownSeconds = 0;
	let statsTimer: ReturnType<typeof setInterval> | undefined;

	function showStatusBarMenu(option: string) {
		// invoke("open_context_menu", {option:"statusbar"}).then((res) => {});
		goto("/magic/");
	}

	function reCalculateOnDocsShownChange() {
		numFiles = $documentsShown.length;
		showingResults = numFiles > 20;
		selectAllRows(true); // remove selected class from all rows
	}

	$: $documentsShown && reCalculateOnDocsShownChange();

	function toggleCompactViewMode() {
		$compactViewMode = !$compactViewMode;
		trackEvent('click:toggleCompactViewMode', { compactViewMode: $compactViewMode });
		if ($compactViewMode === true) {
			document.querySelectorAll('td').forEach((el) => {
				el.classList.add('compact-view');
			});
			document.querySelectorAll('th').forEach((el) => {
				el.classList.add('compact-view');
			});
		} else {
			document.querySelectorAll('td').forEach((el) => {
				el.classList.remove('compact-view');
			});
			document.querySelectorAll('th').forEach((el) => {
				el.classList.remove('compact-view');
			});
		}
	}

	async function toggleBackgroundTextProcessing() {
		trackEvent('click:toggleBackgroundTextProcessing', { syncStats: $syncStatus });
		// if $syncStatus is true, switch_off is true, so we want to stop the sync
		invoke("run_file_sync", {switchOff: $syncStatus, filePaths: []});
		if ($syncStatus) {
			$statusMessage = "Stopping background scan...";
			setTimeout(() => {$statusMessage = "";}, 3000);
			userAskedToDisable = true;
		} else {
			$statusMessage = "Starting background scan...";
			setTimeout(() => {$statusMessage = "";}, 3000);
		}
		// disable `bg-sync-btn` for 5 seconds
		// this allows any pending processes to complete when stopping the sync
		syncCoolingPeriod = true;
		setTimeout(() => {
			// if userAskedToDisable and sync is still running, then keep the cooling period on
			if (userAskedToDisable && $syncStatus) {
				syncCoolingPeriod = true;
			} else {
				syncCoolingPeriod = false;
			}
		}, 5000);
	}

	// if $syncStatus is false, then reset cooling period and userAskedToDisable
	$: if (!$syncStatus) {
		userAskedToDisable = false;
		syncCoolingPeriod = false;
	}

	function goToSearch(from_onboarding: boolean = false) {
		trackEvent('click:goToSearch');
		if (from_onboarding) {
			// start background processing to get file contents
			toggleBackgroundTextProcessing();
			goto('/search?highlight-search-bar=true&q=this%20month');
		} else {
			goto('/search');
		}
	}

	function update_files_added_count(filesAddedPayload: Payload) {
		filesAddedCount = parseInt(filesAddedPayload.data);
		// Compute the scan speed from the files counted since the previous event
		// and the wall-clock time that elapsed between the two events. Events
		// arrive roughly every 500 files, which gives a smooth-enough average
		// without over-sampling.
		const now = Date.now();
		if (lastFilesTimestamp > 0 && filesAddedCount > lastFilesCount) {
			const deltaFiles = filesAddedCount - lastFilesCount;
			const deltaMs = now - lastFilesTimestamp;
			if (deltaMs > 0) {
				scanSpeed = (deltaFiles / deltaMs) * 1000;
			}
		}
		lastFilesCount = filesAddedCount;
		lastFilesTimestamp = now;
		if (filesAddedPayload.message == "files_added_complete") {
			$dbCreationInProgress = false;
			dbReady = true;
		}
	}

	async function refreshStatistics() {
		try {
			stats = await invoke<AppStatistics>("get_app_statistics");
			$appStatistics = stats;
			if (stats && stats.next_scan_in_seconds >= 0) {
				countdownSeconds = stats.next_scan_in_seconds;
			}
		} catch (err) {
			console.warn("Failed to load app statistics", err);
		}
	}

	function formatCountdown(totalSeconds: number): string {
		if (totalSeconds < 0) return '';
		const m = Math.floor(totalSeconds / 60);
		const s = totalSeconds % 60;
		return `${m}:${s.toString().padStart(2, '0')}`;
	}

	function formatSize(bytes: number): string {
		if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + ' GB';
		if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + ' MB';
		if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB';
		return bytes + ' B';
	}

	function statusLabel(status: string): string {
		return status.charAt(0).toUpperCase() + status.slice(1);
	}

	// FOR ONBOARDING PROCESS
	let unlisten_files_added:UnlistenFn;

	// FOR SYNC STATUS WHEN CLICKED
	let unlisten_sync_status:UnlistenFn;
	// FOR FILE SYNC FINISHED
	let unlisten_file_sync_finished:UnlistenFn;
	// FOR TEXT/OCR PARSE PHASE PROGRESS
	let unlisten_scan_progress:UnlistenFn;

	onMount(async () => {
		invoke("get_os").then((res) => {
			// @ts-ignore
			if (res == "macos") {
				$isMac = true;
			} else {
				$isMac = false;
			}
		});

		invoke("get_user_preferences_state").then((res) => {
			// @ts-ignore
			$userPreferences = res;
		});

		// Listener for when every batch (500) of files gets added to the database
		unlisten_files_added = await listen<Payload>('files-added', (event: any) => {
			update_files_added_count(event.payload);
			if (event.payload.message === "files_added_complete") {
				$userPreferences.onboarding_done = true;
			}
		});
		// Listener for sync status changes from inside the Tokio process in db_sync.rs
		unlisten_sync_status = await listen<Payload>('sync-status', (event: any) => {
			$syncStatus = event.payload.data === 'true';
			if (event.payload.data === 'true') {
				// Reset the scan-speed window for a fresh scan: the backend resets
				// its per-scan counter, so we must too before the next files-added.
				lastFilesCount = 0;
				lastFilesTimestamp = 0;
				scanSpeed = 0;
			}
		});
		// Listener for when the db_sync process is done
		unlisten_file_sync_finished = await listen<Payload>('file-sync-finished', (event: any) => {
			fileSyncFinished = event.payload.data === 'true';
		});
		// Listener for the text/OCR parsing phase progress (scan-progress).
		// payload message "scan_started" carries the total, "scan_progress" carries
		// "processed/total" as its data string.
		unlisten_scan_progress = await listen<Payload>('scan-progress', (event: any) => {
			if (event.payload.message === 'scan_started') {
				parseProgress = 0;
				parseTotal = Number(event.payload.data);
			} else if (event.payload.message === 'scan_progress') {
				const [processed, total] = event.payload.data.split('/');
				parseProgress = Number(processed) || 0;
				parseTotal = Number(total) || 0;
			}
		});

		// Ask for sync status on each mount to keep it updated in case of page changes
		$syncStatus = await invoke("get_sync_status") === 'true';

		// Load the status bar statistics, then refresh them periodically and tick
		// the "next scan" countdown every second.
		await refreshStatistics();
		statsTimer = setInterval(() => {
			if (stats && stats.auto_sync_enabled && stats.status !== 'scanning' && countdownSeconds > 0) {
				countdownSeconds -= 1;
			} else if (stats && (stats.status === 'scanning' || countdownSeconds <= 0)) {
				// Re-fetch so counts/size/state stay accurate after a scan.
				refreshStatistics();
			}
		}, 1000);

		// on renderer launch
		appMode = "window";
	});

	onDestroy(() => {
		if (statsTimer) clearInterval(statsTimer);
		unlisten_files_added();
		unlisten_sync_status();
		unlisten_file_sync_finished();
		unlisten_scan_progress();
	});
</script>

<div
	id="status-bar-footer"
	class={`mx-0 flex flex-row justify-between px-2 
      ${showingResults ? 'sticky-bottom' : 'fixed-bottom'}
			${$compactViewMode ? 'compact-view' : ''}
			${$windowBlurred ? 'grayscale' : ''}
  `}
>
	<!-- Left end -->
	<div class="relative flex-grow max-w-full flex-1 px-0 flex justify-start disable-select cursor-default" id="status-bar-left">
		{#if stats}
			{#if stats.status === 'scanning'}
				<span class="status-pill status-scanning" title="A background scan is running">
					<i class="bi bi-arrow-repeat spin-right" />
					{#if parseTotal > 0}
						Extracting text&hellip; {parseProgress}/{parseTotal}
					{:else}
						Scanning…
					{/if}
					{#if scanSpeed > 0}
						<span class="scan-speed">({scanSpeed.toFixed(1)} files/s)</span>
					{/if}
				</span>
				{#if parseTotal > 0}
					<span class="status-stat scan-progress-bar" title={`Extracting text from ${parseTotal} files`}>
						<div class="progress">
							<div
								class="progress-bar progress-bar-striped"
								id="scan-progress-bar"
								role="progressbar"
								style={`width: ${(parseProgress / parseTotal) * 100}%`}
								aria-valuenow={parseProgress}
								aria-valuemin={0}
								aria-valuemax={parseTotal}
							/>
						</div>
					</span>
				{/if}
			{:else if stats.status === 'ready'}
				<span class="status-pill status-ready" title="Automatic scan is enabled">
					<i class="bi bi-dot" />
					Ready
				</span>
			{:else}
				<span class="status-pill status-idle" title="Automatic scan is off">
					<i class="bi bi-dot" />
					Idle
				</span>
			{/if}
			<span class="status-stat" title="Files in the index">
				<i class="bi bi-files" />
				{stats.total_files}
			</span>
			<span class="status-stat" title="Database size">
				<i class="bi bi-database" />
				{formatSize(stats.database_size_bytes)}
			</span>
			{#if stats.auto_sync_enabled && stats.status !== 'scanning'}
				<span class="status-stat" title="Time until the next automatic scan">
					<i class="bi bi-clock" />
					{formatCountdown(countdownSeconds)} until next scan
				</span>
			{/if}
		{/if}
		{#if $userPreferences.onboarding_done}
			{#if $onSearchPage}
				<!-- <button
					type="button"
					class="px-1 mx-1 status-item"
					on:click={() => goToSearch()}
					title="View search results"
				> -->
				Showing {numFiles} {numFiles === 1 ? "result" : "results"}
				<!-- </button> -->
			{:else}
				<!-- This is used on the Settings page when adding docs manually -->
				{#if $dbCreationInProgress}
					Scanning... {filesAddedCount}	files added
				{/if}
			{/if}
		{:else if dbReady || $dbCreationInProgress}
			{#if $dbCreationInProgress}
				Scanning... {filesAddedCount}	files added
			{:else if dbReady}
				Scan complete!
			{/if}
		{:else}
			Hello!
		{/if}
	</div>

	<!-- Center -->
	<div class="relative flex-grow max-w-full flex-1 px-0 flex justify-center disable-select cursor-default" id="status-bar-center">
		{#if $userPreferences.onboarding_done}
			{$statusMessage}
		{:else if dbReady}
			<button
				type="button"
				class="px-1 mx-1 status-item"
				on:click={() => goToSearch(true)}
				title="Scan complete. Start searching!"
			>
				<i class="bi bi-check-circle" />
			</button>
		{:else if $searchInProgress || $dbCreationInProgress}
			<div class="flex justify-content-center items-center">
				<div class="spinner-border spinner-border-sm" role="status">
					<span class="visually-hidden">Loading...</span>
				</div>
			</div>
		{/if}
	</div>

	<!-- Right end -->
	<div class="relative flex-grow max-w-full flex-1 px-0 flex justify-end disable-select" id="status-bar-right">
		{#if $userPreferences.onboarding_done}
			<!-- Notifications -->
			<!-- <div class="dropup dropup-center px-0 mx-0 status-item">
				<button
					id="bg-sync-btn"
					type="button"
					class={`status-item px-1  ${$syncStatus ? (syncCoolingPeriod ? 'disabled-gray' : 'bg-code-pink') : ''}`}
					title={syncCoolingPeriod ? 'Please wait for a few seconds...' : `Background scan is ${$syncStatus ? 'running' : 'stopped'}. Click to ${$syncStatus ? 'stop' : 'start'}.`}
					disabled={syncCoolingPeriod}
					data-bs-toggle="dropdown"
					aria-expanded="false"
					style="all: unset;"
				>
					<i id="bg-sync-icon" class='bi bi-bell' />
				</button>
				<ul class="dropdown-menu py-0 px-2" style="cursor: default;">
					<div class="file-stats-header text-center">
						<strong><small>Notifications</small></strong>
					</div>
					<hr class="mb-1 mt-0" />
					<div><small>Files Indexed</small></div>
					<div><small>124</small></div>
				</ul>
			</div> -->
			<button
				id="bg-sync-btn"
				type="button"
				class={`px-2 status-item ${$syncStatus ? (syncCoolingPeriod ? 'disabled-gray' : 'bg-code-pink') : ''}`}
				title={syncCoolingPeriod ? 
				`${userAskedToDisable ? "Please wait... Shutting down background processes" : "Booting up... Please wait for a few seconds"}` 
				: 
				`Click to ${$syncStatus ? 'stop' : 'start'} background scan`}
				on:click={() => toggleBackgroundTextProcessing()}
				disabled={syncCoolingPeriod}
			>
				<i id="bg-sync-icon" class={`bi bi-arrow-repeat ${$syncStatus ? 'spin-right' : ''}`} />
			</button>
			<button
				type="button"
				id="status-bar-extras"
				class="px-2 status-item"
				title="View the fun stuff"
				on:click={() => showStatusBarMenu('extras')}
			>
				<i class="bi bi-stars" />
			</button>
			{#if $showIconGrid}
			<button
				type="button"
				id="status-bar-extras"
				class="px-2 status-item"
				title="Switch to List View"
				on:click={() => $showIconGrid = !$showIconGrid}
			>
				<i class="bi bi-list-ul" />
			</button>
			{:else}
				<button
					type="button"
					id="status-bar-extras"
					class="px-2 status-item"
					title="Switch to Icon Grid"
					on:click={() => $showIconGrid = !$showIconGrid}
				>
					<i class="bi bi-grid" />
				</button>
			{/if}
			{#if $compactViewMode}
				<button
					type="button"
					class="px-2 status-item"
					title="Show results in normal view"
					on:click={() => toggleCompactViewMode()}
				>
					<i class="bi bi-arrows-expand" />
				</button>
			{:else}
				<button
					type="button"
					class="px-2 status-item"
					title="Show results in compact view"
					on:click={() => toggleCompactViewMode()}
				>
					<i class="bi bi-arrows-collapse" />
				</button>
			{/if}
		{/if}
	</div>
</div>

<style lang="scss">
	.code-pink {
		color: var(--bs-code-color) !important;
	}
	.bg-code-pink {
		background: var(--bs-code-color) !important;
	}
	.disabled-gray {
		background: var(--bs-gray) !important;
		color: white;
		&:hover {
			cursor: not-allowed !important;
		}
	}
	#status-bar-footer.compact-view {
		height: 1.75em;
		line-height: 1.75em;
		font-size: 0.8em;
	}
	#status-bar-footer {
		height: 2em;
		line-height: 2em;
		font-size: 0.85em;
		color: white;
		text-align: center;
		background-color: var(--purple);
		/* background-color: rgb(0, 122, 204); */
		position: fixed;
		bottom: 0px;
		width: 100%;
	}
	.status-item:hover {
		background-color: rgba(255, 255, 255, 0.12);
		cursor: pointer;
	}
	.status-pill {
		display: inline-flex;
		align-items: center;
		gap: 0.25em;
		padding: 0 0.5em;
		margin-right: 0.75em;
		border-radius: 1em;
		font-weight: 600;
		line-height: 1.5em;
		background-color: rgba(0, 0, 0, 0.25);
	}
	.status-pill i {
		font-size: 1.1em;
	}
	.status-scanning {
		background-color: #e8556d;
	}
	.scan-speed {
		font-weight: 400;
		opacity: 0.85;
	}
	.status-ready {
		background-color: #21a366;
	}
	.status-idle {
		background-color: rgba(255, 255, 255, 0.2);
	}
	.status-stat {
		display: inline-flex;
		align-items: center;
		gap: 0.35em;
		margin-right: 1em;
		opacity: 0.95;
	}
	.status-stat i {
		font-size: 1.05em;
	}
	button {
		all: unset;
		cursor: pointer;
	}
</style>
