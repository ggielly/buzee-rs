<script lang="ts">
	import { fade } from 'svelte/transition';
	import PopoverIcon from '$lib/components/ui/popoverIcon.svelte';
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { trackEvent } from '@aptabase/web';
	import { invoke } from '@tauri-apps/api/core';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { isMac, statusMessage, userPreferences, dbCreationInProgress, syncStatus, darkMode } from '$lib/stores';
	import { ask, open, message } from '@tauri-apps/plugin-dialog';
	import * as Dialog from "$lib/components/ui/dialog";
  import Button from "$lib/components/ui/button/button.svelte";
	import * as Select from "$lib/components/ui/select";
	import { Switch } from "$lib/components/ui/switch";
	import {PencilLine, TriangleAlert, RefreshCw, ScanLine} from "lucide-svelte";
	import Separator from '$lib/components/ui/separator/separator.svelte';
	import Input from '$lib/components/ui/input/input.svelte';

	let showSearchSuggestions: boolean;
	let launchAtStartup: boolean;
	let globalShortcutDialogOpen = false;
	let globalShortcutEnabled: boolean;
	let globalShortcut: String;
	let globalShortcutModifiers: any[] = [{value: "Alt", label: "Option (⌥)"}, {value: "", label: " "}];
	if (!$isMac) { globalShortcutModifiers[0].label = "Alt"; }
	let globalShortcutCode: String = "Space";
	let automaticBackgroundSyncEnabled: boolean;
	let detailedScanEnabled: boolean;
	let parsePDF: boolean;
	let manualSetupMode: boolean;
	let clearIndexDialogOpen = false;
	let rescanDialogOpen = false;
	let rescanInProgress = false;
	let enableLogs: boolean;
	let pdfMaxOcrPages: number;
	let ocrThreads: number;
	let ocrSortOrder: string;
	let selectedOcrSortOrder = { value: "size_asc", label: "Smallest first" };
	let ocrSortOrderLabels: Record<string, string> = {
		"size_asc": "Smallest first",
		"size_desc": "Largest first",
		"name_asc": "Name (A-Z)",
		"name_desc": "Name (Z-A)",
		"modified_asc": "Oldest modified first",
		"modified_desc": "Newest modified first",
	};
	let ocrRescanInProgress = false;
	let ocrRescanDialogOpen = false;
	let ocrRescanProgress = 0;
	let ocrRescanTotal = 0;
	let ocrRescanSuccess = 0;
	let ocrRescanFailed = 0;
	let ocrRescanCurrentFile = "";
	let ocrRescanThreads = 1;
	let ocrRescanFailedFiles: { path: string; name: string; error: string }[] = [];
	let ocrRescanSuccessFiles: { path: string; name: string }[] = [];
	let ocrRescanFinished = false;
	let showFailedList = false;
	let showSuccessList = false;
	let unlistenOcrScanProgress: UnlistenFn;
	let unlistenOcrRescanProgress: UnlistenFn;

	function setKeydownHandlerOnGlobalShortuctInput(event: KeyboardEvent) {
		console.log("~>>! pressed:", event.key);
		event.preventDefault(); // Prevent the default action of the keypress
		if (event.key === 'Backspace' || event.key === 'Delete') {
			// if the pressed key is backspace or delete, clear the input field
			const shortcutInput = document.getElementById('shortcut-input');
			(shortcutInput as HTMLInputElement).value = '';
			globalShortcutCode = '';
			return;
		}
		// if event.key is alphanumeric, space or F1-F24, proceed
		console.log("pressed:", event.key);
		if (event.key.match(/^[a-zA-Z0-9]$/) || event.key.match(/^F[1-2]?[0-9]$/) || event.key === ' ') {
			let shortcut = '';
			if (event.key === ' ') shortcut = 'Space';
			else shortcut = event.key.toUpperCase();
			// Update the input field value with the captured shortcut
			const shortcutInput = document.getElementById('shortcut-input');
			(shortcutInput as HTMLInputElement).value = shortcut;
			globalShortcutCode = shortcut;
		}
	}

	function toggleShowSearchSuggestions() {
		showSearchSuggestions = !showSearchSuggestions;
		$userPreferences.show_search_suggestions = showSearchSuggestions;
		trackEvent('click:toggleShowSearchSuggestions', { showSearchSuggestions });
		$statusMessage = `Setting changed!`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("set_user_preference", {key: "show_search_suggestions", value: showSearchSuggestions}).then(() => {
			console.log("Set show search suggestions flag to: " + showSearchSuggestions);
		});
	}

	function toggleLaunchAtStartup() {
		launchAtStartup = !launchAtStartup;
		trackEvent('click:toggleLaunchAtStartup', { launchAtStartup });
		$statusMessage = `Setting changed!`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("set_user_preference", {key: "launch_at_startup", value: launchAtStartup}).then(() => {
			console.log("Set launch at startup flag to: " + showSearchSuggestions);
		});
	}

	function toggleGlobalShortcut() {
		globalShortcutEnabled = !globalShortcutEnabled;
		trackEvent('click:toggleGlobalShortcut', { globalShortcutEnabled });
		$statusMessage = `Setting changed. Restarting the app...`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("set_user_preference", {key: "global_shortcut_enabled", value: globalShortcutEnabled}).then(() => {
			console.log("Set global shortcut flag to: " + globalShortcutEnabled);
		});
	}

	function toggleDetailedScan() {
		detailedScanEnabled = !detailedScanEnabled;
		trackEvent('click:toggleDetailedScan', { detailedScanEnabled });
		$statusMessage = `Setting changed!`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("set_user_preference", {key: "detailed_scan", value: detailedScanEnabled}).then(() => {
			console.log("Set detailed scan flag to: " + detailedScanEnabled);
		});
	}

	function toggleManualSetupMode() {
		manualSetupMode = !manualSetupMode;
		trackEvent('click:toggleManualSetupMode', { manualSetupMode });
		$statusMessage = `Setting changed!`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("set_user_preference", {key: "manual_setup", value: manualSetupMode}).then(() => {
			console.log("Set manual setup flag to: " + manualSetupMode);
		});
	}

	function toggleParsePDF() {
		parsePDF = !parsePDF;
		trackEvent('click:toggleParsePDF', { parsePDF });
		$statusMessage = `Setting changed!`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("set_user_preference", {key: "parse_pdfs", value: parsePDF}).then(() => {
			console.log("Set parsePDF flag to: " + parsePDF);
		});
	}

	function toggleEnableLogs() {
		enableLogs = !enableLogs;
		trackEvent('click:toggleEnableLogs', { enableLogs });
		$statusMessage = enableLogs
			? `Logging enabled. Restart the app to start writing logs.`
			: `Logging disabled. Restart the app to stop writing logs.`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("set_user_preference", {key: "enable_logs", value: enableLogs}).then(() => {
			console.log("Set enable logs flag to: " + enableLogs);
		});
	}

	function updatePdfMaxOcrPages() {
		if (pdfMaxOcrPages < 1) pdfMaxOcrPages = 1;
		trackEvent('click:updatePdfMaxOcrPages', { pdfMaxOcrPages });
		$statusMessage = `Setting changed. Restart the app to take effect.`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("set_pdf_max_ocr_pages", { pages: pdfMaxOcrPages }).then(() => {
			console.log("Set PDF max OCR pages to: " + pdfMaxOcrPages);
		});
	}

	function updateOcrThreads() {
		ocrThreads = Math.max(1, Math.min(4, Math.round(ocrThreads)));
		trackEvent('click:updateOcrThreads', { ocrThreads });
		$statusMessage = `Setting changed.`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("set_ocr_threads", { threads: ocrThreads }).then(() => {
			console.log("Set OCR threads to: " + ocrThreads);
		});
	}

	function updateOcrSortOrder() {
		trackEvent('click:updateOcrSortOrder', { ocrSortOrder });
		$statusMessage = `Setting changed.`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("set_ocr_sort_order", { sortOrder: ocrSortOrder }).then(() => {
			console.log("Set OCR sort order to: " + ocrSortOrder);
		});
	}

	async function startOcrRescan() {
		trackEvent('click:startOcrRescan');
		ocrRescanInProgress = true;
		ocrRescanFinished = false;
		ocrRescanProgress = 0;
		ocrRescanSuccess = 0;
		ocrRescanFailed = 0;
		ocrRescanCurrentFile = "";
		ocrRescanFailedFiles = [];
		ocrRescanSuccessFiles = [];
		showFailedList = false;
		showSuccessList = false;
		$statusMessage = "Starting OCR rescan...";
		try {
			await invoke("start_ocr_rescan");
		} catch (error) {
			$statusMessage = "OCR rescan could not start.";
			console.error(error);
			ocrRescanInProgress = false;
		}
	}

	async function retryFailedOcrFiles() {
		trackEvent('click:retryFailedOcrFiles');
		if (ocrRescanFailedFiles.length === 0) return;
		const paths = ocrRescanFailedFiles.map((f) => f.path);
		ocrRescanInProgress = true;
		ocrRescanFinished = false;
		ocrRescanProgress = 0;
		ocrRescanSuccess = 0;
		ocrRescanFailed = 0;
		ocrRescanCurrentFile = "";
		ocrRescanFailedFiles = [];
		ocrRescanSuccessFiles = [];
		showFailedList = false;
		showSuccessList = false;
		$statusMessage = "Retrying failed OCR files...";
		try {
			await invoke("rescan_ocr_files", { paths });
		} catch (error) {
			$statusMessage = "OCR rescan could not start.";
			console.error(error);
			ocrRescanInProgress = false;
		}
	}

	async function retrySingleOcrFile(path: string) {
		trackEvent('click:retrySingleOcrFile');
		ocrRescanInProgress = true;
		ocrRescanFinished = false;
		ocrRescanProgress = 0;
		ocrRescanSuccess = 0;
		ocrRescanFailed = 0;
		ocrRescanCurrentFile = "";
		ocrRescanFailedFiles = [];
		ocrRescanSuccessFiles = [];
		showFailedList = false;
		showSuccessList = false;
		$statusMessage = "Retrying OCR on one file...";
		try {
			await invoke("rescan_ocr_files", { paths: [path] });
		} catch (error) {
			$statusMessage = "OCR rescan could not start.";
			console.error(error);
			ocrRescanInProgress = false;
		}
	}

	async function stopOcrRescan() {
		trackEvent('click:stopOcrRescan');
		$statusMessage = "Stopping OCR rescan...";
		await invoke("stop_ocr_rescan");
	}

	// The per-file progress events deliberately omit the file lists (they can be
	// huge). Fetch them from the backend on demand when the user expands a list.
	async function toggleSuccessList() {
		showSuccessList = !showSuccessList;
		if (showSuccessList && ocrRescanSuccessFiles.length === 0) {
			try {
				ocrRescanSuccessFiles = await invoke("get_ocr_rescan_success_files");
			} catch (error) {
				console.error("Failed to fetch succeeded files:", error);
			}
		}
	}

	async function toggleFailedList() {
		showFailedList = !showFailedList;
		if (showFailedList && ocrRescanFailedFiles.length === 0) {
			try {
				ocrRescanFailedFiles = await invoke("get_ocr_rescan_failed_files");
			} catch (error) {
				console.error("Failed to fetch failed files:", error);
			}
		}
	}

	function toggleAutomaticBackgroundSync() {
		automaticBackgroundSyncEnabled = !automaticBackgroundSyncEnabled;
		trackEvent('click:toggleAutomaticBackgroundSync', { automaticBackgroundSyncEnabled });
		$statusMessage = `Setting changed!`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("set_user_preference", {key: "automatic_background_sync", value: automaticBackgroundSyncEnabled}).then(() => {
			console.log("Set automatic background sync flag to: " + automaticBackgroundSyncEnabled);
		});
	}

	function resetDefault() {
    trackEvent('click:resetDefault');
		$statusMessage = `Settings reset. Restarting the app...`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("reset_user_preferences").then(() => {
			console.log("User preferences reset to default");
		});
	}

	async function clearIndex() {
    trackEvent('click:clearIndex');
		await invoke("clear_index");
		$statusMessage = `Cleared!`;
	}

	async function rescanDocuments(rescanAll: boolean) {
		trackEvent('click:rescanDocuments', { rescanAll });
		rescanInProgress = true;
		$statusMessage = rescanAll
			? "Rescanning all documents..."
			: "Rescanning for missing documents...";
		$syncStatus = true;
		try {
			await invoke("rescan_documents", { rescanAll });
			$statusMessage = rescanAll
				? "Rescan complete! All documents re-indexed."
				: "Rescan complete! Missing documents indexed.";
		} catch (error) {
			$statusMessage = "Rescan failed. Please try again.";
			console.error(error);
		} finally {
			setTimeout(() => {
				$statusMessage = "";
				$syncStatus = false;
				rescanInProgress = false;
				rescanDialogOpen = false;
			}, 3000);
		}
	}

	function uninstallApp() {
    trackEvent("click:uninstallApp");
		goto('/uninstall');
	}

	async function addDocsToDB() {
		trackEvent('click:addDocsToDB');
		let isFolder = true;
		const yesFolders = await ask("Would you like to add individual files or complete folders?", { 
			title: 'Files or Folders',
			kind: 'info',
			okLabel: 'Folders',
			cancelLabel: 'Files'
		});
		let filePaths: String[] = [];
		if (yesFolders) {
			let folderPaths = await open({ 
				title: 'Add Folders',
				directory: true,
				recursive: true,
				multiple: true,
				canCreateDirectories: false
			});
			if (folderPaths === null) {
				return;
			}
			filePaths = folderPaths;
		} else {
			let filePathObjects = await open({ 
				title: 'Add Files',
				directory: false,
				multiple: true,
				canCreateDirectories: false
			});
			if (filePathObjects === null) {
				return;
			}
			filePaths = filePathObjects.map((file) => file);
			isFolder = false;
		}
		$statusMessage = "Adding documents to the database...";
		$dbCreationInProgress = true;
		$syncStatus = true;
		invoke("run_file_indexing", {filePaths: filePaths, isFolder: isFolder }).then((res) => {
			console.log(res);
			$statusMessage = "Documents added successfully!";
			setTimeout(() => {
				$statusMessage = "";
				$dbCreationInProgress = false;
				$syncStatus = false;
			}, 3000);
		});
	}

	function setNewGlobalShortcut() {
		// ensure that globalShortcutModifers[1] is not empty and different from globalShortcutModifiers[0]
		if (globalShortcutModifiers[1].value === globalShortcutModifiers[0].value) {
			globalShortcutModifiers[1] = {value: "", label: ""};
		}
		if (globalShortcutModifiers[1].value === "") {
			globalShortcut = globalShortcutModifiers[0].value + "+" + globalShortcutCode;
		} else {
			globalShortcut = globalShortcutModifiers[0].value + "+" + globalShortcutModifiers[1].value + "+" + globalShortcutCode;
		}
		console.log(globalShortcut);
		$statusMessage = `Setting changed. Restarting the app...`;
		setTimeout(() => {$statusMessage = "";}, 3000);
		invoke("set_new_global_shortcut", { newShortcutString: globalShortcut }).then((res) => {
			console.log(res);
		});
		if ($isMac) {
			globalShortcut = globalShortcut.replace("Alt", "Option");
			globalShortcut = globalShortcut.replace("Super", "Command");
		}
	}

	async function checkForAppUpdates() {
		await message('Automatic updates are disabled in this build.', {
			title: 'Updates Disabled',
			kind: 'info',
			okLabel: 'OK'
		});
	}

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
			console.log(res);
			// @ts-ignore
			$userPreferences = res;
			showSearchSuggestions = $userPreferences.show_search_suggestions;
			launchAtStartup = $userPreferences.launch_at_startup;
			globalShortcutEnabled = $userPreferences.global_shortcut_enabled;
			globalShortcut = $userPreferences.global_shortcut;
			console.log(globalShortcut);
			globalShortcut = globalShortcut.replace("Key", "");
			globalShortcut = globalShortcut.replace("Digit", "");
			if (globalShortcut.split("+").length === 2) {
				globalShortcutModifiers[0].value = globalShortcut.split("+")[0];
				globalShortcutModifiers[1].value = "";
				globalShortcutModifiers[0].label = globalShortcut.split("+")[0];
				globalShortcutModifiers[1].label = " ";
				globalShortcutCode = globalShortcut.split("+")[1];
			} else if (globalShortcut.split("+").length === 3) {
				globalShortcutModifiers[0].value = globalShortcut.split("+")[0];
				globalShortcutModifiers[1].value = globalShortcut.split("+")[1];
				globalShortcutModifiers[0].label = globalShortcut.split("+")[0];
				globalShortcutModifiers[1].label = globalShortcut.split("+")[1];
				globalShortcutCode = globalShortcut.split("+")[2];
			}
			console.log(globalShortcutModifiers);
			console.log(globalShortcutCode);
			if ($isMac) {
				globalShortcut = globalShortcut.replace("Alt", "Option");
				globalShortcut = globalShortcut.replace("Super", "Command");
			}
			console.log(globalShortcut);
			automaticBackgroundSyncEnabled = $userPreferences.automatic_background_sync;
			detailedScanEnabled = $userPreferences.detailed_scan;
			parsePDF = $userPreferences.parse_pdfs;
			manualSetupMode = $userPreferences.manual_setup;
			enableLogs = $userPreferences.enable_logs;
			pdfMaxOcrPages = $userPreferences.pdf_max_ocr_pages;
			ocrThreads = $userPreferences.ocr_threads;
			ocrSortOrder = $userPreferences.ocr_sort_order;
			selectedOcrSortOrder = { value: ocrSortOrder, label: ocrSortOrderLabels[ocrSortOrder] ?? "Smallest first" };
		});

		// Listen to OCR rescan progress/finish events. The rich "ocr-rescan-progress"
		// event carries a JSON payload in its data field with all the stats the
		// dialog needs (total, processed, success, failed, current file, threads,
		// and the running list of failed files).
		unlistenOcrRescanProgress = await listen<{message: string, data: string}>('ocr-rescan-progress', (event) => {
			if (!event.payload) return;
			try {
				const progress = JSON.parse(event.payload.data);
				if (progress.message === "started") {
					ocrRescanInProgress = true;
					ocrRescanFinished = false;
					ocrRescanProgress = 0;
					ocrRescanTotal = progress.total || 0;
					ocrRescanSuccess = 0;
					ocrRescanFailed = 0;
					ocrRescanCurrentFile = "";
					ocrRescanThreads = progress.threads || 1;
					ocrRescanFailedFiles = progress.failed_files || [];
					ocrRescanSuccessFiles = progress.success_files || [];
					showFailedList = false;
					showSuccessList = false;
				} else if (progress.message === "progress") {
					ocrRescanProgress = progress.processed || 0;
					ocrRescanTotal = progress.total || 0;
					ocrRescanSuccess = progress.success || 0;
					ocrRescanFailed = progress.failed || 0;
					ocrRescanCurrentFile = progress.current_file || "";
					ocrRescanThreads = progress.threads || 1;
					ocrRescanFailedFiles = progress.failed_files || [];
					ocrRescanSuccessFiles = progress.success_files || [];
				} else if (progress.message === "finished" || progress.message === "cancelled") {
					ocrRescanProgress = progress.processed || 0;
					ocrRescanTotal = progress.total || 0;
					ocrRescanSuccess = progress.success || 0;
					ocrRescanFailed = progress.failed || 0;
					ocrRescanCurrentFile = "";
					ocrRescanThreads = progress.threads || 1;
					ocrRescanFailedFiles = progress.failed_files || [];
					ocrRescanSuccessFiles = progress.success_files || [];
					ocrRescanInProgress = false;
					ocrRescanFinished = true;
					showFailedList = false;
					showSuccessList = false;
					$statusMessage = progress.message === "finished"
						? "OCR rescan complete!"
						: "OCR rescan cancelled.";
					setTimeout(() => {$statusMessage = "";}, 4000);
				}
			} catch (error) {
				console.error("Failed to parse OCR rescan progress:", error);
			}
		});
		unlistenOcrScanProgress = await listen<{message: string, data: string}>('scan-progress', (event) => {
			if (event.payload?.message === "scan_started") {
				ocrRescanProgress = 0;
				ocrRescanTotal = Number(event.payload.data) || 0;
			} else if (event.payload?.message === "scan_progress") {
				const [processed, total] = event.payload.data.split("/");
				ocrRescanProgress = Number(processed) || 0;
				ocrRescanTotal = Number(total) || 0;
			}
		});
	});

	onDestroy(() => {
		if (unlistenOcrRescanProgress) unlistenOcrRescanProgress();
		if (unlistenOcrScanProgress) unlistenOcrScanProgress();
	});
