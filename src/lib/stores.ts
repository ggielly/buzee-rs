import { writable } from 'svelte/store'
import type { SvelteVirtualizer } from '@tanstack/svelte-virtual'

// 1. Get the value out of storage on load.
// let storedSearchQuery = localStorage.searchQuery;
// let storedSearchTrigger = localStorage.searchTrigger;
// let storedSearchResults = localStorage.searchResults;
// let storedSearchHistory = localStorage.searchHistory;
// let storedTimeTaken = localStorage.timeTaken;
// let storedNumResultsReceived = localStorage.numResultsReceived;
// let storedResultFolderPaths = localStorage.resultFolderPaths;
// let storedResultFolderObjects = localStorage.resultFolderObjects;
let storedCompactViewMode = false;
if (typeof window !== "undefined") {
  storedCompactViewMode = localStorage.compactViewMode === 'true';
}

// 1.5 Not storing values in localStorage yet because then the last search shows up on each new load.
let storedSearchQuery = '';
let storedDocumentsShown: DocumentSearchResult[] = [];
// let storedSearchTrigger = false;
// let storedSearchResults = [];
// let storedSearchHistory = [];
// let storedTimeTaken = 0;
// let storedNumResultsReceived = 0;
// let storedDBStats = {};
// let storedCurrentCollection = 'Everything';
// let currentUser = null;
let storedSelectedResult: DocumentSearchResult = {
  id: 0,
  source_domain: '',
  created_at: 0,
  name: '',
  path: '',
  size: 0,
  file_type: '',
  last_modified: 0,
  last_opened: 0,
  last_synced: 0,
  last_parsed: 0,
  is_pinned: false,
  freceny_rank: 0,
  frecency_last_accessed: 0,
  comment: null,
};

let storedAllowedExtensions: FileTypesDropdown = {
  categories: [],
  items: []
};

let storedAllowedLocations = ["my computer", "browser history"];

let storedDateLimitUNIX: ParsedDatesUNIX = {
  start: '',
  end: '',
  text: ''
};

let storedFileText: string[] = [];
let storedSearchSuggestions: string[] = [];
let storedIgnoredPaths: IgnoreListType[] = [];
let storedBase64Images: Base64ImageObject[] = [];

// 2. Set the stored value or a sane default.
export const userPreferences = writable({
  "automatic_background_sync": true,
  "detailed_scan": true,
  "first_launch_done": true,
  "global_shortcut": "Alt+Space",
  "global_shortcut_enabled": true,
  "show_search_suggestions": true,
  "launch_at_startup": true,
  "onboarding_done": false,
  "show_in_dock": true,
  "roadmap_survey_answered": false,
  "parse_pdfs": true,
  "manual_setup": false,
  "enable_logs": false,
  "pdf_max_ocr_pages": 150,
  "ocr_threads": 1,
  "ocr_sort_order": "size_asc",
})
export const pagePath = writable("")
export const isMac = writable(false)
export const pinMode = writable(false)
export const showIconGrid = writable(false)
export const cronJobSet = writable(false)
export const onSearchPage = writable(false)
export const onboardingDone = writable(false)
export const syncStatus = writable(false)
export const disableInteraction = writable(false)
export const searchQuery = writable(storedSearchQuery || '')
export const searchSuggestions = writable(storedSearchSuggestions || [])
export const documentsShown = writable(storedDocumentsShown || [])
export const filetypeShown = writable('any')
export const locationShown = writable("my computer")
export const allowedExtensions = writable(storedAllowedExtensions);
export const allowedLocations = writable(storedAllowedLocations);
export const resultsPageShown = writable(0)
export const resultsPerPage = writable(50)
export const statusMessage = writable("")
export const compactViewMode = writable(storedCompactViewMode || false)
export const selectedResult = writable(storedSelectedResult || {})
export const selectedResultText = writable(storedFileText || [])
export const ignoredPaths = writable(storedIgnoredPaths || [])
export const shiftKeyPressed = writable(false);
export const metaKeyPressed = writable(false);
export const mouseDown = writable(false);
export const searchInProgress = writable(false);
export const base64SearchInProgress = writable(false);
export const dbCreationInProgress = writable(false);
export const windowBlurred = writable(false);
export const base64Images = writable(storedBase64Images || [])

