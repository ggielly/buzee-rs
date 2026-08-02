<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import Dashboard from '$lib/components/dashboard/Dashboard.svelte';
	import { onSearchPage, userPreferences } from '$lib/stores';
	import { invoke } from '@tauri-apps/api/core';

	onMount(async () => {
		onSearchPage.set(false);
		// get user preferences here because this somehow loads before layout finishes its onMount
		$userPreferences = await invoke("get_user_preferences_state");
		if (!$userPreferences.onboarding_done) {
			goto('/onboarding');
		}
	});
</script>

{#if $userPreferences.onboarding_done}
	<Dashboard />
{:else}
	<div class="flex h-full items-center justify-center text-muted-foreground">Loading…</div>
{/if}