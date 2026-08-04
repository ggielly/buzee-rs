<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { onMount } from 'svelte';
	import { clickRow, selectAllRows } from './fileUtils';
	import { isMac, documentsShown, shiftKeyPressed, metaKeyPressed, showResultTextPreview, selectedResult, showIconGrid, searchSuggestionsDialogOpen, tableVirtualizer } from '$lib/stores';
	import { trackEvent } from '@aptabase/web';
	import { goto } from '$app/navigation';
	import { page } from "$app/stores";
	

	const allowedKeys = [
		'Space',
		'Enter',
		'KeyO',
		'KeyA',
		'KeyF',
		'KeyK',
		'KeyP',
		'KeyN',
		'ShiftLeft',
		'ShiftRight',
		'Tab',
		'MetaLeft',
		'MetaRight',
		'ArrowDown',
		'ArrowUp',
		'ArrowLeft',
		'ArrowRight',
		'Escape',
		'CommandOrControl+A',
		'Digit1',
		'Digit2',
		'Digit3',
		'Digit4',
		'Digit5',
		'Digit6',
		'Digit7',
		'Digit8',
		'Digit9',
	];

	const eventPrefix = 'keyboardListener:';

	// Move the selection to the result at `index`. In the table view only the
	// visible rows are rendered (virtualized), so the target is scrolled into
	// view first and then selected/focused once it is in the DOM.
	function selectResultByIndex(index: number) {
		if (index < 0 || index >= $documentsShown.length) return;
		$selectedResult = $documentsShown[index];
		const selectAndFocus = () => {
			const rowEl = document.querySelector(`.result-${index}`) as HTMLElement | null;
			if (rowEl) {
				document.querySelectorAll('.table-row.selected').forEach((r) => r.classList.remove('selected'));
				clickRow(
					{ currentTarget: rowEl } as MouseEvent & { currentTarget: EventTarget & HTMLDivElement },
					$shiftKeyPressed
				);
			}
		};
		if ($showIconGrid) {
			// Grid rows are all rendered; select immediately.
			selectAndFocus();
		} else {
			// Table view: bring the target into view, then select once rendered.
			$tableVirtualizer?.scrollToIndex(index, { align: 'auto' });
			setTimeout(selectAndFocus, 60);
		}
	}

	function keydownListener(e: KeyboardEvent) {
		if (e.code === 'MetaLeft' || e.code === 'MetaRight' || e.code === 'ControlLeft' || e.code === 'ControlRight') {
			console.log('meta key down');
			$metaKeyPressed = true;
		}
		if (e.code === 'ShiftLeft' || e.code === 'ShiftRight') {
			console.log('shift key down');
			$shiftKeyPressed = true;
		}
		if (document.activeElement instanceof HTMLInputElement) return;
		if (allowedKeys.indexOf(e.code) < 0) return;

		// if on search page
		if ($page.route.id === '/search' ) {
			if (e.code === 'KeyN') {
				e.preventDefault();
				// Scroll the results container down (infinite scroll loads more).
				const tbody = document.querySelector('tbody') as HTMLElement | null;
				const grid = document.querySelector('#parent-grid') as HTMLElement | null;
				const el = tbody || grid;
				if (el) el.scrollBy({ top: 400, behavior: 'smooth' });
			}
			if (e.code === 'KeyP') {
				e.preventDefault();
				const tbody = document.querySelector('tbody') as HTMLElement | null;
				const grid = document.querySelector('#parent-grid') as HTMLElement | null;
				const el = tbody || grid;
				if (el) el.scrollBy({ top: -400, behavior: 'smooth' });
			}
			if (e.code === 'Escape') {
				e.preventDefault();
				trackEvent(eventPrefix + 'deselectAllRows');
				selectAllRows(true);
				document.body.focus();
				return;
			}

			const selectedElement = document.activeElement as HTMLElement;
			let thisResultIndex: string | undefined = '-1';

			if ($metaKeyPressed && e.code === 'KeyA' && document.activeElement?.tagName !== 'INPUT' && document.activeElement?.tagName !== 'TEXTAREA') {
				e.preventDefault();
				trackEvent(eventPrefix + 'selectAllRow');
				selectAllRows(false);
				return;
			}

			// If a result is selected
			if (selectedElement?.classList.contains('selected')) {
				thisResultIndex = Array.from(selectedElement?.classList)
					.find((className) => className.startsWith('result-'))
					?.split('-')[1];
				let result = $documentsShown[Number(thisResultIndex)];

				console.log("thisResultIndex:", thisResultIndex);
				console.log("meta key pressed:", $metaKeyPressed);

				if (e.code === 'Space') {
					e.preventDefault();
					trackEvent(eventPrefix + 'openQuickLook');
					// window.electronAPI?.openQuickLook(result.path);
					invoke("open_quicklook", { filePath: result.path })
				} else if (e.code === 'Enter') {
					e.preventDefault();
					trackEvent(eventPrefix + 'openFile');
					// window.electronAPI?.openFile(result.path);
					invoke("open_file_or_folder", { filePath: result.path })
				} else if (e.code === 'ArrowDown' && $metaKeyPressed && $isMac) {
					e.preventDefault();
					// window.electronAPI?.openFile(result.path);
					invoke("open_file_or_folder", { filePath: result.path })
				} else if (e.code === 'KeyO') {
					e.preventDefault();
					// window.electronAPI?.openFileFolder(result.path);
					invoke("open_folder_containing_file", { filePath: result.path })
				} else if (e.code === 'KeyP' && $shiftKeyPressed) {
					e.preventDefault();
					console.log(result);
					
					if (result.file_type !== 'folder' && result.last_parsed !== 0) {
						$showResultTextPreview = true;
						$selectedResult = result;
					}
				} else if (e.code === 'Tab' && $shiftKeyPressed) {
					$shiftKeyPressed = false;
				} else if (e.code === 'KeyP') {
					e.preventDefault();
					// togglePinState();
				} else if ((!$showIconGrid && e.code === 'ArrowUp') || ($showIconGrid && e.code === 'ArrowLeft')) {
					e.preventDefault();
					if (document.getElementsByClassName('selected').length > 2) {
						trackEvent(eventPrefix + 'deselectAllRows');
						selectAllRows(true);
					}
					const current = Number(thisResultIndex);
					selectResultByIndex(current >= 0 ? current - 1 : -1);
					return;
				} else if ((!$showIconGrid && e.code === 'ArrowDown') || ($showIconGrid && e.code === 'ArrowRight')) {
					e.preventDefault();
					if (document.getElementsByClassName('selected').length > 2) {
						trackEvent(eventPrefix + 'deselectAllRows');
						selectAllRows(true);
					}
					const current = Number(thisResultIndex);
					selectResultByIndex(current + 1);
					return;
				}
			}
		}

		if ($metaKeyPressed && (e.code === 'KeyF' || e.code === 'KeyK')) {
			e.preventDefault();
			if ($shiftKeyPressed) {
				console.log('Cmd + Shift + F');
				trackEvent(eventPrefix + 'toggleAppMode');
				// window.electronAPI?.toggleAppMode();
			} else {
				console.log('Cmd + F');
				trackEvent(eventPrefix + 'focusSearchBar');
				// if page is not /search, go to that page
				if (window.location.pathname !== '/search') {
					goto('/search?highlight-search-bar=true');
				}
				// focus the search bar
				(document.querySelector('#search-input') as HTMLElement)?.click();
				// focus the search bar in the cmdk dialog (input tag with [data-cmdk-input] attribute)
				if ($searchSuggestionsDialogOpen) {
					(document.querySelector('[data-cmdk-input]') as HTMLElement)?.focus();
				}
			}
			return;
		}

		
	}

	function keyupListener(e: KeyboardEvent) {
		if (e.code === 'ShiftLeft' || e.code === 'ShiftRight') {
			console.log('shift key up');
			$shiftKeyPressed = false;
		}
		if (e.code === 'MetaLeft' || e.code === 'MetaRight' || e.code === 'ControlLeft' || e.code === 'ControlRight') {
			console.log('meta key up');
			$metaKeyPressed = false;
		}
		// HACK to prevent meta and shift key values from getting stuck
		if (['MetaLeft','MetaRight','ControlLeft','ControlRight','ShiftLeft','ShiftRight'].indexOf(e.code) < 0 
			&& ($searchSuggestionsDialogOpen && document.activeElement?.hasAttribute('data-cmdk-input'))) {
			console.log("hacker");
			$metaKeyPressed = false;
			$shiftKeyPressed = false;
		}
	}

	onMount(() => {
		document.addEventListener('keyup', keyupListener);
		document.addEventListener('keydown', keydownListener);
	});
</script>
