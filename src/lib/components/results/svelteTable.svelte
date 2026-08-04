<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { createVirtualizer } from '@tanstack/svelte-virtual';
	import type { SvelteVirtualizer } from '@tanstack/svelte-virtual';
	import { documentsShown, ignoreDialogOpen, locationShown, preferLastOpened, shiftKeyPressed, compactViewMode, selectedResult, showResultTextPreview, noMoreResults, searchInProgress, showIconGrid, base64Images, isMac, resultsPageShown, tableVirtualizer } from '$lib/stores';
	import FileTypeIcon from '$lib/components/ui/FileTypeIcon.svelte';
	import { stringToHash, resetColumnSize } from '$lib/utils/miscUtils';
	import { clickRow } from '$lib/utils/fileUtils';
	import { trackEvent } from '@aptabase/web';
	import * as ContextMenu from "$lib/components/ui/context-menu";
	import ResultTextPreview from "./ResultTextPreview.svelte";
	import { openFileFolder, openFile, formatPath, startDragging } from '$lib/utils/searchItemUtils';
	import { createTableFromResults, getResultThumbnails, findBase64ImageObjectFromPath } from '$lib/utils/fileTable';
	// @ts-ignore
	import { Subscribe, Render } from 'svelte-headless-table';
	import Label from '../ui/label/label.svelte';
	import { loadMoreResults } from '$lib/utils/dbUtils';
	import { Check, LoaderCircle } from 'lucide-svelte';
	import IgnoreDialog from '$lib/components/settings/IgnoreDialog.svelte';

	let pathToIgnore = "";

	// The scrollable container that wraps this component (set by resultsTable.svelte).
	export let scrollElement: HTMLElement | null = null;

	// Only the rows visible in the scroll viewport are rendered; scrolling the
	// container recomputes the visible range. Rows are measured so their real
	// height is used for the scroll extent.
	let virtualizer: SvelteVirtualizer<HTMLElement, HTMLElement> | null = null;
	const tableVirtualizerStore = createVirtualizer<HTMLElement, HTMLElement>({
		count: 0,
		getScrollElement: () => scrollElement,
		estimateSize: () => ($compactViewMode ? 38 : 46),
		overscan: 12,
	});
	$: virtualizer = $tableVirtualizerStore;
	$: tableVirtualizer.set(virtualizer);
	$: virtualizer?.setOptions({ count: allRows.length });

	// Measure a virtual row and clean up its ResizeObserver when it scrolls out
	// of the rendered window (measureElement(null) disposes detached elements).
	function measureRow(node: HTMLElement) {
		virtualizer?.measureElement(node);
		return {
			destroy() {
				virtualizer?.measureElement(null);
			}
		};
	}

	// Infinite-scroll state: whether a load-more request is currently in flight.
	let loadingMore = false;
	// Sentinel element that triggers loading more results when it scrolls into view.
	let loadMoreSentinel: HTMLDivElement | null = null;
	let sentinelObserver: IntersectionObserver | null = null;

	$: if (loadMoreSentinel) {
		sentinelObserver?.disconnect();
		sentinelObserver = new IntersectionObserver(
			(entries) => {
				if (entries.some((entry) => entry.isIntersecting)) {
					void loadMoreThenExtend();
				}
			},
			// Pre-trigger when the sentinel comes within 300px of viewport bottom
			{ rootMargin: '0px 0px 300px 0px' }
		);
		sentinelObserver.observe(loadMoreSentinel);
	}

	onDestroy(() => {
		sentinelObserver?.disconnect();
	});

	// Render every row loaded so far. `rows` is the full (unpaged) store the
	// table produces from $documentsShown, so no local pagination is needed.
	$: allRows = $rows;

	async function loadMoreThenExtend() {
		if (loadingMore || $searchInProgress || $noMoreResults) return;
		loadingMore = true;
		try {
			await loadMoreResults();
		} finally {
			loadingMore = false;
		}
	}

	function showHideColumn(colID: string) {
		console.log("Hiding column", colID);
		trackEvent('right_click:resultTableHeaderContextMenu', {colID});
		resetColumnSize();
		hideForId[colID] = !hideForId[colID];
		if (colID === 'lastModified' || colID === 'lastOpened') {
			if (hideForId['lastModified']) {
				$preferLastOpened = true;
			}
		}
	}

	function createTableVars(dataRows: DocumentSearchResult[]) {
		const [table, columns] = createTableFromResults(dataRows);
		// @ts-ignore
		const { flatColumns, headerRows, rows, tableAttrs, tableBodyAttrs, pluginStates } = table.createViewModel(columns);
		const { hiddenColumnIds } = pluginStates.hideCols;
		const ids = flatColumns.map((c: any) => c.id);
		const labels = flatColumns.map((c: any) => c.header);
		const hideForId: Record<string, boolean> = Object.fromEntries(ids.map((id: any) => [id, false]));
		return { table, columns, flatColumns, headerRows, rows, tableAttrs, tableBodyAttrs, pluginStates, hiddenColumnIds, ids, labels, hideForId };
	}

	let { table, columns, flatColumns, headerRows, rows, tableAttrs, tableBodyAttrs, pluginStates, hiddenColumnIds, ids, labels, hideForId } = createTableVars($documentsShown);
	
	// HACK: hide columns by default
	// hideForId['size'] = true;
	hideForId['lastModified'] = $preferLastOpened;
	if ($locationShown === "browser history") {
		hideForId['lastOpened'] = true;
		hideForId['lastModified'] = false;
	} else {
		hideForId['lastOpened'] = !$preferLastOpened;
	}
	
	// @ts-ignore
	let columnsArray = columns.map((column: any) => ({ id: column.id, header: column.header }));

	$: if ($documentsShown) {
		console.log(">>> reloading... " + $documentsShown.length + " docs");
		
		({ table, columns, flatColumns, headerRows, rows, tableAttrs, tableBodyAttrs, pluginStates, hiddenColumnIds, ids, labels, hideForId } = createTableVars($documentsShown));
		
		hideForId['lastModified'] = $preferLastOpened;
		if ($locationShown === "browser history") {
			hideForId['lastOpened'] = false;
			hideForId['lastModified'] = true;
		} else {
			hideForId['lastOpened'] = !$preferLastOpened;
		}
		// @ts-ignore
		columnsArray = columns.map((column: any) => ({ id: column.id, header: column.header }));
		// Select and focus the first result only on a fresh search (page 0). When
		// infinite scroll appends more results, re-selecting and focusing the first
		// row would scroll the container back to the top and trap the user on the
		// first page.
		if ($resultsPageShown === 0) {
			// Start a fresh search at the top of the list.
			virtualizer?.scrollToIndex(0);
			$selectedResult = $documentsShown[0];
			let firstResult = document.querySelector('.result-0') as HTMLElement | null;
			if (firstResult) {
				firstResult.focus({ preventScroll: true });
			}
		}
		resetColumnSize();
	}
	
	$: $hiddenColumnIds = Object.entries(hideForId)
		.filter(([, hide]) => hide)
		.map(([id]) => id);

	function findBase64ImageObjectFromPathLocal(path: string) {
		let imageObject = $base64Images.find(image => image.path === path);
		console.log(">> imageObject?", imageObject);
		if (imageObject) {
			return imageObject;
		} else {
			return { path: '', base64: '' };
		}
	}

	onMount(async () => {
		// select the first result when loading new search results
		$selectedResult = $documentsShown[0];
		let firstResult = document.querySelector('.result-0') as HTMLElement | null;
		if (firstResult) {
			firstResult.focus({ preventScroll: true });
		}
		resetColumnSize();

		// always get thumbnails when the table is loaded for the first time
		console.log(">> sveltetable mount");
		
		getResultThumbnails($documentsShown);
  })
