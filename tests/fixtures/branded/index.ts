// Branded types test
import { u32, f32 } from 'ts-auto-wit';

export function addInts(a: u32, b: u32): u32 {
    return (a + b) as u32;
}

export function scale(value: f32, factor: f32): f32 {
    return (value * factor) as f32;
}

export interface Dimensions {
    width: u32;
    height: u32;
}
