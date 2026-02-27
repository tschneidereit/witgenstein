// Async and Result type test
export type Result<T, E> = { tag: 'ok', value: T } | { tag: 'err', value: E };

export async function fetchData(url: string): Promise<string> {
    return '';
}

export function parseNumber(input: string): Result<number, string> {
    const n = Number(input);
    if (isNaN(n)) {
        return { tag: 'err', value: 'not a number' };
    }
    return { tag: 'ok', value: n };
}

export function getOptionalName(id: number): string | undefined {
    return undefined;
}
