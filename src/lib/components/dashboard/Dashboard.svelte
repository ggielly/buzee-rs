<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { invoke } from '@tauri-apps/api/core';
	import { goto } from '$app/navigation';
	import * as Card from '$lib/components/ui/card/index.js';
	import Progress from '$lib/components/ui/progress/progress.svelte';
	import Separator from '$lib/components/ui/separator/separator.svelte';
	import { Button } from '$lib/components/ui/button';
	import {
		Search,
		Files,
		FolderOpen,
		Database,
		FileCheck2,
		FileClock,
		Pin,
		HardDrive,
		RefreshCw,
		ArrowRight
	} from 'lucide-svelte';
	import { readableFileSize } from '$lib/utils/miscUtils';
	import { openFileFolder } from '$lib/utils/searchItemUtils';
	import { trackEvent } from '@aptabase/web';

	let loading = true;
	let error = '';
	let stats: DashboardStats | null = null;

	const typeColors: Record<string, string> = {
		doc: 'var(--purple)',
		docx: 'var(--purple)',
		pdf: '#e15554',
		xls: '#00a86b',
		xlsx: '#00a86b',
		ppt: 'var(--hot-pink)',
		pptx: 'var(--hot-pink)',
		png: '#4a90d9',
		jpg: '#4a90d9',
		jpeg: '#4a90d9',
		txt: '#6c757d',
		md: '#6c757d',
		folder: '#f4b942',
		other: '#adb5bd'
	};

	function colorFor(type: string): string {
		return typeColors[type] ?? typeColors.other;
	}

	function formatDate(unix: number): string {
		if (!unix) return 'Never';
		return new Date(unix * 1000).toLocaleDateString(undefined, {
			year: 'numeric',
			month: 'short',
			day: 'numeric'
		});
	}

	function formatHeading(num: number): string {
		if (num >= 1_000_000) return (num / 1_000_000).toFixed(1) + 'M';
		if (num >= 1_000) return (num / 1_000).toFixed(1) + 'k';
		return String(num);
	}

	function percent(part: number, whole: number): string {
		if (!whole) return '0';
		return ((100 * part) / whole).toFixed(1);
	}

	function formatCountdown(secs: number): string {
		if (secs <= 0) return 'now';
		const m = Math.floor(secs / 60);
		const s = secs % 60;
		if (m >= 60) {
			const h = Math.floor(m / 60);
			return `${h}h ${m % 60}m`;
		}
		if (m === 0) return `${s}s`;
		return `${m}m ${s}s`;
	}

	async function load() {
		loading = true;
		error = '';
		try {
			stats = (await invoke('get_dashboard_stats')) as DashboardStats;
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	onMount(load);
</script>

<div class="dashboard" in:fade={{ delay: 0, duration: 400 }}>
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div>
			<h3 class="text-xl font-semibold leading-tight tracking-tight flex items-center gap-2">
				<Database class="h-5 w-5 text-muted-foreground" />
				Dashboard
			</h3>
			<p class="text-sm text-muted-foreground">An overview of everything Buzee is indexing for you.</p>
		</div>
		<div class="flex items-center gap-2">
			<Button variant="outline" size="sm" on:click={() => goto('/search')}>
				<Search class="mr-2 h-4 w-4" /> Search
			</Button>
			<Button variant="outline" size="sm" on:click={load} title="Refresh statistics">
				<RefreshCw class={`mr-2 h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
				Refresh
			</Button>
		</div>
	</div>

	<Separator class="my-4" />

	{#if loading && !stats}
		<div class="flex items-center justify-center py-16 text-muted-foreground">
			<RefreshCw class="mr-2 h-5 w-5 animate-spin" /> Loading statistics…
		</div>
	{:else if error}
		<div class="rounded-lg border border-destructive/50 p-6 text-center text-destructive">
			Couldn't load statistics: {error}
			<Button variant="outline" size="sm" class="mt-3" on:click={load}>Try again</Button>
		</div>
	{:else if stats}
		<!-- KPI cards -->
		<div class="grid grid-cols-2 gap-4 md:grid-cols-3 xl:grid-cols-6">
			<Card.Root>
				<Card.Header class="flex flex-row items-center justify-between space-y-0 pb-2">
					<Card.Title class="text-xs font-medium text-muted-foreground">Total Files</Card.Title>
					<Files class="h-4 w-4 text-muted-foreground" />
				</Card.Header>
				<Card.Content>
					<div class="text-2xl font-bold">{formatHeading(stats.total_files)}</div>
					<p class="text-xs text-muted-foreground">
						{readableFileSize(stats.total_size_bytes)} indexed
					</p>
				</Card.Content>
			</Card.Root>

			<Card.Root>
				<Card.Header class="flex flex-row items-center justify-between space-y-0 pb-2">
					<Card.Title class="text-xs font-medium text-muted-foreground">Folders</Card.Title>
					<FolderOpen class="h-4 w-4 text-muted-foreground" />
				</Card.Header>
				<Card.Content>
					<div class="text-2xl font-bold">{formatHeading(stats.total_folders)}</div>
					<p class="text-xs text-muted-foreground">Directories indexed</p>
				</Card.Content>
			</Card.Root>

			<Card.Root>
				<Card.Header class="flex flex-row items-center justify-between space-y-0 pb-2">
					<Card.Title class="text-xs font-medium text-muted-foreground">Files parsed</Card.Title>
					<FileCheck2 class="h-4 w-4 text-muted-foreground" />
				</Card.Header>
				<Card.Content>
					<div class="text-2xl font-bold">{formatHeading(stats.parsed_files)}</div>
					<p class="text-xs text-muted-foreground">
						{readableFileSize(stats.parsed_total_size_bytes)} of text scanned
					</p>
				</Card.Content>
			</Card.Root>

			<Card.Root>
				<Card.Header class="flex flex-row items-center justify-between space-y-0 pb-2">
					<Card.Title class="text-xs font-medium text-muted-foreground">Not scanned yet</Card.Title>
					<FileClock class="h-4 w-4 text-muted-foreground" />
				</Card.Header>
				<Card.Content>
					<div class="text-2xl font-bold">{formatHeading(stats.unparsed_files)}</div>
					<p class="text-xs text-muted-foreground">Awaiting text extraction</p>
				</Card.Content>
			</Card.Root>

			<Card.Root>
				<Card.Header class="flex flex-row items-center justify-between space-y-0 pb-2">
					<Card.Title class="text-xs font-medium text-muted-foreground">Pinned</Card.Title>
					<Pin class="h-4 w-4 text-muted-foreground" />
				</Card.Header>
				<Card.Content>
					<div class="text-2xl font-bold">{formatHeading(stats.pinned_files)}</div>
					<p class="text-xs text-muted-foreground">Pinned documents</p>
				</Card.Content>
			</Card.Root>

			<Card.Root>
				<Card.Header class="flex flex-row items-center justify-between space-y-0 pb-2">
					<Card.Title class="text-xs font-medium text-muted-foreground">Database</Card.Title>
					<HardDrive class="h-4 w-4 text-muted-foreground" />
				</Card.Header>
				<Card.Content>
					<div class="text-2xl font-bold">{readableFileSize(stats.database_size_bytes)}</div>
					<p class="text-xs text-muted-foreground">
						Largest file {readableFileSize(stats.largest_file_size_bytes)}
					</p>
				</Card.Content>
			</Card.Root>
		</div>

		<!-- Scan status row -->
		<div class="mt-4 flex flex-wrap items-center gap-4 rounded-lg border bg-muted/40 px-4 py-3 text-sm">
			<span
				class="inline-flex items-center gap-2"
				class:status-scanning={stats.scan_running}
			>
				<RefreshCw class="h-4 w-4 {stats.scan_running ? 'animate-spin' : ''}" />
				{#if stats.scan_running}
					Scan is currently running…
				{:else if stats.auto_sync_enabled}
					Auto-scan enabled
				{:else}
					Auto-scan is off
				{/if}
			</span>
			<span>Last scan: {formatDate(stats.last_scan_time)}</span>
			{#if stats.next_scan_in_seconds > 0}
				<span>Next scan in {formatCountdown(stats.next_scan_in_seconds)}</span>
			{/if}
		</div>

		<!-- File type distribution -->
		<div class="mt-6 grid grid-cols-1 gap-4 lg:grid-cols-2">
			<Card.Root>
				<Card.Header>
					<Card.Title>Files by type</Card.Title>
					<Card.Description>
						Distribution across the index ({stats.filetype_counts.length} types)
					</Card.Description>
				</Card.Header>
				<Card.Content>
					{#if stats.filetype_counts.length > 0}
						<div class="mb-3 flex h-3 w-full overflow-hidden rounded-full">
							{#each stats.filetype_counts as b, i}
								<div
									style={`width: ${100 * b.count / stats.total_files}%; background-color: ${colorFor(b.file_type)}`}
									title={`${b.file_type} (${b.count})`}
									class="h-full"
								></div>
							{/each}
						</div>
						<div class="max-h-64 space-y-1.5 overflow-y-auto pr-1">
							{#each stats.filetype_counts as f}
								<div class="flex items-center justify-between text-sm">
									<span class="flex items-center gap-2">
										<span
											class="inline-block h-2.5 w-2.5 rounded-sm"
											style={`background-color: ${colorFor(f.file_type)}`}
										></span>
										<span class="font-medium">{f.file_type || 'unknown'}</span>
										<span class="text-muted-foreground">{f.count}</span>
									</span>
									<span class="text-xs text-muted-foreground">
										{readableFileSize(f.size_bytes)} · {percent(f.count, stats.total_files)}%
									</span>
								</div>
							{/each}
						</div>
					{:else}
						<p class="text-sm text-muted-foreground">No files indexed yet.</p>
					{/if}
				</Card.Content>
			</Card.Root>

			<!-- Largest files -->
			<Card.Root>
				<Card.Header>
					<Card.Title>Largest files</Card.Title>
					<Card.Description>Top {stats.top_largest.length} by size</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-2">
					{#each stats.top_largest as file}
						<button
							type="button"
							class="group flex w-full items-center justify-between rounded-md border bg-background px-3 py-2 text-left text-sm hover:bg-muted/40"
							on:click={() => openFileFolder(file.path)}
						>
							<span class="truncate pr-3 font-medium">{file.name}</span>
							<span class="shrink-0 text-xs text-muted-foreground">
								{readableFileSize(file.size)}
							</span>
						</button>
					{/each}
				</Card.Content>
			</Card.Root>
		</div>

		<!-- Recently modified -->
		<div class="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
			<Card.Root>
				<Card.Header>
					<Card.Title>Recently modified</Card.Title>
					<Card.Description>Most recently changed documents</Card.Description>
				</Card.Header>
				<Card.Content class="space-y-2">
					{#each stats.top_recent as file}
						<button
							type="button"
							class="group flex w-full items-center justify-between gap-3 rounded-md border px-3 py-2 text-sm hover:bg-muted/40"
							on:click={() => openFileFolder(file.path)}
						>
							<span class="flex min-w-0 items-center gap-2">
								<span class="truncate font-medium">{file.name}</span>
								<span class="shrink-0 rounded bg-muted px-1.5 py-0.5 text-[10px] uppercase text-muted-foreground">
									{file.file_type}
								</span>
							</span>
							<span class="shrink-0 text-xs text-muted-foreground">
								{formatDate(file.last_modified)}
							</span>
						</button>
					{:else}
						<p class="text-sm text-muted-foreground">No documents yet. Run a scan to populate the dashboard.</p>
					{/each}
				</Card.Content>
			</Card.Root>

			<!-- Parse progress -->
			<Card.Root>
				<Card.Header>
					<Card.Title>Parsing progress</Card.Title>
					<Card.Description>
						How much of your library's text has been extracted
					</Card.Description>
				</Card.Header>
				<Card.Content>
					{#if stats.total_files > 0}
						<Progress value={100 * stats.parsed_files / stats.total_files} class="h-3" />
						<div class="mt-3 flex justify-between text-sm">
							<span>{stats.parsed_files} parsed</span>
							<span>{stats.unparsed_files} remaining</span>
						</div>
					{:else}
						<p class="text-sm text-muted-foreground">Nothing indexed yet.</p>
					{/if}
					<Separator class="my-4" />
					<div class="space-y-1 text-sm">
						<div class="flex justify-between">
							<span class="text-muted-foreground">Average file size</span>
							<span class="font-medium">{readableFileSize(stats.average_size_bytes)}</span>
						</div>
						<div class="flex justify-between">
							<span class="text-muted-foreground">Files with frecency</span>
							<span class="font-medium">{stats.most_frequent_count}</span>
						</div>
					</div>
				</Card.Content>
			</Card.Root>
		</div>
	{/if}
</div>

<style lang="scss">
	.status-scanning {
		color: var(--purple);
	}
</style>