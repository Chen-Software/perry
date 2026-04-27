// demonstrates: TypeScript subset shown in docs/src/language/type-system.md
// docs: docs/src/language/type-system.md
// platforms: macos, linux, windows

// Each ANCHOR block below is the exact code that the type-system docs
// page renders inline (via {{#include ... :NAME}}). Snippets are wrapped
// in their own functions so top-level identifiers don't collide across
// anchors. The whole file is compiled and run by the doc-tests harness —
// if anything drifts from what the compiler accepts, CI fails.

// ANCHOR: inference-basics
function inferenceBasics(): void {
    let x = 5;           // inferred as number
    let s = "hello";     // inferred as string
    let b = true;        // inferred as boolean
    let arr = [1, 2, 3]; // inferred as number[]

    console.log(`inference: x=${x} s=${s} b=${b} arr_len=${arr.length}`)
}
// ANCHOR_END: inference-basics

// ANCHOR: inference-function
function inferenceFunction(): void {
    function double(n: number): number {
        return n * 2;
    }
    let result = double(5); // inferred as number

    console.log(`inference-function: result=${result}`)
}
// ANCHOR_END: inference-function

// ANCHOR: annotations
interface Config {
    port: number;
    host: string;
}

function annotations(): void {
    let name: string = "Perry";
    let count: number = 0;
    let items: string[] = [];

    function greet(name: string): string {
        return `Hello, ${name}`;
    }

    const cfg: Config = { port: 8080, host: "localhost" }
    console.log(`annotations: ${greet(name)} count=${count} items=${items.length} port=${cfg.port}`)
}
// ANCHOR_END: annotations

// ANCHOR: utility-types
type MyPartial<T> = { [P in keyof T]?: T[P] };
type MyPick<T, K extends keyof T> = { [P in K]: T[P] };
type MyRecord<K extends string, V> = { [P in K]: V };
type MyOmit<T, K extends keyof T> = MyPick<T, Exclude<keyof T, K>>;
type MyReturnType<T extends (...args: any) => any> = T extends (...args: any) => infer R ? R : never;
type MyReadonly<T> = { readonly [P in keyof T]: T[P] };
// ANCHOR_END: utility-types

// ANCHOR: generics
function identity<T>(value: T): T {
    return value;
}

class Box<T> {
    value: T;
    constructor(value: T) {
        this.value = value;
    }
}

function genericsDemo(): void {
    const box = new Box<number>(42);
    const id = identity<string>("hello")
    console.log(`generics: box.value=${box.value} id=${id}`)
}
// ANCHOR_END: generics

// ANCHOR: union-narrowing
type StringOrNumber = string | number;

function process(value: StringOrNumber) {
    if (typeof value === "string") {
        console.log(value.toUpperCase());
    } else {
        console.log(value + 1);
    }
}
// ANCHOR_END: union-narrowing

// ANCHOR: type-guards
function isString(value: any): value is string {
    return typeof value === "string";
}

function typeGuardsDemo(): void {
    const x: any = "hello"
    if (isString(x)) {
        console.log(x.toUpperCase());
    }
}
// ANCHOR_END: type-guards

// driver
inferenceBasics()
inferenceFunction()
annotations()
genericsDemo()
process("hello")
process(41)
typeGuardsDemo()
