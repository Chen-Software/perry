// JSON parse + stringify polyglot benchmark — Go.
// 10k records, ~1 MB blob, 50 iterations.
// IDENTICAL workload to bench.ts / bench.rs / bench.swift / bench.cpp / bench.js.

package main

import (
	"encoding/json"
	"fmt"
	"strconv"
	"time"
)

type Nested struct {
	X int `json:"x"`
	Y int `json:"y"`
}

type Item struct {
	Id     int      `json:"id"`
	Name   string   `json:"name"`
	Value  float64  `json:"value"`
	Tags   []string `json:"tags"`
	Nested Nested   `json:"nested"`
}

func main() {
	items := make([]Item, 10000)
	for i := 0; i < 10000; i++ {
		items[i] = Item{
			Id:    i,
			Name:  "item_" + strconv.Itoa(i),
			Value: float64(i) * 3.14159,
			Tags:  []string{"tag_" + strconv.Itoa(i%10), "tag_" + strconv.Itoa(i%5)},
			Nested: Nested{X: i, Y: i * 2},
		}
	}
	blob, _ := json.Marshal(items)

	// Warmup
	for i := 0; i < 3; i++ {
		var parsed []Item
		_ = json.Unmarshal(blob, &parsed)
		_, _ = json.Marshal(parsed)
	}

	const iterations = 50
	start := time.Now()

	checksum := 0
	for iter := 0; iter < iterations; iter++ {
		var parsed []Item
		_ = json.Unmarshal(blob, &parsed)
		checksum += len(parsed)
		reStringified, _ := json.Marshal(parsed)
		checksum += len(reStringified)
	}

	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("ms:%d\n", elapsed)
	fmt.Printf("checksum:%d\n", checksum)
}
