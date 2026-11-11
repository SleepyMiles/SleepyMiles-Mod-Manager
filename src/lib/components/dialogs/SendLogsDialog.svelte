<script lang="ts">
	import Dialog from '$lib/components/ui/Dialog.svelte';
	import SendLogsForm from '$lib/components/misc/SendLogsForm.svelte';

	type Props = { open?: boolean };
	let { open = $bindable(false) }: Props = $props();
</script>

<Dialog bind:open title="Send logs to Miles">
	<SendLogsForm active={open} />
</Dialog>
