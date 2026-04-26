// demonstrates: per-API state snippets shown in docs/src/ui/state.md
// docs: docs/src/ui/state.md
// platforms: macos, linux, windows

import {
    App,
    VStack,
    Text, Button, TextField, ForEach, Spacer,
    State, stateOnChange, stateBindTextfield,
} from "perry/ui"

// ANCHOR: creating
const counter = State(0)               // number state
const username = State("Perry")        // string state
const items = State<string[]>([])      // array state
// ANCHOR_END: creating

// ANCHOR: read-write
const value = counter.value     // Read current value
counter.set(42)                  // Set new value → triggers UI update
// ANCHOR_END: read-write

// ANCHOR: reactive-text
const showCount = State(0)
const countLabel = Text(`Count: ${showCount.value}`)
// The text updates whenever showCount changes.
// ANCHOR_END: reactive-text

// ANCHOR: bind-textfield
const input = State("")
const field = TextField("Type here...", (v: string) => input.set(v))

// Optional: also let input.set("hello") update the field on screen.
stateBindTextfield(input, field)
// ANCHOR_END: bind-textfield

// ANCHOR: on-change
const watched = State(0)
stateOnChange(watched, (newValue: number) => {
    console.log(`Count changed to ${newValue}`)
})
// ANCHOR_END: on-change

// ANCHOR: foreach
const fruits = State(["Apple", "Banana", "Cherry"])
const fruitCount = State(3)

const fruitList = VStack(16, [
    ForEach(fruitCount, (i: number) =>
        Text(`${i + 1}. ${fruits.value[i]}`),
    ),
])
// ANCHOR_END: foreach

// ANCHOR: foreach-mutate
// Add an item:
fruits.set([...fruits.value, "Date"])
fruitCount.set(fruitCount.value + 1)

// Remove an item:
fruits.set(fruits.value.filter((_, i) => i !== 1))
fruitCount.set(fruitCount.value - 1)
// ANCHOR_END: foreach-mutate

// ANCHOR: conditional
const showDetails = State(false)
const detailsLabel: number = showDetails.value
    ? Text("Details are visible!")
    : Spacer()
const detailsPanel = VStack(16, [
    Button("Toggle", () => showDetails.set(!showDetails.value)),
    detailsLabel,
])
// ANCHOR_END: conditional

// ANCHOR: multi-state
const firstName = State("John")
const lastName = State("Doe")

const greeting = Text(`Hello, ${firstName.value} ${lastName.value}!`)
// Updates when either firstName or lastName changes.
// ANCHOR_END: multi-state

// ANCHOR: object-state
const user = State({ name: "Perry", age: 0 })

// Update by replacing the whole object:
user.set({ ...user.value, age: 1 })

const todos = State<{ text: string; done: boolean }[]>([])

// Add a todo:
todos.set([...todos.value, { text: "New task", done: false }])

// Toggle a todo (must produce a new array reference):
const next = todos.value.slice()
if (next.length > 0) {
    next[0] = { ...next[0], done: !next[0].done }
    todos.set(next)
}
// ANCHOR_END: object-state

App({
    title: "state-snippets",
    width: 480,
    height: 600,
    body: VStack(12, [
        Text(`counter=${counter.value}`),
        Text(`username=${username.value}`),
        Text(`items=${items.value.length}`),
        countLabel,
        field,
        Text(`watched=${watched.value}`),
        fruitList,
        detailsPanel,
        greeting,
        Text(`user.age=${user.value.age}`),
    ]),
})