</script>

{#if $showIconGrid}
	<div id="parent-grid" class="flex flex-col">
		<div class={`file-grid p-2 ${$compactViewMode ? 'gap-2' : 'gap-4'}`}>
			{#each allRows as row (row.id)}
				<ContextMenu.Root>
					<ContextMenu.Trigger>
					<button
						id={stringToHash($documentsShown[Number(row.id)].path)}
						style="all: unset;"
						class={`icon-item w-full h-full p-1 grid items-center justify-between table-row result-${Number(row.id)} ${$compactViewMode ? 'compact-view' : ''}`}
						tabindex="0"
						on:focus={(e) => clickRow(e, $shiftKeyPressed)}
						on:click={(e) => clickRow(e, $shiftKeyPressed)}
						on:dblclick={() => openFile($documentsShown[Number(row.id)].path)}
						draggable="true"
						on:dragstart={(event) => startDragging($documentsShown[Number(row.id)].path)}
						title={$documentsShown[Number(row.id)].name}
					>
						<div class="flex justify-center">
							{#if ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes($documentsShown[Number(row.id)].file_type)}
								{#if $searchInProgress}
									<LoaderCircle class="mr-2 h-4 w-4 animate-spin" />
								{:else}
									<img src={"data:image/png;base64, " + findBase64ImageObjectFromPathLocal($documentsShown[Number(row.id)].path).base64} alt={$documentsShown[Number(row.id)].name} class={`img-thumbnail ${$compactViewMode ? 'compact-view' : ''}`} />
								{/if}
							{:else}
								<FileTypeIcon filetype={$documentsShown[Number(row.id)].file_type} extraClasses={`${$compactViewMode ? 'text-lg' : 'text-2xl'}`}/>
							{/if}
						</div>
						<div class="filename text-center p-1 w-full">
							{$documentsShown[Number(row.id)].name}
						</div>
					</button>
				</ContextMenu.Trigger>
				<ContextMenu.Content>
					{#if $documentsShown[Number(row.id)].file_type !== 'folder' && $documentsShown[Number(row.id)].last_parsed !== 0}
						<ContextMenu.Item on:click={() => {$showResultTextPreview = true; $selectedResult = $documentsShown[Number(row.id)];}}>
							Show Preview
						</ContextMenu.Item>
					{/if}
					<ContextMenu.Item>
						Open {$documentsShown[Number(row.id)].file_type === 'folder' ? 'Folder' : 'File'}
					</ContextMenu.Item>
					<ContextMenu.Sub>
						<ContextMenu.SubTrigger>Ignore</ContextMenu.SubTrigger>
						<ContextMenu.SubContent class="w-48">
							<ContextMenu.Item>Ignore this {$documentsShown[Number(row.id)].file_type === 'folder' ? 'folder' : 'file'}</ContextMenu.Item>
							<ContextMenu.Item>Ignore parent folder</ContextMenu.Item>
							{#if $documentsShown[Number(row.id)].file_type !== 'folder'}
								<ContextMenu.Item>Ignore this file's text</ContextMenu.Item>
							{/if}
						</ContextMenu.SubContent>
					</ContextMenu.Sub>
				</ContextMenu.Content>
			</ContextMenu.Root>
			{/each}
		</div>
		{#if $documentsShown.length > 0}
			<div class="infinite-footer" bind:this={loadMoreSentinel}>
				<div class="scroll-fade" aria-hidden="true"></div>
				<div class="flex w-full items-center justify-center gap-2 py-2" id="load-more-indicator">
					{#if $searchInProgress}
						<LoaderCircle class="h-4 w-4 animate-spin text-muted-foreground" />
						<Label class="font-normal text-sm text-muted-foreground">Loading more results&hellip;</Label>
					{:else if $noMoreResults}
						<span class="end-badge">Fin des résultats</span>
					{/if}
				</div>
			</div>
		{/if}
	</div>
{:else}
	<table {...$tableAttrs} class="block w-full relative border-spacing-0">
		<thead id="real-thead" class="sticky top-0 z-10 bg-white">
			{#each $headerRows as headerRow (headerRow.id)}
				<Subscribe rowAttrs={headerRow.attrs()} let:rowAttrs>
					<tr {...rowAttrs}>
						{#each headerRow.cells as cell (cell.id)}
							<Subscribe attrs={cell.attrs()} let:attrs props={cell.props()} let:props>
								<th
									{...attrs}
									class={`${cell.id}-col px-4 text-left align-middle font-medium text-muted-foreground ${$compactViewMode ? 'compact-view' : ''}`}
									role="button"
									tabindex="0"
									use:props.resize
									on:click={props.sort.toggle}
									class:sorted={props.sort.order !== undefined}
								>
									{#if cell.id === 'file_type'}
										<div class="header-grid justify-items-stretch items-center px-2">
											<div class="flex justify-items-start">
												<FileTypeIcon filetype="other" />
											</div>
											<div class="flex justify-end">
												{#if props.sort.order === 'asc'}
													<i class="bi bi-caret-up-fill" style="font-size: 0.5rem;" />
												{:else if props.sort.order === 'desc'}
													<i class="bi bi-caret-down-fill" style="font-size: 0.5rem;" />
												{/if}
											</div>
										</div>
									{:else}
										<ContextMenu.Root>
											<ContextMenu.Trigger>
												<div class="header-grid justify-items-stretch items-center px-2">
													<div class="flex justify-items-start">
														<Render of={cell.render()} />
													</div>
													<div class="flex justify-end">
														{#if props.sort.order === 'asc'}
															<i class="bi bi-caret-up-fill" style="font-size: 0.5rem;" />
														{:else if props.sort.order === 'desc'}
															<i class="bi bi-caret-down-fill" style="font-size: 0.5rem;" />
														{/if}
													</div>
												</div>
											</ContextMenu.Trigger>
											<ContextMenu.Content>
												{#each columnsArray as col}
													<ContextMenu.Item on:click={() => {showHideColumn(col.id)}}>
														<Check class={`mr-2 h-3 w-3 ${hideForId[col.id] ? 'text-white' : ''}`} />{col.header}
													</ContextMenu.Item>
												{/each}
											</ContextMenu.Content>
										</ContextMenu.Root>
									{/if}
									{#if !props.resize.disabled}
										<button
											aria-hidden="false"
											tabindex="-1"
											class="resizer"
											on:click|stopPropagation
											use:props.resize.drag
											use:props.resize.reset
										/>
									{/if}
								</th>
							</Subscribe>
						{/each}
					</tr>
				</Subscribe>
			{/each}
		</thead>
		{#if $documentsShown.length > 0}
			<tbody {...$tableBodyAttrs} style="position: relative; display: block; height: {virtualizer ? virtualizer.getTotalSize() : 0}px;">
				{#if virtualizer}
					{#each virtualizer.getVirtualItems() as virtualRow (virtualRow.key)}
						{@const row = allRows[virtualRow.index]}
						<Subscribe rowAttrs={row.attrs()} let:rowAttrs>
							<ContextMenu.Root>
								<ContextMenu.Trigger>
									<tr
										{...rowAttrs}
										id={stringToHash($documentsShown[Number(row.id)].path)}
										class={`table-row result-${Number(row.id)}`}
										role="button"
										tabindex="0"
										data-index={virtualRow.index}
										style="position: absolute; top: 0; left: 0; width: 100%; transform: translateY({virtualRow.start}px);"
										use:measureRow
										on:focus={(e) => clickRow(e, $shiftKeyPressed)}
										on:click={(e) => clickRow(e, $shiftKeyPressed)}
										on:dblclick={() => openFile($documentsShown[Number(row.id)].path)}
										draggable="true"
										on:dragstart={(event) => startDragging($documentsShown[Number(row.id)].path)}
									>
										{#each row.cells as cell (cell.id)}
											<Subscribe attrs={cell.attrs()} let:attrs>
												<td {...attrs} class={`${cell.id}-col ${$compactViewMode ? 'compact-view' : ''} ${cell.id === 'file_type' ? 'justify-center' : ''}`}
													title={cell.id === 'name' || cell.id === 'path' ? String(cell.render()) : ''}
												>
													{#if cell.id === 'file_type'}
														<FileTypeIcon filetype={String(cell.render())} />
													{:else if cell.id === 'name'}
														{#if $documentsShown[Number(row.id)].last_parsed > 0}
															<span class="flex items-center gap-1">
																<i class="bi bi-check-circle fs-small" title="Item contents scanned" style="font-size: 8px; color: var(--bs-success);"></i>
																<Render of={cell.render()} />
															</span>
														{:else}
															<span><Render of={cell.render()} /></span>
														{/if}
													{:else if cell.id === 'path'}
														<button class="w-full text-left truncate hover:underline hover:cursor-pointer" on:click={() => openFileFolder(cell.render().toString())}>
															<Render of={formatPath(cell.render().toString())} />
														</button>
													{:else}
														<span><Render of={cell.render()} /></span>
													{/if}
												</td>
											</Subscribe>
										{/each}
									</tr>
								</ContextMenu.Trigger>
								<ContextMenu.Content>
									{#if $documentsShown[Number(row.id)].file_type !== 'folder' && $documentsShown[Number(row.id)].last_parsed !== 0}
										<ContextMenu.Item on:click={() => {$showResultTextPreview = true; $selectedResult = $documentsShown[Number(row.id)];}}>
											Show Preview
										</ContextMenu.Item>
									{/if}
									<ContextMenu.Item on:click={() => {
										$selectedResult = $documentsShown[Number(row.id)];
										openFileFolder($selectedResult.path)}
									}>
										Open {$documentsShown[Number(row.id)].file_type === 'folder' ? 'Folder' : 'File'}
									</ContextMenu.Item>
									<ContextMenu.Sub>
										<ContextMenu.SubTrigger>Ignore</ContextMenu.SubTrigger>
										<ContextMenu.SubContent class="w-48">
											<ContextMenu.Item on:click={() => {
												$ignoreDialogOpen = true;
												$selectedResult = $documentsShown[Number(row.id)];
												pathToIgnore = $selectedResult.path;
											}}>
												Ignore this {row.cells[0].render().toString() === 'folder' ? 'folder' : 'file'}
											</ContextMenu.Item>
											<ContextMenu.Item on:click={() => {
												$ignoreDialogOpen = true; 
												$selectedResult = $documentsShown[Number(row.id)];
												if ($isMac) pathToIgnore = $selectedResult.path.split('/').slice(0, -1).join('/');
												else pathToIgnore = $selectedResult.path.split('\\').slice(0, -1).join('\\');
											}}>
												Ignore parent folder
											</ContextMenu.Item>
										</ContextMenu.SubContent>
									</ContextMenu.Sub>
								</ContextMenu.Content>
							</ContextMenu.Root>
						</Subscribe>
					{/each}
				{/if}
			</tbody>
		{/if}
	</table>
	{#if $documentsShown.length > 0}
		<div class="infinite-footer" bind:this={loadMoreSentinel}>
			<div class="scroll-fade" aria-hidden="true"></div>
			<div class="flex w-full items-center justify-center gap-2 py-2" id="load-more-indicator">
				{#if $searchInProgress}
					<LoaderCircle class="h-4 w-4 animate-spin text-muted-foreground" />
					<Label class="font-normal text-sm text-muted-foreground">Loading more results&hellip;</Label>
				{:else if $noMoreResults}
					<span class="end-badge">Fin des résultats</span>
				{/if}
			</div>
		</div>
	{/if}
{/if}

{#key $selectedResult}
	<ResultTextPreview open={$showResultTextPreview} />
{/key}

<IgnoreDialog dialogOpen={$ignoreDialogOpen} {pathToIgnore} />

<style lang="scss">
	tr {
		cursor: default;
		outline: none;
	}
	td {
		position: relative;
		overflow: hidden;
		span {
			display: block;
			width: 100%;
			overflow: hidden;
			text-overflow: ellipsis;
			white-space: nowrap;
		}
	}
	// regular padding
	td {
		font-size: 0.9rem;
		padding: 8px;
	}
	// compact padding
	td.compact-view,
	th.compact-view {
		font-size: 0.8rem;
		padding: 4px !important;
	}
	th {
		// border-bottom: 1px solid var(--light-purple);
		text-align: center;
		font-size: 0.9rem !important;
		font-weight: 600;
		padding: 4px 4px !important;
	}
	th:last-of-type {
		overflow-x: clip;
	}
	.type-col,
	.size-col,
	.lastModified-col,
	.lastOpened-col {
		text-align: center;
	}
	// selected row
	.selected {
		background-color: var(--purple) !important;
		color: white;
		.pinned,
		.pin {
			color: white;
		}
		&.grayscale {
			filter: grayscale(.7);
			background-color: transparent !important;
		}
	}
	// pinned rows
	.pinned {
		color: var(--hot-pink);
	}
	.pin {
		color: var(--bs-body-color);
	}
	.pin:hover {
		color: var(--hot-pink);
	}
	// resize column handle
	th {
		position: relative; // need this to position the resizer
	}
	th .resizer {
		width: 1px;
		position: absolute;
		top: 0%;
		bottom: 0;
		right: -4px;
		height: 100%;
		background: black;
		opacity: 0.05;
	}

	.header-grid {
		display: grid; 
		grid-template-columns: 1.5fr 0.5fr; 
		grid-template-rows: 1fr; 
		gap: 0px 0px; 
		grid-template-areas: 
			". ."; 
	}

	// table head fixed
	thead,
	tbody tr {
		display: table;
		width: 100%;
		table-layout: fixed; /* even columns width , fix width of table too*/
	}

	tbody {
		display: block;
	}

	:global::-webkit-scrollbar {
		width: 0px;
		background: transparent; /* make scrollbar transparent */
	}

	// Scroll offset is owned by the resultsTable.svelte wrapper viewport
	// (scrollElement); neither tbody nor #parent-grid may create their own
	// vertical scrollbar or they'd fight the virtualizer for the scroll extent.
	#parent-grid {
		overflow-x: hidden;
	}
  .img-thumbnail {
    max-height: 72px;
    max-width: 96px;
  }
  .img-thumbnail.compact-view {
    max-height: 48px;
    max-width: 64px;
  }
  .file-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(100px, 1fr));
  }
  .filename {
    font-size: 0.75rem;
    width: 100px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  // Infinite-scroll footer with a gradient fade over the bottom of the results.
  .infinite-footer {
    position: relative;
    width: 100%;
  }
  .scroll-fade {
    position: absolute;
    inset-inline: 0;
    bottom: 100%;
    height: 96px;
    // Gradient fade hiding the end of the list (adapts to the app background).
    background: linear-gradient(to top, hsl(var(--background)), transparent);
    pointer-events: none;
    z-index: 5;
  }
  // Polished end-of-results badge shown when every loaded page has been fetched.
  .end-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.3rem 0.9rem;
    border-radius: 9999px;
    font-size: 0.8rem;
    font-weight: 500;
    letter-spacing: 0.01em;
    color: var(--bs-secondary-color, inherit);
    background: hsl(var(--background));
    border: 1px solid hsl(var(--border) / 0.6);
    box-shadow: 0 1px 2px hsl(var(--foreground) / 0.06);
    opacity: 0.9;
  }
</style>
