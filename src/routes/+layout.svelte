<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let { children } = $props();

  onMount(() => { getCurrentWindow().show(); });
</script>

{@render children()}
