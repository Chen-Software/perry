import { App, VStack, Text, Button, TextField, Slider, Toggle } from "perry/ui";

const label = Text("Hello Geisterhand");

let count = 0;
const countLabel = Text("Count: 0");

const btn = Button("Click Me", () => {
  count++;
  countLabel.setText("Click count: " + count);
});

const field = TextField("Type here...", (text: string) => {
  console.log("TextField:", text);
});

const slider = Slider(0, 100, 50, (value: number) => {
  console.log("Slider:", value);
});

const toggle = Toggle("Enable", false, (on: boolean) => {
  console.log("Toggle:", on);
});

const stack = VStack(8, [label, countLabel, btn, field, slider, toggle]);

App({ title: "Geisterhand Test", width: 400, height: 350, body: stack });