</script>

<div class="flex flex-col" in:fade={{ delay: 0, duration: 500 }}>
  <h3 class="text-lg font-semibold leading-none tracking-tight">Settings</h3>
  <p class="text-sm text-muted-foreground">Tune the knobs to make Buzee yours</p>
</div>
<div class="flex flex-1 flex-col items-center justify-center rounded-lg border border-dashed shadow-sm p-4">
	<table class="w-4/5 md:w-3/5 table table-bordered my-2">
		<!-- Buttons / Links -->
		<tr class="hover:text-violet-500">
			<td class="text-center px-2">
				<button on:click={() => addDocsToDB()}>
					<i class="bi bi-plus-circle" />
				</button>
			</td>
			<td class="py-2" role="button" on:click={() => addDocsToDB()}>
				Add Documents
				<div class="flex items-center small-explanation gap-1">
					Add more documents to search in Buzee
				</div>
			</td>
			<td class="">
				<PopoverIcon
					title="By default, Buzee scans your entire system. You can add files from external drives or network drives here."
				/>
			</td>
		</tr>
		<tr class="hover:text-violet-500">
			<td class="text-center px-2">
				<button on:click={() => goto('/settings/ignore')}>
					<div class="flex">
						<i class="bi bi-file-earmark-x" />
						<i class="bi bi-folder-x" />
					</div>
				</button>
			</td>
			<td class="py-2" role="button" on:click={() => goto('/settings/ignore')}>
				Ignore List
				<div class="flex items-center small-explanation gap-1">
					<div>List of files and folders that you want Buzee to ignore</div>
				</div>
			</td>
		</tr>
		<tr class="hover:text-violet-500">
			<td class="text-center px-2">
				<button on:click={() => goto('/settings/filetype-list')}>
					<div class="flex">
						<i class="bi bi-file-earmark" />
					</div>
				</button>
			</td>
			<td class="py-2" role="button" on:click={() => goto('/settings/filetype-list')}>
				File Type List
				<div class="flex items-center small-explanation gap-1">
					<div>List of file types that Buzee can scan</div>
				</div>
			</td>
		</tr>

		<tr class="h-10">
			<td colspan="3"><Separator /></td>
		</tr>

		<!-- Dark Mode -->
		<tr>
			<td class="text-center px-2">
				<Switch class="hover:data-[state=checked]:bg-violet-500" bind:checked={$darkMode} />
			</td>
			<td class="py-2 skip-hover">
				Dark Mode
				<div class="flex items-center small-explanation gap-1">
					<div>Use a dark theme for the app</div>
				</div>
			</td>
		</tr>

		<!-- On/Off Toggles -->
		<tr>
			<td class="text-center px-2">
				<Switch class="hover:data-[state=checked]:bg-violet-500" bind:checked={showSearchSuggestions} on:click={() => toggleShowSearchSuggestions()} />
			</td>
			<td class="py-2 skip-hover">
				Show Search Suggestions
				<div class="flex items-center small-explanation gap-1">
					Buzee will suggest search terms from your documents
				</div>
			</td>
		</tr>
		<tr>
			<td class="text-center px-2">
				<Switch class="hover:data-[state=checked]:bg-violet-500" bind:checked={globalShortcutEnabled} on:click={() => toggleGlobalShortcut()} />
			</td>
			<td class="py-2 skip-hover">
				Allow Global Shortcut
				<div class="flex items-center small-explanation gap-1">
					Pressing <code class="small-explanation">{globalShortcut}</code>
						<Dialog.Root bind:open={globalShortcutDialogOpen}>
							<Dialog.Trigger class="skip-hover border p-1 rounded-full flex justify-center items-center gap-1 hover:border-violet-500 hover:text-violet-500"><PencilLine class="h-3 w-3"/></Dialog.Trigger>
							<Dialog.Content>
								<Dialog.Header>
									<Dialog.Title>Change Global Shortcut</Dialog.Title>
								</Dialog.Header>
								<div>
									<p>Pressing the global shortcut shows the app from anywhere.</p>
									<p>Current shortcut: <code>{globalShortcut}</code></p>
									<p>Set new shortcut below:</p>
									<div class="flex gap-1 justify-center items-center">
										<div class="col-4 flex items-center">
											<Select.Root bind:selected={globalShortcutModifiers[0]}>
												<Select.Trigger class="md:w-[150px]">
													<Select.Value placeholder={globalShortcutModifiers[0]} />
												</Select.Trigger>
												<Select.Content>
													{#if $isMac}
														<Select.Item value="Super">Command (⌘)</Select.Item>
														<Select.Item value="Alt">Option (⌥)</Select.Item>
														<Select.Item value="Control">Control (^)</Select.Item>
													{:else}
														<Select.Item value="Control">Control</Select.Item>
														<Select.Item value="Alt">Alt</Select.Item>
													{/if}
													<Select.Item value="Shift">Shift</Select.Item>
												</Select.Content>
											</Select.Root>
										</div>
										<div class="col-4 flex items-center">
											<Select.Root bind:selected={globalShortcutModifiers[1]}>
												<Select.Trigger class="md:w-[150px]">
													<Select.Value placeholder={globalShortcutModifiers[1]} />
												</Select.Trigger>
												<Select.Content>
													{#if $isMac}
														<Select.Item value="Super">Command (⌘)</Select.Item>
														<Select.Item value="Alt">Option (⌥)</Select.Item>
														<Select.Item value="Control">Control (^)</Select.Item>
													{:else}
														<Select.Item value="Control">Control</Select.Item>
														<Select.Item value="Alt">Alt</Select.Item>
													{/if}
													<Select.Item value="Shift">Shift</Select.Item>
													<Select.Item value="">&nbsp;</Select.Item>
												</Select.Content>
											</Select.Root>
										</div>
										<div class="col-4">
											<Input
												type="text"
												id="shortcut-input"
												class={`flex h-10 w-full items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-offset-0 focus-visible:ring-0 md:w-[150px] ${globalShortcutCode === '' ? 'border-red-500' : ''}`}
												placeholder="Key"
												on:keydown={(e) => setKeydownHandlerOnGlobalShortuctInput(e)}
												bind:value={globalShortcutCode}
											/>
										</div>
									</div>
									<div class="my-2">
										{#if globalShortcutCode === ""}
											<small class="text-danger small-explanation">Shortcut value cannot be empty</small>
											{#if globalShortcutModifiers[1].value === globalShortcutModifiers[0].value}<br/>{/if}
										{/if}
										{#if globalShortcutModifiers[1].value === globalShortcutModifiers[0].value}
											<small class="text-danger small-explanation">Both modifier keys cannot be the same</small>
										{/if}
									</div>
								</div>
								<Dialog.Footer class="flex sm:justify-between items-center gap-2">
									<Dialog.Description>Setting a new shortcut will automatically restart the app</Dialog.Description>
									<Button
												type="button"
												class="btn btn-success"
												disabled={globalShortcutCode === "" || globalShortcutModifiers[1].value === globalShortcutModifiers[0].value}
												on:click={() => setNewGlobalShortcut()}>Save</Button>
								</Dialog.Footer>
							</Dialog.Content>
						</Dialog.Root>
					will show the app from anywhere
				</div>
			</td>
			<td>
				<PopoverIcon title="Changes will take effect after the app restarts" />
			</td>
		</tr>

		<tr class="h-10">
			<td colspan="3"><Separator /></td>
		</tr>

		<tr class="h-10">
			<td colspan="3" class="text-center text-muted-foreground font-mono"><small>ADVANCED</small></td>
		</tr>

		<tr>
			<td class="text-center px-2">
				<Switch class="hover:data-[state=checked]:bg-violet-500" bind:checked={automaticBackgroundSyncEnabled} on:click={() => toggleAutomaticBackgroundSync()} />
			</td>
			<td class="py-2 skip-hover">
				Allow Automatic Background Scan
				<div class="flex items-center small-explanation gap-1">
					<div>This allows Buzee to scan your files automatically (twice an hour)</div>
				</div>
			</td>
			<td>
				<PopoverIcon title="We recommend keeping this setting enabled" />
			</td>
		</tr>
		<tr>
			<td class="text-center px-2">
				<Switch class="hover:data-[state=checked]:bg-violet-500" bind:checked={detailedScanEnabled} on:click={() => toggleDetailedScan()} />
			</td>
			<td class="py-2 skip-hover">
				Scan File Text
				<div class="flex items-center small-explanation gap-1">
					<div>Keep this on so you can search inside files (PDFs are scanned last)</div>
				</div>
			</td>
			<td>
				<PopoverIcon title="Disabling this setting may improve speed but reduce quality of search results"/>
			</td>
		</tr>
		<tr>
			<td class="text-center px-2">
				<Switch class="hover:data-[state=checked]:bg-violet-500" bind:checked={parsePDF} on:click={() => toggleParsePDF()} />
			</td>
			<td class="py-2 skip-hover">
				Scan Text from PDFs and Images
				<div class="flex items-center small-explanation gap-1">
					<div>This feature uses OCR and may take a lot of time.</div>
				</div>
			</td>
			<td>
				<PopoverIcon title="Disabling this setting may improve the quality of search results but make the app buggy"/>
			</td>
		</tr>
		<tr>
			<td class="text-center px-2">
				<Switch class="hover:data-[state=checked]:bg-violet-500" bind:checked={enableLogs} on:click={() => toggleEnableLogs()} />
			</td>
			<td class="py-2 skip-hover">
				Enable Logging
				<div class="flex items-center small-explanation gap-1">
					<div>Writes diagnostic logs to a file in the app data directory. Requires an app restart.</div>
				</div>
			</td>
			<td>
				<PopoverIcon title="Logs are stored as buzee.log in the app data folder and are useful for troubleshooting."/>
			</td>
		</tr>
		<tr>
			<td class="text-center px-2">
				<Button variant="outline" size="sm" type="button" on:click={() => updatePdfMaxOcrPages()}>Save</Button>
			</td>
			<td class="py-2 skip-hover">
				Max OCR Pages per PDF
				<div class="flex items-center small-explanation gap-1">
					<div>Maximum number of pages of a scanned PDF that are OCR-ed. Larger values give better results but take longer.</div>
				</div>
			</td>
			<td class="w-32">
				<Input
					type="number"
					min="1"
					bind:value={pdfMaxOcrPages}
					class="h-9 w-full"
				/>
			</td>
		</tr>
		<tr>
			<td class="text-center px-2">
				<Button variant="outline" size="sm" type="button" on:click={() => updateOcrThreads()}>Save</Button>
			</td>
			<td class="py-2 skip-hover">
				OCR Threads
				<div class="flex items-center small-explanation gap-1">
					<div>Number of files OCR-ed in parallel during a rescan. Higher values are faster but use more CPU.</div>
				</div>
			</td>
			<td class="w-32">
				<Input
					type="number"
					min="1"
					max="4"
					bind:value={ocrThreads}
					class="h-9 w-full"
				/>
			</td>
		</tr>
		<tr>
			<td class="text-center px-2">
				<Button variant="outline" size="sm" type="button" on:click={() => updateOcrSortOrder()}>Save</Button>
			</td>
			<td class="py-2 skip-hover">
				OCR Rescan Order
				<div class="flex items-center small-explanation gap-1">
					<div>Order in which files are OCR-ed during a rescan.</div>
				</div>
			</td>
			<td class="w-40">
				<Select.Root bind:selected={selectedOcrSortOrder} onSelectedChange={(v) => { if (v?.value) { ocrSortOrder = v.value; updateOcrSortOrder(); } }}>
					<Select.Trigger class="w-full h-9 justify-between">
						<Select.Value placeholder="Smallest first" />
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="size_asc">Smallest first</Select.Item>
						<Select.Item value="size_desc">Largest first</Select.Item>
						<Select.Item value="name_asc">Name (A-Z)</Select.Item>
						<Select.Item value="name_desc">Name (Z-A)</Select.Item>
						<Select.Item value="modified_asc">Oldest modified first</Select.Item>
						<Select.Item value="modified_desc">Newest modified first</Select.Item>
					</Select.Content>
				</Select.Root>
			</td>
		</tr>
		<tr class="hover:text-violet-500">
			<td class="text-center px-2">
				<Dialog.Root bind:open={ocrRescanDialogOpen}>
					<Dialog.Trigger class="flex justify-center items-center w-full">
						<ScanLine class="h-6 w-6" />
					</Dialog.Trigger>
					<Dialog.Content>
						<Dialog.Header>
							<Dialog.Title>Rescan OCR Documents</Dialog.Title>
							<Dialog.Description>Re-run OCR on all PDFs and images</Dialog.Description>
						</Dialog.Header>
						{#if ocrRescanInProgress}
							<!-- Live progress view -->
							<div class="flex flex-col gap-3">
								{#if ocrRescanTotal > 0}
									<div class="w-full">
										<div class="flex justify-between text-sm mb-1">
											<span>{ocrRescanProgress} / {ocrRescanTotal} files</span>
											<span>{Math.round((ocrRescanProgress / ocrRescanTotal) * 100)}%</span>
										</div>
										<div class="w-full h-2 bg-muted rounded-full overflow-hidden">
											<div
												class="h-full bg-violet-500 transition-all"
												style="width: {(ocrRescanTotal > 0 ? (ocrRescanProgress / ocrRescanTotal) * 100 : 0)}%"
											></div>
										</div>
									</div>
								{/if}
								<div class="grid grid-cols-2 gap-2 text-sm">
									<div class="flex items-center gap-2">
										<span class="text-muted-foreground">Threads:</span>
										<span>{ocrRescanThreads}</span>
									</div>
									<div class="flex items-center gap-2">
										<span class="text-muted-foreground">Remaining:</span>
										<span>{Math.max(0, ocrRescanTotal - ocrRescanProgress)}</span>
									</div>
									<div class="flex items-center gap-2">
										<span class="text-muted-foreground">Succeeded:</span>
										<span class="text-green-600">{ocrRescanSuccess}</span>
									</div>
									<div class="flex items-center gap-2">
										<span class="text-muted-foreground">Errors:</span>
										<span class="text-red-600">{ocrRescanFailed}</span>
									</div>
								</div>
								{#if ocrRescanCurrentFile}
									<div class="text-sm">
										<span class="text-muted-foreground">Currently:</span>
										<span class="ml-1 break-all font-mono">{ocrRescanCurrentFile}</span>
									</div>
								{/if}

								<!-- Expandable success list (live) -->
								{#if ocrRescanSuccessFiles.length > 0}
									<div class="border rounded-md overflow-hidden">
										<button
											type="button"
											class="w-full flex items-center justify-between px-3 py-2 text-sm font-medium hover:bg-muted"
											on:click={() => toggleSuccessList()}
										>
											<span>Succeeded files ({ocrRescanSuccessFiles.length})</span>
											<span>{showSuccessList ? '▲' : '▼'}</span>
										</button>
										{#if showSuccessList}
											<ul class="max-h-40 overflow-y-auto p-2 text-xs flex flex-col divide-y divide-border">
												{#each ocrRescanSuccessFiles as ok}
													<li class="py-1 break-all font-mono" title={ok.path}>
														<span class="text-green-600 mr-1">✓</span>{ok.path}
													</li>
												{/each}
											</ul>
										{/if}
									</div>
								{/if}
								<!-- Expandable failed list (live) -->
								{#if ocrRescanFailedFiles.length > 0}
									<div class="border rounded-md overflow-hidden">
										<button
											type="button"
											class="w-full flex items-center justify-between px-3 py-2 text-sm font-medium hover:bg-muted"
											on:click={() => toggleFailedList()}
										>
											<span class="text-red-600">Failed files ({ocrRescanFailedFiles.length})</span>
											<span>{showFailedList ? '▲' : '▼'}</span>
										</button>
										{#if showFailedList}
											<ul class="max-h-40 overflow-y-auto p-2 text-xs flex flex-col divide-y divide-border">
												{#each ocrRescanFailedFiles as failed}
													<li class="py-1 flex items-center justify-between gap-2">
														<span class="break-all font-mono flex-1" title={`${failed.path}\n${failed.error}`}>
															<span class="text-red-600 mr-1">✕</span>{failed.path}
															{#if failed.error}
																<div class="text-muted-foreground mt-0.5">{failed.error}</div>
															{/if}
														</span>
														<Button variant="outline" size="sm" type="button" on:click={() => retrySingleOcrFile(failed.path)} class="shrink-0">
															Retry
														</Button>
													</li>
												{/each}
											</ul>
										{/if}
									</div>
								{/if}
							</div>
						{:else if ocrRescanFinished}
							<!-- Result view -->
							<div class="flex flex-col gap-3">
								<div class="grid grid-cols-2 gap-2 text-sm">
									<div class="flex items-center gap-2">
										<span class="text-muted-foreground">Processed:</span>
										<span>{ocrRescanProgress}</span>
									</div>
									<div class="flex items-center gap-2">
										<span class="text-muted-foreground">Threads:</span>
										<span>{ocrRescanThreads}</span>
									</div>
									<div class="flex items-center gap-2">
										<span class="text-muted-foreground">Succeeded:</span>
										<span class="text-green-600">{ocrRescanSuccess}</span>
									</div>
									<div class="flex items-center gap-2">
										<span class="text-muted-foreground">Errors:</span>
										<span class="text-red-600">{ocrRescanFailed}</span>
									</div>
								</div>

								{#if ocrRescanSuccessFiles.length > 0}
									<div class="border rounded-md overflow-hidden">
										<button
											type="button"
											class="w-full flex items-center justify-between px-3 py-2 text-sm font-medium hover:bg-muted"
											on:click={() => toggleSuccessList()}
										>
											<span>Succeeded files ({ocrRescanSuccessFiles.length})</span>
											<span>{showSuccessList ? '▲' : '▼'}</span>
										</button>
										{#if showSuccessList}
											<ul class="max-h-40 overflow-y-auto p-2 text-xs flex flex-col divide-y divide-border">
												{#each ocrRescanSuccessFiles as ok}
													<li class="py-1 break-all font-mono" title={ok.path}>
														<span class="text-green-600 mr-1">✓</span>{ok.path}
													</li>
												{/each}
											</ul>
										{/if}
									</div>
								{/if}

								{#if ocrRescanFailedFiles.length > 0}
									<div class="border rounded-md overflow-hidden">
										<button
											type="button"
											class="w-full flex items-center justify-between px-3 py-2 text-sm font-medium hover:bg-muted"
											on:click={() => toggleFailedList()}
										>
											<span class="text-red-600">Failed files ({ocrRescanFailedFiles.length})</span>
											<span>{showFailedList ? '▲' : '▼'}</span>
										</button>
										{#if showFailedList}
											<ul class="max-h-40 overflow-y-auto p-2 text-xs flex flex-col divide-y divide-border">
												{#each ocrRescanFailedFiles as failed}
													<li class="py-1 flex items-center justify-between gap-2">
														<span class="break-all font-mono flex-1" title={`${failed.path}\n${failed.error}`}>
															<span class="text-red-600 mr-1">✕</span>{failed.path}
															{#if failed.error}
																<div class="text-muted-foreground mt-0.5">{failed.error}</div>
															{/if}
														</span>
														<Button variant="outline" size="sm" type="button" on:click={() => retrySingleOcrFile(failed.path)} class="shrink-0">
															Retry
														</Button>
													</li>
												{/each}
											</ul>
										{/if}
									</div>
								{:else}
									<p class="text-sm text-green-600 mb-0">All files were processed successfully!</p>
								{/if}
							</div>
						{:else}
							<p class="mb-0">
								This will re-run OCR on every PDF and image in the database, including files
								that were already processed. This can take a long time depending on the
								number of files.<br/><br/>
								Processing order and parallelism can be tuned in the OCR settings above.
							</p>
						{/if}
						<Dialog.Footer>
							{#if ocrRescanInProgress}
								<Button variant="secondary" on:click={() => stopOcrRescan()}>
									Stop
								</Button>
							{:else}
								<Dialog.Close asChild let:builder>
									<Button variant="secondary" aria-label="Close" builders={[builder]}>Close</Button>
								</Dialog.Close>
								{#if ocrRescanFinished && ocrRescanFailedFiles.length > 0}
									<Button variant="destructive" on:click={() => retryFailedOcrFiles()}>
										Retry failed ({ocrRescanFailedFiles.length})
									</Button>
								{/if}
								<Button on:click={() => startOcrRescan()}>
									Start OCR rescan
								</Button>
							{/if}
						</Dialog.Footer>
					</Dialog.Content>
				</Dialog.Root>
			</td>
			<td class="py-2 skip-hover" role="button" on:click={() => {ocrRescanDialogOpen = true;}}>
				Rescan OCR Documents
				<div class="flex items-center small-explanation gap-1">
					<div>Re-run OCR on all PDFs and images already in the database.</div>
				</div>
			</td>
			<td>
			</td>
		</tr>
		<tr class="hover:text-red-500">
			<td class="text-center px-2">
				<Dialog.Root bind:open={rescanDialogOpen}>
					<Dialog.Trigger class="flex justify-center items-center w-full">
						<RefreshCw class="h-6 w-6" />
					</Dialog.Trigger>
					<Dialog.Content>
						<Dialog.Header>
							<Dialog.Title>Rescan Documents</Dialog.Title>
							<Dialog.Description>Choose what to rescan</Dialog.Description>
						</Dialog.Header>
						{#if rescanInProgress}
							<p class="mb-0">Rescan in progress...</p>
						{:else}
							<p class="mb-0">
								Buzee can rescan your files now to pick up documents that are missing from
								the database, or force a full re-index of every document (including OCR of
								PDFs and images).<br/><br/>
								<strong>Rescan new documents</strong> only indexes files that are missing or
								changed.<br/>
								<strong>Rescan all documents</strong> re-extracts text and OCR from every file,
								which can take a long time.
							</p>
						{/if}
						<Dialog.Footer>
							<Dialog.Close asChild let:builder>
								<Button variant="secondary" aria-label="Close" builders={[builder]}>Close</Button>
							</Dialog.Close>
							<Button variant="secondary" on:click={() => rescanDocuments(false)} disabled={rescanInProgress}>
								Rescan new documents
							</Button>
							<Button on:click={() => rescanDocuments(true)} disabled={rescanInProgress}>
								Rescan all documents
							</Button>
						</Dialog.Footer>
					</Dialog.Content>
				</Dialog.Root>
			</td>
			<td class="py-2 skip-hover" role="button" on:click={() => {rescanDialogOpen = true;}}>
				Rescan Documents
				<div class="flex items-center small-explanation gap-1">
					<div>Find missing documents or force a full re-index.</div>
				</div>
			</td>
			<td>

			</td>
		</tr>
		<tr>
			<td class="text-center px-2">
				<Switch class="hover:data-[state=checked]:bg-violet-500" bind:checked={manualSetupMode} on:click={() => toggleManualSetupMode()} />
			</td>
			<td class="py-2 skip-hover">
				Run in Manual Setup Mode
				<div class="flex items-center small-explanation gap-1">
					<div>In manual mode, Buzee will only sync the files and folders that you add yourself.</div>
				</div>
			</td>
			<td>
				<PopoverIcon title="Disabling this setting will make Buzee scan your entire system automatically"/>
			</td>
		</tr>
		<tr class="hover:text-red-500">
			<td class="text-center px-2">
				<!-- <Switch class="hover:data-[state=checked]:bg-violet-500" bind:checked={manualSetupMode} on:click={() => toggleManualSetupMode()} /> -->
				<Dialog.Root bind:open={clearIndexDialogOpen}>
					<Dialog.Trigger class="flex justify-center items-center w-full">
						<TriangleAlert class="h-6 w-6" />
					</Dialog.Trigger>
					<Dialog.Content>
						<Dialog.Header>
							<Dialog.Title>Clear Index</Dialog.Title>
							<Dialog.Description>Remember to run the background sync after clearing the index</Dialog.Description>
						</Dialog.Header>
						{#if $statusMessage === "Cleared!"}
							<p>Done!</p>
							<div class="flex justify-center items-center">
								<lottie-player src="/checkmark-done.json" background="transparent"  speed="1"  style="width: 200px; height: 200px;" autoplay></lottie-player>
							</div>
						{:else}
							<p class="mb-0">
								If the search results are of poor quality, clearing the index and then rebuilding it can help.<br/><br/>
								Alternatively, you can clear the index, turn off the Scan File Text setting and use Buzee only for searching file metadata.
							</p>
						{/if}
						<Dialog.Footer>
							<Dialog.Close asChild let:builder>
								<Button variant="secondary" aria-label="Close" builders={[builder]}>Close</Button>
							</Dialog.Close>
							<Button on:click={() => clearIndex()}>Yes, clear the index</Button>
						</Dialog.Footer>
					</Dialog.Content>
				</Dialog.Root>
			</td>
			<td class="py-2 skip-hover" role="button" on:click={() => {clearIndexDialogOpen = true;}}>
				Clear the Index
				<div class="flex items-center small-explanation gap-1">
					<div>If the search results are of poor quality, clearing the index can help.</div>
				</div>
			</td>
			<td>
				
			</td>
		</tr>

		<tr class="h-10">
			<td colspan="3"><Separator /></td>
		</tr>
	</table>
	<div class="w-4/5 md:w-3/5 flex justify-between settings-links">
		<div class="relative flex-grow max-w-full flex-1 px-4 text-start mobile-text-center my-1">
			<Button variant="link" class="gap-2 text-xs !px-2" on:click={() => resetDefault()}>
				<span class="font-normal text-xs">Reset Default</span>
				<PopoverIcon title="Reset all settings to default and restart the app. This will NOT clear the database." />
			</Button>
		</div>
		<div class="relative flex-grow max-w-full flex-1 px-4 text-end mobile-text-center my-1">
			<Button variant="link" class="gap-2 text-xs !px-2" on:click={() => checkForAppUpdates()}>
				<span class="font-normal text-xs">Check for Updates</span>
			</Button>
		</div>
	</div>
</div>

<style lang="scss">
	.small-explanation {
		font-size: 0.7rem;
		font-weight: 300;
		padding: 0;
		background-color: inherit;

		&:not(code) {
			color: var(--bs-gray);
		}
	}

	.settings-links > div {
		@media (min-width: 576px) {
			font-size: 0.7rem;
			font-weight: 300;
			padding: 0;
			background-color: inherit;
			color: var(--bs-gray);
		}
	}

	tr {
		cursor: default;
		// &:not(.skip-hover):hover {
		// 	cursor: default;
		// 	color: var(--purple);
		// }
	}

	i {
		font-size: 1.5rem;
	}

	.mobile-text-center {
		@media (max-width: 576px) {
			text-align: center !important;
		}
	}
</style>