// Maximum number of base64 thumbnails to keep in memory. Beyond this, the
// least-recently-added entry is evicted to avoid unbounded memory growth
// during long search sessions.
export const MAX_CACHED_THUMBNAILS = 80;

// Add or refresh a thumbnail in LRU order: an existing entry is moved to the
// newest slot (unique per path) and the oldest entry is evicted once the cache
// exceeds its cap. Ordering by most-recently-added mirrors the on-screen
// scroll window, so the scrolled-away rows are the first to be dropped.
export function upsertBase64Image(image: Base64ImageObject) {
  base64Images.update((images) => {
    const next = images.filter((i) => i.path !== image.path);
    next.push(image);
    if (next.length > MAX_CACHED_THUMBNAILS) {
      next.splice(0, next.length - MAX_CACHED_THUMBNAILS);
    }
    return next;
  });
}

// Clear the whole thumbnail cache (used when starting a fresh search).
export function clearBase64Images() {
  base64Images.set([]);
}
export const preferLastOpened = writable(false);
export const showResultTextPreview = writable(false);
export const noMoreResults = writable(false);
export const searchSuggestionsDialogOpen = writable(false);
export const searchFiltersOpen = writable(false);
export const ignoreDialogOpen = writable(false);
export const dateLimitUNIX = writable(storedDateLimitUNIX || null)

export interface AppStatistics {
  status: string;
  total_files: number;
  parsed_files: number;
  database_size_bytes: number;
  last_scan_time: number;
  next_scan_in_seconds: number;
  auto_sync_enabled: boolean;
}

// Live statistics fetched from the backend for the status bar.
export const appStatistics = writable<AppStatistics | null>(null);

// Dark mode theme. Persisted locally so it survives restarts.
let storedDarkMode = false;
if (typeof window !== "undefined") {
  storedDarkMode = localStorage.getItem('darkMode') === 'true';
}
export const darkMode = writable(storedDarkMode);
if (typeof window !== "undefined") {
  darkMode.subscribe((value) => { localStorage.darkMode = value; });
}

// Row virtualizer instance set by svelteTable.svelte; read by the keyboard
// listeners to scroll a result index into view after arrow-key navigation.
export const tableVirtualizer = writable<SvelteVirtualizer<HTMLElement, HTMLElement> | null>(null);

// 3. Anytime a display preference changes, update the local storage value.
// Only lightweight UI preferences are persisted — never the result sets or the
// search query, which would bloat the 5MB WebView quota and leak searched text.
if(typeof window !== "undefined") {
  pinMode.subscribe(value => { localStorage.pinMode = value })
  filetypeShown.subscribe(value => { localStorage.filetypeShown = value })
  resultsPerPage.subscribe(value => { localStorage.resultsPerPage = value })
  compactViewMode.subscribe(value => { localStorage.compactViewMode = value })
  // searchTrigger.subscribe(value => { localStorage.searchTrigger = value })
  // searchResults.subscribe(value => { localStorage.searchResults = value })
  // searchHistory.subscribe(value => { localStorage.searchHistory = value })
  // searchQuery.subscribe(value => { localStorage.searchQuery = value })
  // documentsShown.subscribe(value => { localStorage.documentsShown = value })
  // timeTaken.subscribe(value => { localStorage.timeTaken = value })
  // numResultsReceived.subscribe(value => { localStorage.numResultsReceived = value })
  // dbStats.subscribe(value => { localStorage.dbStats = value })
  // currentCollection.subscribe(value => { localStorage.currentCollection = value })
  // user.subscribe(value => { localStorage.user = value })
}
