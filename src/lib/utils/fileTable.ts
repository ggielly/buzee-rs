import { get, readable, writable } from 'svelte/store';
import { createTable } from 'svelte-headless-table';
import { addResizedColumns, addSortBy, addHiddenColumns } from 'svelte-headless-table/plugins';
import { documentsShown, resultsPerPage, base64Images, locationShown, upsertBase64Image } from '$lib/stores';
import { formatUpdatedTime } from '$lib/utils/searchItemUtils';
import { readableFileSize } from '$lib/utils/miscUtils';
import { invoke } from "@tauri-apps/api/core";

export function createTableFromResults(resultsShown: DocumentSearchResult[]) {
  const table = createTable(readable(resultsShown), {
    resize: addResizedColumns(),
    sort: addSortBy({ disableMultiSort: true }),
    hideCols: addHiddenColumns(),
  });

  let columnsArray:any = [];
  if (get(locationShown) === "my computer") {
    columnsArray = [
      table.column({
        header: 'Type',
        accessor: 'file_type',
        plugins: {
          resize: {
            initialWidth: 30,
            minWidth: 30,
            maxWidth: 30
          },
          sort: { disable: false }
        }
      }),
      table.column({
        header: 'Name',
        accessor: 'name',
        plugins: {
          resize: {
            initialWidth: 250,
            minWidth: 250,
            maxWidth: 250
          }
        }
      }),
      table.column({
        header: 'Last Modified',
        accessor: 'last_modified',
        id: 'lastModified',
        cell: ({ value }: { value: number }) => formatUpdatedTime(value) ?? value,
        plugins: {
          resize: {
            initialWidth: 140,
            minWidth: 125,
            maxWidth: 150
          }
        }
      }),
      table.column({
        header: 'Last Opened',
        accessor: 'last_opened',
        id: 'lastOpened',
        cell: ({ value }: { value: number }) => formatUpdatedTime(value) ?? value,
        plugins: {
          resize: {
            initialWidth: 140,
            minWidth: 125,
            maxWidth: 150
          }
        }
      }),
      table.column({
        header: 'Size',
        accessor: 'size',
        id: 'size',
        cell: ({ value }: { value: number }) => readableFileSize(value) ?? "",
        plugins: {
          resize: {
            initialWidth: 75,
            minWidth: 75,
            maxWidth: 75
          }
        }
      }),
      table.column({
        header: 'Location',
        accessor: 'path',
        plugins: {
          resize: {
            initialWidth: 200,
            minWidth: 200,
            maxWidth: 200
          }
        }
      })
    ];
  } else if (get(locationShown) === "browser history") {
    columnsArray = [
      table.column({
        header: 'Type',
        accessor: 'file_type',
        plugins: {
          resize: {
            initialWidth: 30,
            minWidth: 30,
            maxWidth: 30
          },
          sort: { disable: false }
        }
      }),
      table.column({
        header: 'Title',
        accessor: 'name',
        plugins: {
          resize: {
            initialWidth: 250,
            minWidth: 250,
            maxWidth: 250
          }
        }
      }),
      table.column({
        header: 'Last Opened',
        accessor: 'last_opened',
        id: 'lastOpened',
        cell: ({ value }: { value: number }) => formatUpdatedTime(value) ?? value,
        plugins: {
          resize: {
            initialWidth: 140,
            minWidth: 125,
            maxWidth: 150
          }
        }
      }),
      table.column({
        header: 'URL',
        accessor: 'path',
        plugins: {
          resize: {
            initialWidth: 200,
            minWidth: 200,
            maxWidth: 200
          }
        }
      }),
    ];
  }

  // @ts-ignore
  const columns = table.createColumns(columnsArray);

  return [table, columns];
}

const pendingThumbnailRequests = new Set<string>();

export async function getResultThumbnails(resultsShown: DocumentSearchResult[]) {
  const currentImages = get(base64Images);
  const existingPaths = new Set(currentImages.map((img) => img.path));

  for (const result of resultsShown) {
    if (
      ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(result.file_type) &&
      !existingPaths.has(result.path) &&
      !pendingThumbnailRequests.has(result.path)
    ) {
      pendingThumbnailRequests.add(result.path);
      try {
        const res = await invoke<string>('get_image_base64', { filePath: result.path });
        if (res) {
          upsertBase64Image({ path: result.path, base64: res });
        }
      } catch (err) {
        console.warn(`Failed to fetch thumbnail for ${result.path}`, err);
      } finally {
        pendingThumbnailRequests.delete(result.path);
      }
    }
  }
}

export function findBase64ImageObjectFromPath(path: string) {
  const images = get(base64Images);
  const found = images.find((img) => img.path === path);
  if (found) {
    return found;
  }
  return { path, base64: '' };
}