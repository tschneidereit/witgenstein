// Multi-module test - entry file
export { processItems } from './processor';
export { Config } from './config';

export function initialize(config: import('./config').Config): boolean {
    return true;
}
