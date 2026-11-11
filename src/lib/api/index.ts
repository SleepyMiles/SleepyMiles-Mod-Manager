import { invoke } from '$lib/invoke';

export * as config from './config';
export * as logger from './logger';
export * as prefs from './prefs';
export * as profile from './profile';
export * as state from './state';
export * as thunderstore from './thunderstore';
export * as site from './site';
export * as storage from './storage';
export * as logs from './logs';

export const isFlatpak = () => invoke<boolean>('is_flatpak');
