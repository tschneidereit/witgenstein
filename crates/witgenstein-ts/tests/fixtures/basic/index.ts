// Basic exported function and types
export function greet(name: string): string {
    return `Hello, ${name}!`;
}

export function add(a: number, b: number): number {
    return a + b;
}

export interface Point {
    x: number;
    y: number;
}

export function distance(a: Point, b: Point): number {
    return Math.sqrt((a.x - b.x) ** 2 + (a.y - b.y) ** 2);
}

export enum Color {
    Red,
    Green,
    Blue,
}

export function getColorName(color: Color): string {
    return Color[color];
}
