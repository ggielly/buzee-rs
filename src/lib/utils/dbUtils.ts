import { invoke } from "@tauri-apps/api/core";
import { extractDate, cleanSearchQuery } from "./queryParsing";
import { searchQuery, locationShown, resultsPageShown, noMoreResults, searchInProgress, filetypeShown, resultsPerPage, documentsShown, allowedExtensions, clearBase64Images, showIconGrid,  dateLimitUNIX } from "$lib/stores";
import { trackEvent } from "@aptabase/web";
import { setExtensionCategory } from "$lib/utils/miscUtils";
import { getResultThumbnails } from '$lib/utils/fileTable';
import { get } from 'svelte/store';

// Lightweight in-memory search query cache to optimize IPC traffic
const searchCache = new Map<string, { results: DocumentSearchResult[]; timestamp: number }>();
const CACHE_TTL_MS = 45_000;

export function clearSearchCache() {
  searchCache.clear();
}

export async function getDocumentsFromDB(page:number, limit:number) {
  let filetypeToGet = get(filetypeShown);
  if (filetypeToGet !== 'any') {
    filetypeToGet = setExtensionCategory(get(filetypeShown), get(allowedExtensions));
  }

  let type: String | undefined = filetypeToGet;
  if (type === "any") type = undefined;
  
  const cacheKey = `recent|${page}|${limit}|${type || 'all'}`;
  const cached = searchCache.get(cacheKey);
  if (cached && Date.now() - cached.timestamp < CACHE_TTL_MS) {
    return cached.results;
  }

  console.log("getting documents from db of type", type);
  const results: DocumentSearchResult[] = await invoke("get_recent_docs", { page: page, limit: limit*2, fileType: type });
  searchCache.set(cacheKey, { results, timestamp: Date.now() });
  return results;
}

export async function searchDocuments(query:string, page:number, limit:number, type?:string, dateLimitUNIX?: ParsedDatesUNIX | null) {
  let results: DocumentSearchResult[] = [];
  const cacheKey = `${get(locationShown)}|${query}|${page}|${limit}|${type || 'any'}|${JSON.stringify(dateLimitUNIX || {})}`;
  const cached = searchCache.get(cacheKey);
  if (cached && Date.now() - cached.timestamp < CACHE_TTL_MS) {
    return cached.results;
  }

  console.log("searching documents with query", query, "page", page, "limit", limit, "type", type, "dateLimitUNIX", dateLimitUNIX);

  if (get(locationShown) === "my computer") {
    let dateLimit: ParsedDatesUNIX | null = null;
    if (dateLimitUNIX) { dateLimit = dateLimitUNIX; }
    let parsedDates = extractDate(query);
    
    if (dateLimitUNIX && dateLimitUNIX.start !== "" && dateLimitUNIX.end !== "") {
      if (parsedDates && parsedDates.start === dateLimitUNIX.start && parsedDates.end === dateLimitUNIX.end) {
        dateLimit = dateLimitUNIX;
        query = dateLimit.text;
      } else if (parsedDates) {
        dateLimit = parsedDates;
        query = dateLimit.text;
      }
    }
    if (dateLimit && dateLimit.text.length > 0) {
      query = dateLimit.text;
    }
    let querySegments = cleanSearchQuery(query);
    
    if (type === "any") type = undefined;

    if (query.length === 0 && !(dateLimitUNIX && dateLimitUNIX.start !== "" && dateLimitUNIX.end !== "")) {
      results = await getDocumentsFromDB(page, limit);
    } else {
      if (dateLimit && dateLimit.start !== "" && dateLimit.end !== "") {
        results = await invoke("run_search", { query: JSON.stringify(querySegments), page: page, limit: limit, fileType: type, dateLimit: dateLimit});
      } else {
        results = await invoke("run_search", { query: JSON.stringify(querySegments), page: page, limit: limit, fileType: type});
      }
    }
  } else if (get(locationShown) === "browser history") {
    results = await invoke("run_browser_history_search", { userProfile: "Default", userQuery: query, limit: limit, page: page});
  }
  searchCache.set(cacheKey, { results, timestamp: Date.now() });
  return results;
}

export async function triggerSearch() {
  resultsPageShown.set(0); // reset page number
  noMoreResults.set(false); // CRITICAL: Reset end-of-results flag for new search
  searchInProgress.set(true);
  clearBase64Images();
  trackEvent('search-triggered', {
    filetypeShown: get(filetypeShown),
    resultsPageShown: get(resultsPageShown)
  });
  let filetypeToGet = get(filetypeShown);
  if (filetypeToGet !== 'any') {
    filetypeToGet = setExtensionCategory(get(filetypeShown), get(allowedExtensions));
  }
  
  try {
    let result = await searchDocuments(
      get(searchQuery),
      0,
      get(resultsPerPage),
      filetypeToGet,
      get(dateLimitUNIX)
    );
    documentsShown.set(result);
    if (result.length < get(resultsPerPage)) {
      noMoreResults.set(true);
    }
    if (get(showIconGrid)) {
      await getResultThumbnails(get(documentsShown));
    }
  } catch (e) {
    console.error("Error during triggerSearch:", e);
  } finally {
    searchInProgress.set(false);
  }
}

export async function loadMoreResults() {
  if (get(noMoreResults) || get(searchInProgress)) return;
  
  const nextPage = get(resultsPageShown) + 1;
  resultsPageShown.set(nextPage);
  searchInProgress.set(true);
  trackEvent('loadMoreResults', {
    filetypeShown: get(filetypeShown),
    resultsPageShown: nextPage
  });
  let filetypeToGet = get(filetypeShown);
  if (filetypeToGet !== 'any') {
    filetypeToGet = setExtensionCategory(get(filetypeShown), get(allowedExtensions));
  }
  try {
    let results = await searchDocuments(
      get(searchQuery),
      nextPage,
      get(resultsPerPage),
      filetypeToGet,
    );
    if (!results || results.length === 0) {
      noMoreResults.set(true);
    } else {
      if (results.length < get(resultsPerPage)) {
        noMoreResults.set(true);
      }
      documentsShown.set([...get(documentsShown), ...results]);
      if (get(showIconGrid)) {
        await getResultThumbnails(results);
      }
    }
  } catch (e) {
    console.error("Error during loadMoreResults:", e);
  } finally {
    searchInProgress.set(false);
  }
}