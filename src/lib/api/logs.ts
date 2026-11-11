import { invoke } from '$lib/invoke';

/** See src-tauri/src/send_logs.rs. */
export type LogFile = {
	name: string;
	label: string;
	path: string;
	exists: boolean;
	size: number;
	modified: number | null;
};

export type LogsInfo = {
	profile: string;
	modCount: number;
	files: LogFile[];
};

export const info = () => invoke<LogsInfo>('logs_info');
export const send = (name: string, message: string) => invoke('send_logs', { name, message });
export const save = (name: string, message: string) =>
	invoke<string>('save_logs', { name, message });
